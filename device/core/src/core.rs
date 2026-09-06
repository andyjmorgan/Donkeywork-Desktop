use crate::{
    backend::{self, ActionGuard, Backend, Display, Input, Resolution},
    wire::Reply,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use uuid::Uuid;

const RETENTION: Duration = Duration::from_secs(120);
const LEASE: Duration = Duration::from_secs(45);
type Result<T> = backend::Result<T>;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub uid: u32,
    pub profile: String,
    pub permissions: Vec<String>,
}
#[derive(Clone)]
pub struct Connection {
    pub uid: u32,
    pub id: Uuid,
    pub cancelled: Arc<AtomicBool>,
}
impl Connection {
    pub fn new(uid: u32) -> Self {
        Self {
            uid,
            id: Uuid::new_v4(),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}
struct Snapshot {
    id: String,
    display: Display,
    captured: Instant,
}
struct Session {
    uid: u32,
    profile: String,
    epoch: Uuid,
    origin: Instant,
    connections: HashSet<Uuid>,
    retain_until: Option<Instant>,
    closed: bool,
    snapshots: VecDeque<Snapshot>,
}
struct Lease {
    id: Uuid,
    session: String,
    connection: Uuid,
    deadline: Instant,
    sequence: u64,
}
struct Cached {
    request: Value,
    reply: Reply,
    expires: Instant,
}
pub struct Core {
    worker_id: Uuid,
    policies: HashMap<u32, Policy>,
    backend: Box<dyn Backend>,
    sessions: HashMap<String, Session>,
    lease: Option<Lease>,
    cache: HashMap<(u32, String, String), Cached>,
    faulted: bool,
}
struct Guard {
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}
impl ActionGuard for Guard {
    fn check(&mut self) -> Result<()> {
        if self.cancelled.load(Ordering::Acquire) || Instant::now() >= self.deadline {
            Err("expired")
        } else {
            Ok(())
        }
    }
}
fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap()
}
fn number(value: &Value, key: &str) -> u64 {
    value[key].as_u64().unwrap()
}

impl Core {
    pub fn new(worker_id: Uuid, policies: Vec<Policy>, backend: Box<dyn Backend>) -> Result<Self> {
        let mut map = HashMap::new();
        for policy in policies {
            let allowed = [
                "desktop.view",
                "desktop.control",
                "desktop.resize",
                "clipboard.read",
                "clipboard.write",
                "terminal.open",
                "terminal.input",
            ];
            if policy.profile.is_empty()
                || policy.profile.len() > 128
                || policy.permissions.len() > 7
                || policy
                    .permissions
                    .iter()
                    .any(|p| !allowed.contains(&p.as_str()))
                || map.contains_key(&policy.uid)
            {
                return Err("invalid_argument");
            }
            map.insert(policy.uid, policy);
        }
        Ok(Self {
            worker_id,
            policies: map,
            backend,
            sessions: HashMap::new(),
            lease: None,
            cache: HashMap::new(),
            faulted: false,
        })
    }
    pub fn authorized(&self, uid: u32) -> bool {
        self.policies.contains_key(&uid)
    }
    fn effective(&self, uid: u32) -> Vec<String> {
        let caps = self.backend.capabilities();
        let mut result: Vec<_> = self.policies[&uid]
            .permissions
            .iter()
            .filter(|p| caps.contains(&p.as_str()))
            .cloned()
            .collect();
        result.sort();
        result.dedup();
        result
    }
    fn require(&self, uid: u32, permission: &str) -> Result<()> {
        if !self.policies[&uid]
            .permissions
            .iter()
            .any(|p| p == permission)
        {
            return Err("forbidden");
        }
        if !self.effective(uid).iter().any(|p| p == permission) {
            return Err("unsupported_capability");
        }
        if self.faulted {
            return Err("worker_unavailable");
        }
        Ok(())
    }
    fn release(&mut self) {
        if self.lease.take().is_some() && self.backend.release_all().is_err() {
            self.faulted = true;
        }
    }
    pub fn maintain(&mut self, now: Instant) {
        if self.lease.as_ref().is_some_and(|l| now >= l.deadline) {
            self.release();
        }
        let expired: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.retain_until.is_some_and(|t| now >= t))
            .map(|(id, _)| id.clone())
            .collect();
        for id in expired {
            if self.lease.as_ref().is_some_and(|l| l.session == id) {
                self.release();
            }
            self.sessions.remove(&id);
        }
        for session in self.sessions.values_mut() {
            session
                .snapshots
                .retain(|s| now.duration_since(s.captured) < RETENTION);
        }
        self.cache.retain(|_, c| now < c.expires);
    }
    pub fn disconnect(&mut self, connection: &Connection) {
        connection.cancelled.store(true, Ordering::Release);
        if self
            .lease
            .as_ref()
            .is_some_and(|l| l.connection == connection.id)
        {
            self.release();
        }
        for session in self.sessions.values_mut() {
            session.connections.remove(&connection.id);
            if session.connections.is_empty() && session.retain_until.is_none() {
                session.retain_until = Some(Instant::now() + RETENTION);
            }
        }
    }
    pub fn revoke_uid(&mut self, uid: u32) {
        self.policies.remove(&uid);
        let ids: HashSet<_> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.uid == uid)
            .map(|(id, _)| id.clone())
            .collect();
        if self
            .lease
            .as_ref()
            .is_some_and(|l| ids.contains(&l.session))
        {
            self.release();
        }
        self.sessions.retain(|_, s| s.uid != uid);
        self.cache.retain(|(u, _, _), _| *u != uid);
    }
    fn bound(&mut self, connection: &Connection, request: &Value) -> Result<()> {
        let id = string(request, "sessionId");
        let session = self.sessions.get_mut(id).ok_or("not_found")?;
        if session.uid != connection.uid
            || session.profile != self.policies[&connection.uid].profile
        {
            return Err("forbidden");
        }
        if session.epoch.to_string() != string(request, "sessionEpoch") {
            return Err("stale_epoch");
        }
        if session.closed {
            return if request["type"] == "session.close" {
                Ok(())
            } else {
                Err("not_found")
            };
        }
        session.connections.insert(connection.id);
        session.retain_until = None;
        Ok(())
    }
    fn guard(&self, connection: &Connection, request: &Value) -> Result<Guard> {
        let lease = self.lease.as_ref().ok_or("expired")?;
        if lease.connection != connection.id
            || lease.session != string(request, "sessionId")
            || lease.id.to_string() != string(&request["payload"], "leaseId")
        {
            return Err("control_conflict");
        }
        let mut guard = Guard {
            deadline: lease.deadline,
            cancelled: connection.cancelled.clone(),
        };
        guard.check()?;
        Ok(guard)
    }
    pub fn process(&mut self, connection: &Connection, request: Value) -> Reply {
        // Socket parsing performs strict validation first; library entrypoint also rejects safely.
        if !crate::wire::valid_request(&request) {
            return Reply::error(&request, "invalid_argument");
        }
        self.maintain(Instant::now());
        if !self.authorized(connection.uid) || connection.cancelled.load(Ordering::Acquire) {
            return Reply::error(&request, "unauthorized");
        }
        let kind = string(&request, "type");
        if request.get("sessionId").is_some() {
            if let Err(code) = self.bound(connection, &request) {
                return Reply::error(&request, code);
            }
        }
        let safe = matches!(
            kind,
            "session.open"
                | "session.close"
                | "terminal.open"
                | "terminal.close"
                | "display.resize"
        );
        let key = (
            connection.uid,
            request["sessionId"].as_str().unwrap_or("").to_owned(),
            string(&request, "messageId").to_owned(),
        );
        if safe {
            if let Some(cached) = self.cache.get(&key) {
                if cached.request != request {
                    return Reply::error(&request, "invalid_argument");
                }
                let mut reply = cached.reply.clone();
                reply.header["messageId"] = json!(Uuid::new_v4());
                return reply;
            }
            if self
                .cache
                .keys()
                .filter(|(u, _, _)| *u == connection.uid)
                .count()
                >= 256
            {
                return Reply::error(&request, "resource_exhausted");
            }
        }
        let reply = self
            .dispatch(connection, &request)
            .unwrap_or_else(|code| Reply::error(&request, code));
        if safe {
            self.cache.insert(
                key,
                Cached {
                    request,
                    reply: reply.clone(),
                    expires: Instant::now() + RETENTION,
                },
            );
        }
        reply
    }
    fn dispatch(&mut self, connection: &Connection, request: &Value) -> Result<Reply> {
        let kind = string(request, "type");
        let payload = &request["payload"];
        if kind == "describe" {
            let displays = if self
                .effective(connection.uid)
                .iter()
                .any(|x| x == "desktop.view")
            {
                self.backend.displays()?
            } else {
                vec![]
            };
            return Ok(Reply::new(
                request,
                "describe.result",
                json!({"workerId":self.worker_id,"peerUid":connection.uid,"osAccountProfile":self.policies[&connection.uid].profile,"permissions":self.effective(connection.uid),"displays":displays}),
            ));
        }
        if kind == "session.open" {
            if self.effective(connection.uid).is_empty() {
                return Err("unsupported_capability");
            }
            if self
                .sessions
                .values()
                .filter(|s| s.uid == connection.uid && !s.closed)
                .count()
                >= 2
            {
                return Err("resource_exhausted");
            }
            let id = Uuid::new_v4();
            let epoch = Uuid::new_v4();
            self.sessions.insert(
                id.to_string(),
                Session {
                    uid: connection.uid,
                    profile: self.policies[&connection.uid].profile.clone(),
                    epoch,
                    origin: Instant::now(),
                    connections: HashSet::from([connection.id]),
                    retain_until: None,
                    closed: false,
                    snapshots: VecDeque::new(),
                },
            );
            let mut bound = request.clone();
            bound["sessionId"] = json!(id);
            bound["sessionEpoch"] = json!(epoch);
            return Ok(Reply::new(
                &bound,
                "session.opened",
                json!({"state":"ready"}),
            ));
        }
        let sid = string(request, "sessionId").to_owned();
        match kind {
            "session.close" => {
                if self.lease.as_ref().is_some_and(|l| l.session == sid) {
                    self.release();
                }
                let session = self.sessions.get_mut(&sid).unwrap();
                session.closed = true;
                session.snapshots.clear();
                session.retain_until = Some(Instant::now() + RETENTION);
            }
            "control.acquire" => {
                self.require(connection.uid, "desktop.control")?;
                if self.lease.is_some() {
                    return Err("control_conflict");
                }
                let id = Uuid::new_v4();
                self.lease = Some(Lease {
                    id,
                    session: sid,
                    connection: connection.id,
                    deadline: Instant::now() + LEASE,
                    sequence: 0,
                });
                return Ok(Reply::new(
                    request,
                    "control.granted",
                    json!({"leaseId":id,"validForMs":45000}),
                ));
            }
            "control.renew" => {
                self.require(connection.uid, "desktop.control")?;
                self.guard(connection, request)?;
                let lease = self.lease.as_mut().unwrap();
                lease.deadline = Instant::now() + LEASE;
                return Ok(Reply::new(
                    request,
                    "control.granted",
                    json!({"leaseId":lease.id,"validForMs":45000}),
                ));
            }
            "control.release" => {
                if self.lease.is_some() {
                    self.guard(connection, request)?;
                    self.release();
                }
            }
            "screenshot.request" => return self.screenshot(connection, request),
            "input.pointer" | "input.key" | "input.text" | "input.reset" => {
                self.require(connection.uid, "desktop.control")?;
                let mut guard = self.guard(connection, request)?;
                let sequence = number(payload, "inputSequence");
                if sequence != self.lease.as_ref().unwrap().sequence + 1 {
                    return Err("stale_sequence");
                }
                if kind == "input.reset" {
                    self.lease.as_mut().unwrap().sequence = sequence;
                    self.backend.release_all()?;
                } else {
                    let display_id = string(payload, "displayId");
                    let snapshot = self.sessions[&sid]
                        .snapshots
                        .iter()
                        .find(|s| s.id == string(payload, "snapshotId"))
                        .ok_or("not_found")?;
                    if snapshot.display.display_id != display_id {
                        return Err("not_found");
                    }
                    let current = self
                        .backend
                        .displays()?
                        .into_iter()
                        .find(|d| d.display_id == display_id)
                        .ok_or("not_found")?;
                    if current.topology_revision != number(payload, "topologyRevision")
                        || current.topology_revision != snapshot.display.topology_revision
                        || current.width != snapshot.display.width
                        || current.height != snapshot.display.height
                    {
                        return Err("stale_topology");
                    }
                    let input = match kind {
                        "input.pointer" => {
                            let x = number(payload, "x");
                            let y = number(payload, "y");
                            if x >= current.width as u64 || y >= current.height as u64 {
                                return Err("invalid_argument");
                            }
                            let action = string(payload, "action");
                            let button = string(payload, "button");
                            if (action == "move") != (button == "none") {
                                return Err("invalid_argument");
                            }
                            Input::Pointer {
                                x: x as u32,
                                y: y as u32,
                                action: action.into(),
                                button: button.into(),
                            }
                        }
                        "input.key" => Input::Key {
                            usage: number(payload, "usage") as u16,
                            down: payload["action"] == "down",
                        },
                        _ => Input::Text(string(payload, "text").into()),
                    };
                    guard.check()?;
                    self.lease.as_mut().unwrap().sequence = sequence;
                    if let Err(code) = self.backend.inject(
                        current.topology_revision,
                        display_id,
                        input,
                        &mut guard,
                    ) {
                        self.release();
                        return Err(code);
                    }
                }
            }
            "display.resize" => {
                self.require(connection.uid, "desktop.resize")?;
                let mut guard = self.guard(connection, request)?;
                let display_id = string(payload, "displayId");
                let current = self
                    .backend
                    .displays()?
                    .into_iter()
                    .find(|d| d.display_id == display_id)
                    .ok_or("not_found")?;
                if current.topology_revision != number(payload, "topologyRevision") {
                    return Err("stale_topology");
                }
                let requested = Resolution {
                    width: number(payload, "width") as u32,
                    height: number(payload, "height") as u32,
                };
                if !current.can_resize || !current.available_resolutions.contains(&requested) {
                    return Ok(Reply::new(
                        request,
                        "display.resize.result",
                        json!({"displayId":display_id,"status":"rejected","width":current.width,"height":current.height,"topologyRevision":current.topology_revision,"reason":"unsupported_mode"}),
                    ));
                }
                self.backend.release_all()?;
                guard.deadline = guard.deadline.min(Instant::now() + Duration::from_secs(5));
                let outcome = self.backend.resize(
                    current.topology_revision,
                    display_id,
                    requested.clone(),
                    &mut guard,
                )?;
                let actual = self
                    .backend
                    .displays()?
                    .into_iter()
                    .find(|d| d.display_id == display_id)
                    .ok_or("capture_failed")?;
                if actual.width != outcome.display.width
                    || actual.height != outcome.display.height
                    || actual.topology_revision != outcome.display.topology_revision
                {
                    return Err("capture_failed");
                }
                if outcome.applied
                    && (actual.width != requested.width
                        || actual.height != requested.height
                        || (requested.width != current.width || requested.height != current.height)
                            && actual.topology_revision <= current.topology_revision)
                {
                    return Err("capture_failed");
                }
                return Ok(Reply::new(
                    request,
                    "display.resize.result",
                    json!({"displayId":display_id,"status":if outcome.applied {"applied"} else {"rejected"},"width":actual.width,"height":actual.height,"topologyRevision":actual.topology_revision,"reason":outcome.reason}),
                ));
            }
            k if k.starts_with("terminal.") => {
                return Err("unsupported_capability");
            }
            _ => return Err("invalid_argument"),
        }
        Ok(Reply::new(request, "ack", json!({})))
    }
    fn screenshot(&mut self, connection: &Connection, request: &Value) -> Result<Reply> {
        self.require(connection.uid, "desktop.view")?;
        let display_id = string(&request["payload"], "displayId");
        let include_cursor = request["payload"]["includeCursor"].as_bool().unwrap();
        let accepted = Instant::now();
        let deadline = accepted + Duration::from_secs(5);
        let frame = self
            .backend
            .capture_next(display_id, include_cursor, deadline)?;
        if Instant::now() > deadline
            || frame.captured_at < accepted
            || frame.captured_at > Instant::now()
        {
            return Err("capture_failed");
        }
        if connection.cancelled.load(Ordering::Acquire) {
            return Err("expired");
        }
        let display = self
            .backend
            .displays()?
            .into_iter()
            .find(|d| d.display_id == display_id)
            .ok_or("not_found")?;
        if frame.display_id != display_id
            || display.width != frame.width
            || display.height != frame.height
            || display.topology_revision != frame.topology_revision
        {
            return Err("stale_topology");
        }
        if frame.cursor_embedded != include_cursor {
            return Err("unsupported_capability");
        }
        let pixels = (frame.width as usize)
            .checked_mul(frame.height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or("resource_exhausted")?;
        if frame.width == 0
            || frame.height == 0
            || frame.width > 4096
            || frame.height > 4096
            || pixels > crate::wire::MAX_PNG
            || frame.rgba.len() != pixels
        {
            return Err("resource_exhausted");
        }
        struct Bounded(Vec<u8>);
        impl Write for Bounded {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if bytes.len() > crate::wire::MAX_PNG - self.0.len() {
                    return Err(std::io::Error::other("PNG limit"));
                }
                self.0.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut encoded = Bounded(Vec::new());
        {
            let mut encoder = png::Encoder::new(&mut encoded, frame.width, frame.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().map_err(|_| "capture_failed")?;
            writer
                .write_image_data(&frame.rgba)
                .map_err(|_| "capture_failed")?;
            writer.finish().map_err(|_| "capture_failed")?;
        }
        if Instant::now() > deadline {
            return Err("capture_failed");
        }
        let session = self.sessions.get_mut(string(request, "sessionId")).unwrap();
        let id = Uuid::new_v4().to_string();
        if session.snapshots.len() == 32 {
            session.snapshots.pop_front();
        }
        session.snapshots.push_back(Snapshot {
            id: id.clone(),
            display,
            captured: frame.captured_at,
        });
        let mut reply = Reply::new(
            request,
            "screenshot.result",
            json!({"snapshotId":id,"displayId":display_id,"topologyRevision":frame.topology_revision,"width":frame.width,"height":frame.height,"format":"png","pixelFormat":"rgba8","cursorEmbedded":frame.cursor_embedded,"captureTimeUs":frame.captured_at.duration_since(session.origin).as_micros() as u64,"payloadBytes":encoded.0.len()}),
        );
        reply.binary = encoded.0;
        Ok(reply)
    }
}
