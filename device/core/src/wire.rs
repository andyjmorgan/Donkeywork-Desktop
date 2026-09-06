use serde_json::{Value, json};
use std::{io, sync::OnceLock};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use uuid::Uuid;

pub const MAX_HEADER: usize = 65_536;
pub const MAX_PNG: usize = 67_108_864;
pub const REQUESTS: &[&str] = &[
    "describe",
    "session.open",
    "session.close",
    "control.acquire",
    "control.renew",
    "control.release",
    "screenshot.request",
    "input.pointer",
    "input.key",
    "input.text",
    "input.reset",
    "display.resize",
    "terminal.open",
    "terminal.input",
    "terminal.resize",
    "terminal.close",
    "terminal.resume",
];

fn schema() -> &'static jsonschema::Validator {
    static SCHEMA: OnceLock<jsonschema::Validator> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        let mut local: Value = serde_json::from_str(include_str!(
            "../../../contracts/schemas/local-cli.schema.json"
        ))
        .expect("checked-in schema");
        let common: Value = serde_json::from_str(include_str!(
            "../../../contracts/schemas/common.schema.json"
        ))
        .expect("checked-in common schema");
        for (name, definition) in common["$defs"].as_object().unwrap() {
            local["$defs"][name] = definition.clone();
        }
        fn localize(value: &mut Value) {
            match value {
                Value::Object(map) => {
                    if let Some(Value::String(reference)) = map.get_mut("$ref") {
                        if reference.starts_with(
                            "https://donkeywork.dev/desktop/contracts/0.1.0/common.schema.json#",
                        ) {
                            *reference = reference.split_once('#').unwrap().1.to_owned();
                            reference.insert(0, '#');
                        }
                    }
                    for value in map.values_mut() {
                        localize(value);
                    }
                }
                Value::Array(items) => {
                    for item in items {
                        localize(item);
                    }
                }
                _ => {}
            }
        }
        // Do not overwrite local display bounds with the broader M1b display definition.
        let original: Value = serde_json::from_str(include_str!(
            "../../../contracts/schemas/local-cli.schema.json"
        ))
        .unwrap();
        local["$defs"]["display"] = original["$defs"]["display"].clone();
        localize(&mut local);
        jsonschema::options()
            .should_validate_formats(true)
            .build(&local)
            .expect("valid offline schema")
    })
}

pub fn conforms(message: &Value) -> bool {
    schema().is_valid(message)
}
pub fn valid_request(message: &Value) -> bool {
    conforms(message) && REQUESTS.contains(&message["type"].as_str().unwrap_or(""))
}
fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid protocol record")
}

pub async fn read_request<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<Value> {
    let length = reader.read_u32().await? as usize;
    if length == 0 || length > MAX_HEADER {
        return Err(invalid());
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes).await?;
    let message: Value = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if !valid_request(&message) {
        return Err(invalid());
    }
    Ok(message)
}

#[derive(Clone)]
pub struct Reply {
    pub header: Value,
    pub binary: Vec<u8>,
}
impl Reply {
    pub fn new(request: &Value, kind: &str, payload: Value) -> Self {
        let mut header = json!({"protocol":"dwdesktop.local","version":"0.2.0","type":kind,"messageId":Uuid::new_v4(),"requestMessageId":request["messageId"],"payload":payload});
        if kind != "error" && kind != "describe.result" {
            header["sessionId"] = request["sessionId"].clone();
            header["sessionEpoch"] = request["sessionEpoch"].clone();
        }
        Self {
            header,
            binary: vec![],
        }
    }
    pub fn error(request: &Value, code: &str) -> Self {
        Self::new(
            request,
            "error",
            json!({"code":code,"message":code,"retryable":false}),
        )
    }
}

pub async fn write_reply<W: AsyncWrite + Unpin>(writer: &mut W, reply: &Reply) -> io::Result<()> {
    if !conforms(&reply.header) {
        return Err(invalid());
    }
    let bytes = serde_json::to_vec(&reply.header).map_err(|_| invalid())?;
    if bytes.is_empty() || bytes.len() > MAX_HEADER || reply.binary.len() > MAX_PNG {
        return Err(invalid());
    }
    if reply.header["type"] == "screenshot.result" {
        if reply.header["payload"]["payloadBytes"].as_u64() != Some(reply.binary.len() as u64)
            || reply.binary.is_empty()
        {
            return Err(invalid());
        }
    } else if !reply.binary.is_empty() {
        return Err(invalid());
    }
    writer.write_u32(bytes.len() as u32).await?;
    writer.write_all(&bytes).await?;
    writer.write_all(&reply.binary).await?;
    writer.flush().await
}
