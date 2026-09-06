//! Original Linux client for the local v0.2.0 protocol. No desktop implementation.
use nix::sys::socket::{getsockopt, sockopt::PeerCredentials};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Write},
    os::unix::{
        fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
        net::UnixStream,
    },
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};

pub const HEADER_LIMIT: usize = 65_536;
pub const PNG_LIMIT: usize = 67_108_864;
pub type Result<T> = std::result::Result<T, Failure>;

/// Intentionally never retain a remote free-form message or raw parser error.
#[derive(Debug, Serialize)]
pub struct Failure {
    pub code: String,
    pub message: &'static str,
    pub retryable: bool,
}
impl Failure {
    pub fn new(code: &str, message: &'static str) -> Self {
        Self {
            code: code.into(),
            message,
            retryable: false,
        }
    }
}
impl From<std::io::Error> for Failure {
    fn from(_: std::io::Error) -> Self {
        Self::new(
            "io_error",
            "Local I/O failed; delivery may be ambiguous. No automatic retry performed.",
        )
    }
}
fn invalid() -> Failure {
    Failure::new("invalid_record", "Protocol record failed validation.")
}

// Reject duplicate JSON keys instead of letting serde_json silently keep the
// last identity/type/permission-bearing field. The normal recursion limit stays on.
struct Strict(Value);
impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Strict;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("JSON without duplicate keys")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> std::result::Result<Strict, E> {
                Ok(Strict(json!(v)))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Strict, E> {
                Ok(Strict(json!(v)))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Strict, E> {
                Ok(Strict(json!(v)))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Strict, E> {
                Ok(Strict(json!(v)))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<Strict, E> {
                Ok(Strict(json!(v)))
            }
            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Strict, A::Error> {
                let mut values = Vec::new();
                while let Some(Strict(v)) = a.next_element()? {
                    values.push(v);
                }
                Ok(Strict(Value::Array(values)))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Strict, A::Error> {
                let mut object = serde_json::Map::new();
                while let Some(key) = a.next_key::<String>()? {
                    if object.contains_key(&key) {
                        return Err(serde::de::Error::custom("duplicate JSON key"));
                    }
                    let Strict(value) = a.next_value()?;
                    object.insert(key, value);
                }
                Ok(Strict(Value::Object(object)))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}
fn parse_strict(bytes: &[u8]) -> Result<Value> {
    serde_json::from_slice::<Strict>(bytes)
        .map(|v| v.0)
        .map_err(|_| invalid())
}

fn validator() -> &'static jsonschema::Validator {
    static VALIDATOR: OnceLock<jsonschema::Validator> = OnceLock::new();
    VALIDATOR.get_or_init(|| {
        let mut schema: Value = serde_json::from_str(include_str!("../../contracts/schemas/local-cli.schema.json")).expect("checked-in schema JSON");
        let common: Value = serde_json::from_str(include_str!("../../contracts/schemas/common.schema.json")).expect("checked-in common schema JSON");
        // Inline the approved common definitions: avoid all runtime URL resolution.
        schema["$defs"]["common"] = common["$defs"].clone();
        fn localize(v: &mut Value) {
            match v {
                Value::Object(o) => {
                    if let Some(Value::String(r)) = o.get_mut("$ref") {
                        if let Some(suffix) = r.strip_prefix("https://donkeywork.dev/desktop/contracts/0.1.0/common.schema.json#/$defs/") {
                            *r = format!("#/$defs/common/{suffix}");
                        }
                    }
                    for x in o.values_mut() { localize(x); }
                }
                Value::Array(a) => for x in a { localize(x); },
                _ => {}
            }
        }
        localize(&mut schema);
        jsonschema::options().should_validate_formats(true).build(&schema).expect("checked-in contract compiles")
    })
}
pub fn validate(value: &Value) -> Result<()> {
    if validator().is_valid(value) {
        Ok(())
    } else {
        Err(invalid())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Context {
    pub socket: PathBuf,
    pub server_uid: u32,
    pub session_id: String,
    pub session_epoch: String,
}
impl Context {
    pub fn check_endpoint(&self, socket: &Path, uid: u32) -> Result<()> {
        if self.socket != socket
            || self.server_uid != uid
            || uuid::Uuid::parse_str(&self.session_id).is_err()
            || uuid::Uuid::parse_str(&self.session_epoch).is_err()
        {
            return Err(Failure::new(
                "invalid_context",
                "Context does not match this endpoint or session format.",
            ));
        }
        Ok(())
    }
}

pub fn request(kind: &str, payload: Value, context: Option<&Context>) -> Value {
    let mut value = json!({"protocol":"dwdesktop.local","version":"0.2.0","type":kind,
        "messageId":uuid::Uuid::new_v4().to_string(),"payload":payload});
    if let Some(c) = context {
        value["sessionId"] = json!(c.session_id);
        value["sessionEpoch"] = json!(c.session_epoch);
    }
    value
}

pub struct Record {
    pub header: Value,
    pub binary: Vec<u8>,
}

pub fn write_record(writer: &mut impl Write, header: &Value) -> Result<()> {
    validate(header)?;
    let bytes = serde_json::to_vec(header).map_err(|_| invalid())?;
    if bytes.is_empty() || bytes.len() > HEADER_LIMIT {
        return Err(invalid());
    }
    writer.write_all(&(bytes.len() as u32).to_be_bytes())?;
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}

pub fn read_record(reader: &mut impl Read) -> Result<Record> {
    let mut prefix = [0; 4];
    reader.read_exact(&mut prefix)?;
    let size = u32::from_be_bytes(prefix) as usize;
    if size == 0 || size > HEADER_LIMIT {
        return Err(invalid());
    }
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes)?;
    let header = parse_strict(&bytes)?;
    validate(&header)?;
    let body_size = if header["type"] == "screenshot.result" {
        header["payload"]["payloadBytes"]
            .as_u64()
            .ok_or_else(invalid)? as usize
    } else {
        0
    };
    if body_size > PNG_LIMIT {
        return Err(invalid());
    }
    let mut binary = vec![0; body_size];
    reader.read_exact(&mut binary)?;
    if body_size > 0 {
        validate_png(&binary, &header["payload"])?;
    }
    Ok(Record { header, binary })
}

/// Pre-scan bounded chunks before invoking the maintained decoder. Exclude APNG,
/// compressed ancillary metadata, interlace and non-RGBA8 encodings explicitly.
pub fn validate_png(bytes: &[u8], metadata: &Value) -> Result<()> {
    if bytes.len() > PNG_LIMIT || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(invalid());
    }
    let width = metadata["width"].as_u64().ok_or_else(invalid)?;
    let height = metadata["height"].as_u64().ok_or_else(invalid)?;
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err(invalid());
    }
    let pixel_bytes = width
        .checked_mul(height)
        .and_then(|v| v.checked_mul(4))
        .ok_or_else(invalid)? as usize;
    if pixel_bytes > PNG_LIMIT {
        return Err(invalid());
    }
    let mut offset = 8usize;
    let mut first = true;
    let mut ended = false;
    while offset < bytes.len() {
        let h = bytes.get(offset..offset + 8).ok_or_else(invalid)?;
        let len = u32::from_be_bytes(h[..4].try_into().map_err(|_| invalid())?) as usize;
        let end = offset
            .checked_add(12)
            .and_then(|v| v.checked_add(len))
            .ok_or_else(invalid)?;
        if end > bytes.len() {
            return Err(invalid());
        }
        let kind = &h[4..8];
        if first {
            if kind != b"IHDR" || len != 13 {
                return Err(invalid());
            }
            let ihdr = &bytes[offset + 8..offset + 21];
            if u32::from_be_bytes(ihdr[..4].try_into().unwrap()) as u64 != width
                || u32::from_be_bytes(ihdr[4..8].try_into().unwrap()) as u64 != height
                || ihdr[8] != 8
                || ihdr[9] != 6
                || ihdr[10] != 0
                || ihdr[11] != 0
                || ihdr[12] != 0
            {
                return Err(invalid());
            }
            first = false;
        } else if kind == b"IHDR" {
            return Err(invalid());
        }
        // Restrict metadata rather than allow ancillary compressed-data allocations.
        if ![b"IHDR", b"IDAT", b"IEND", b"sRGB", b"gAMA", b"cHRM"]
            .contains(&kind.try_into().unwrap())
        {
            return Err(invalid());
        }
        if kind != b"IDAT" && len > 32 {
            return Err(invalid());
        }
        if kind == b"IEND" {
            if len != 0 || end != bytes.len() {
                return Err(invalid());
            }
            ended = true;
        }
        offset = end;
    }
    if !ended {
        return Err(invalid());
    }
    let mut decoder = png::Decoder::new_with_limits(
        Cursor::new(bytes),
        png::Limits {
            bytes: PNG_LIMIT + 4096,
        },
    );
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info().map_err(|_| invalid())?;
    if reader.output_buffer_size() != pixel_bytes {
        return Err(invalid());
    }
    let mut pixels = vec![0; pixel_bytes];
    let info = reader.next_frame(&mut pixels).map_err(|_| invalid())?;
    if info.width as u64 != width
        || info.height as u64 != height
        || info.buffer_size() != pixel_bytes
        || info.color_type != png::ColorType::Rgba
        || info.bit_depth != png::BitDepth::Eight
    {
        return Err(invalid());
    }
    reader.finish().map_err(|_| invalid())?;
    Ok(())
}

pub fn verify_peer(stream: &UnixStream, server_uid: u32) -> Result<()> {
    let peer = getsockopt(stream, PeerCredentials)
        .map_err(|_| Failure::new("unauthorized", "Cannot verify server peer UID."))?;
    if peer.uid() != server_uid {
        return Err(Failure::new(
            "unauthorized",
            "Server peer UID does not match configured identity.",
        ));
    }
    Ok(())
}
pub struct Client {
    stream: UnixStream,
    healthy: bool,
}
impl Client {
    pub fn connect(socket: &Path, server_uid: u32) -> Result<Self> {
        let meta = fs::symlink_metadata(socket)?;
        let parent = fs::symlink_metadata(socket.parent().ok_or_else(invalid)?)?;
        if !meta.file_type().is_socket()
            || meta.uid() != server_uid
            || meta.mode() & 0o007 != 0
            || !parent.is_dir()
            || parent.uid() != server_uid
            || parent.mode() & 0o022 != 0
        {
            return Err(Failure::new(
                "unsafe_socket",
                "Socket ownership or directory permissions are unsafe.",
            ));
        }
        let stream = UnixStream::connect(socket)?;
        verify_peer(&stream, server_uid)?;
        stream.set_read_timeout(Some(Duration::from_secs(20)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        Ok(Self {
            stream,
            healthy: true,
        })
    }
    pub fn call(
        &mut self,
        kind: &str,
        payload: Value,
        context: Option<&Context>,
        expected: &str,
    ) -> Result<Record> {
        if !self.healthy {
            return Err(Failure::new(
                "closed",
                "Connection failed; no further requests are allowed.",
            ));
        }
        let req = request(kind, payload, context);
        let result = (|| {
            write_record(&mut self.stream, &req)?;
            let response = read_record(&mut self.stream)?;
            let h = &response.header;
            if h["requestMessageId"] != req["messageId"] || h["messageId"] == req["messageId"] {
                return Err(invalid());
            }
            if h["type"] == "error" {
                return Err(Failure::new(
                    h["payload"]["code"].as_str().ok_or_else(invalid)?,
                    "Daemon rejected the request. No automatic retry performed.",
                ));
            }
            if h["type"] != expected {
                return Err(invalid());
            }
            if let Some(c) = context {
                if h["sessionId"] != c.session_id || h["sessionEpoch"] != c.session_epoch {
                    return Err(invalid());
                }
            }
            Ok(response)
        })();
        // Closing even for a semantic error safely releases the connection's lease.
        if result.is_err() {
            self.healthy = false;
            let _ = self.stream.shutdown(std::net::Shutdown::Both);
        }
        result
    }
    pub fn controlled<T>(
        &mut self,
        context: &Context,
        action: impl FnOnce(&mut Self, &str) -> Result<T>,
    ) -> Result<T> {
        let grant = self.call(
            "control.acquire",
            json!({}),
            Some(context),
            "control.granted",
        )?;
        let lease = grant.header["payload"]["leaseId"]
            .as_str()
            .ok_or_else(invalid)?
            .to_owned();
        let result = action(self, &lease);
        if self.healthy {
            let released = self.call(
                "control.release",
                json!({"leaseId":lease}),
                Some(context),
                "ack",
            );
            if result.is_ok() {
                released?;
            }
        }
        result
    }
}

pub fn private_read(path: &Path) -> Result<Vec<u8>> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)?;
    let m = file.metadata()?;
    if !m.is_file()
        || m.uid() != nix::unistd::geteuid().as_raw()
        || m.mode() & 0o077 != 0
        || m.len() > HEADER_LIMIT as u64
    {
        return Err(Failure::new(
            "unsafe_file",
            "Metadata file must be private, owned and bounded.",
        ));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(HEADER_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > HEADER_LIMIT {
        return Err(invalid());
    }
    Ok(bytes)
}
pub fn private_create(path: &Path) -> Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| {
            Failure::new(
                "artifact_exists_or_unwritable",
                "Cannot create private artifact; existing files are never overwritten.",
            )
        })
}
pub fn write_private(file: &mut File, bytes: &[u8]) -> Result<()> {
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

pub fn snapshot(path: &Path, context: &Context) -> Result<Value> {
    let value = parse_strict(&private_read(path)?)?;
    validate(&value)?;
    if value["type"] != "screenshot.result"
        || value["sessionId"] != context.session_id
        || value["sessionEpoch"] != context.session_epoch
    {
        return Err(invalid());
    }
    Ok(value["payload"].clone())
}
