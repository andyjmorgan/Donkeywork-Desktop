use dwdesktop_core::{
    backend::*,
    core::{Connection, Core, Policy},
    wire::{self, Reply},
};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Default)]
struct State {
    revision: u64,
    width: u32,
    height: u32,
    injections: usize,
    releases: usize,
    stale_frame: bool,
}
struct Fake(Arc<Mutex<State>>);
impl Fake {
    fn display(&self) -> Display {
        let s = self.0.lock().unwrap();
        Display {
            display_id: "display".into(),
            width: s.width,
            height: s.height,
            topology_revision: s.revision,
            cursor_embedded: false,
            can_resize: true,
            available_resolutions: vec![
                Resolution {
                    width: 4,
                    height: 4,
                },
                Resolution {
                    width: 2,
                    height: 2,
                },
            ],
        }
    }
}
impl Backend for Fake {
    fn capabilities(&self) -> Vec<&'static str> {
        vec!["desktop.view", "desktop.control", "desktop.resize"]
    }
    fn displays(&mut self) -> Result<Vec<Display>> {
        Ok(vec![self.display()])
    }
    fn capture_next(&mut self, id: &str, cursor: bool, _: Instant) -> Result<Frame> {
        let s = self.0.lock().unwrap();
        Ok(Frame {
            display_id: id.into(),
            topology_revision: s.revision,
            captured_at: if s.stale_frame {
                Instant::now() - Duration::from_secs(1)
            } else {
                Instant::now()
            },
            width: s.width,
            height: s.height,
            cursor_embedded: cursor,
            rgba: vec![0x55; (s.width * s.height * 4) as usize],
        })
    }
    fn inject(&mut self, _: u64, _: &str, _: Input, guard: &mut dyn ActionGuard) -> Result<()> {
        guard.check()?;
        self.0.lock().unwrap().injections += 1;
        Ok(())
    }
    fn resize(
        &mut self,
        _: u64,
        _: &str,
        resolution: Resolution,
        guard: &mut dyn ActionGuard,
    ) -> Result<ResizeResult> {
        guard.check()?;
        {
            let mut s = self.0.lock().unwrap();
            s.width = resolution.width;
            s.height = resolution.height;
            s.revision += 1;
        }
        Ok(ResizeResult {
            display: self.display(),
            applied: true,
            reason: "none",
        })
    }
    fn release_all(&mut self) -> Result<()> {
        self.0.lock().unwrap().releases += 1;
        Ok(())
    }
}
fn request(kind: &str, payload: Value, session: Option<&Reply>) -> Value {
    let mut r = json!({"protocol":"dwdesktop.local","version":"0.2.0","type":kind,"messageId":Uuid::new_v4(),"payload":payload});
    if let Some(session) = session {
        r["sessionId"] = session.header["sessionId"].clone();
        r["sessionEpoch"] = session.header["sessionEpoch"].clone();
    }
    r
}
fn fixture(permissions: &[&str]) -> (Core, Connection, Arc<Mutex<State>>) {
    let state = Arc::new(Mutex::new(State {
        revision: 1,
        width: 4,
        height: 4,
        ..State::default()
    }));
    let policy = Policy {
        uid: 1234,
        profile: "pilot".into(),
        permissions: permissions.iter().map(|s| s.to_string()).collect(),
    };
    (
        Core::new(Uuid::new_v4(), vec![policy], Box::new(Fake(state.clone()))).unwrap(),
        Connection::new(1234),
        state,
    )
}
fn result(core: &mut Core, connection: &Connection, request: Value) -> Reply {
    assert!(wire::valid_request(&request));
    let reply = core.process(connection, request);
    assert!(wire::conforms(&reply.header), "{}", reply.header);
    reply
}
fn error(reply: Reply, code: &str) {
    assert_eq!(reply.header["type"], "error");
    assert_eq!(reply.header["payload"]["code"], code);
}

#[test]
fn canonical_examples_conform_and_spoofed_identity_does_not() {
    let examples: Vec<Value> =
        serde_json::from_str(include_str!("../../../contracts/fixtures/local-cli.json")).unwrap();
    for example in examples {
        assert!(wire::conforms(&example), "{}", example["type"]);
    }
    let mut r = request("describe", json!({}), None);
    r["uid"] = json!(0);
    assert!(!wire::valid_request(&r));
    r.as_object_mut().unwrap().remove("uid");
    r["type"] = json!("attachment.redeem");
    assert!(!wire::valid_request(&r));
}

#[test]
fn unavailable_backend_never_claims_a_session_or_desktop() {
    let connection = Connection::new(1234);
    let mut core = Core::new(
        Uuid::new_v4(),
        vec![Policy {
            uid: 1234,
            profile: "pilot".into(),
            permissions: vec!["desktop.view".into()],
        }],
        Box::new(Unavailable),
    )
    .unwrap();
    let reply = result(&mut core, &connection, request("describe", json!({}), None));
    assert_eq!(reply.header["payload"]["permissions"], json!([]));
    error(
        result(
            &mut core,
            &connection,
            request("session.open", json!({}), None),
        ),
        "unsupported_capability",
    );
}

#[test]
fn policy_idempotency_session_limits_epoch_and_close() {
    let (mut core, connection, _) = fixture(&["desktop.view"]);
    error(
        result(
            &mut core,
            &Connection::new(0),
            request("describe", json!({}), None),
        ),
        "unauthorized",
    );
    let open = request("session.open", json!({}), None);
    let first = result(&mut core, &connection, open.clone());
    assert_eq!(
        result(&mut core, &connection, open.clone()).header["sessionId"],
        first.header["sessionId"]
    );
    result(
        &mut core,
        &connection,
        request("session.open", json!({}), None),
    );
    error(
        result(
            &mut core,
            &connection,
            request("session.open", json!({}), None),
        ),
        "resource_exhausted",
    );
    let mut stale = request(
        "screenshot.request",
        json!({"displayId":"display","includeCursor":false}),
        Some(&first),
    );
    stale["sessionEpoch"] = json!(Uuid::new_v4());
    error(result(&mut core, &connection, stale), "stale_epoch");
    let close = request("session.close", json!({}), Some(&first));
    assert_eq!(
        result(&mut core, &connection, close.clone()).header["type"],
        "ack"
    );
    assert_eq!(result(&mut core, &connection, close).header["type"], "ack");
    error(
        result(
            &mut core,
            &connection,
            request(
                "screenshot.request",
                json!({"displayId":"display","includeCursor":false}),
                Some(&first),
            ),
        ),
        "not_found",
    );
}

#[test]
fn fresh_png_view_without_control_and_revocation() {
    let (mut core, connection, state) = fixture(&["desktop.view"]);
    let session = result(
        &mut core,
        &connection,
        request("session.open", json!({}), None),
    );
    error(
        result(
            &mut core,
            &connection,
            request("control.acquire", json!({}), Some(&session)),
        ),
        "forbidden",
    );
    let shot = result(
        &mut core,
        &connection,
        request(
            "screenshot.request",
            json!({"displayId":"display","includeCursor":false}),
            Some(&session),
        ),
    );
    let mut reader = png::Decoder::new(shot.binary.as_slice())
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let decoded = reader.next_frame(&mut pixels).unwrap();
    assert_eq!((decoded.width, decoded.height), (4, 4));
    assert!(pixels.iter().all(|x| *x == 0x55));
    state.lock().unwrap().stale_frame = true;
    error(
        result(
            &mut core,
            &connection,
            request(
                "screenshot.request",
                json!({"displayId":"display","includeCursor":false}),
                Some(&session),
            ),
        ),
        "capture_failed",
    );
    core.revoke_uid(connection.uid);
    error(
        result(&mut core, &connection, request("describe", json!({}), None)),
        "unauthorized",
    );
}

#[test]
fn lease_snapshot_input_resize_replay_and_cleanup() {
    let (mut core, connection, state) =
        fixture(&["desktop.view", "desktop.control", "desktop.resize"]);
    let session = result(
        &mut core,
        &connection,
        request("session.open", json!({}), None),
    );
    let shot = result(
        &mut core,
        &connection,
        request(
            "screenshot.request",
            json!({"displayId":"display","includeCursor":false}),
            Some(&session),
        ),
    );
    let lease = result(
        &mut core,
        &connection,
        request("control.acquire", json!({}), Some(&session)),
    );
    let lease_id = lease.header["payload"]["leaseId"].clone();
    let payload = json!({"leaseId":lease_id,"snapshotId":shot.header["payload"]["snapshotId"],"displayId":"display","topologyRevision":1,"inputSequence":1,"x":3,"y":3,"action":"click","button":"left"});
    let click = request("input.pointer", payload.clone(), Some(&session));
    assert_eq!(
        result(&mut core, &connection, click.clone()).header["type"],
        "ack"
    );
    error(result(&mut core, &connection, click), "stale_sequence");
    let mut out = payload.clone();
    out["inputSequence"] = json!(2);
    out["x"] = json!(4);
    error(
        result(
            &mut core,
            &connection,
            request("input.pointer", out, Some(&session)),
        ),
        "invalid_argument",
    );
    let resize = request(
        "display.resize",
        json!({"leaseId":lease_id,"displayId":"display","topologyRevision":1,"width":2,"height":2}),
        Some(&session),
    );
    assert_eq!(
        result(&mut core, &connection, resize.clone()).header["payload"]["status"],
        "applied"
    );
    assert_eq!(
        result(&mut core, &connection, resize.clone()).header["payload"]["topologyRevision"],
        2
    );
    let mut conflict = resize;
    conflict["payload"]["width"] = json!(4);
    error(result(&mut core, &connection, conflict), "invalid_argument");
    let mut stale = payload;
    stale["inputSequence"] = json!(2);
    error(
        result(
            &mut core,
            &connection,
            request("input.pointer", stale, Some(&session)),
        ),
        "stale_topology",
    );
    assert_eq!(state.lock().unwrap().injections, 1);
    core.maintain(Instant::now() + Duration::from_secs(46));
    assert!(state.lock().unwrap().releases >= 2);
    error(
        result(
            &mut core,
            &connection,
            request("control.renew", json!({"leaseId":lease_id}), Some(&session)),
        ),
        "expired",
    );
    result(
        &mut core,
        &connection,
        request("control.acquire", json!({}), Some(&session)),
    );
    core.disconnect(&connection);
    assert!(state.lock().unwrap().releases >= 3);
}
