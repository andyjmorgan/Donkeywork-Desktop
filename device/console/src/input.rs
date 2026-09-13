use serde::{Deserialize, Serialize};
use std::io;

pub const MAX_INPUT: usize = 4096;
pub const LEASE_MS: u64 = 1000;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hello {
    #[serde(rename = "type")]
    pub kind: String,
    pub protocol: String,
    pub version: String,
    pub generation: String,
    pub width: u32,
    pub height: u32,
    pub lease_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub generation: String,
    pub sequence: u64,
    pub event: Event,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum Event {
    Move {
        x: u32,
        y: u32,
    },
    Button {
        button: u8,
        down: bool,
        x: u32,
        y: u32,
    },
    Wheel {
        vertical: i16,
        horizontal: i16,
        x: u32,
        y: u32,
    },
    Key {
        hid: u16,
        down: bool,
    },
    Reset {},
    Renew {},
    Release {},
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum Acquire {
    Acquire {
        #[serde(rename = "requestId", default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unavailable {
    #[serde(rename = "type")]
    pub kind: String,
    pub reason: String,
    #[serde(rename = "requestId", default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ack {
    #[serde(rename = "type")]
    pub kind: String,
    pub sequence: u64,
    pub accepted: bool,
}

pub fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "input rejected")
}

pub fn validate(
    request: &Request,
    generation: &str,
    next: u64,
    width: u32,
    height: u32,
) -> io::Result<()> {
    if request.generation != generation || request.sequence != next {
        return Err(invalid());
    }
    match request.event {
        Event::Move { x, y } | Event::Button { x, y, .. } | Event::Wheel { x, y, .. }
            if x >= width || y >= height =>
        {
            return Err(invalid());
        }
        Event::Button { button, .. } if !(1..=3).contains(&button) => return Err(invalid()),
        Event::Wheel { vertical, horizontal, .. }
            if (vertical == 0 && horizontal == 0)
                || vertical.unsigned_abs() > 32
                || horizontal.unsigned_abs() > 32 => return Err(invalid()),
        _ => (),
    }
    Ok(())
}

pub fn peer_root(stream: &std::os::unix::net::UnixStream) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of_val(&cred) as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut cred as *mut libc::ucred).cast(),
            &mut len,
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    if cred.uid != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "root peer required",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistent_control_envelopes_are_strict() {
        assert!(serde_json::from_str::<Acquire>(r#"{"type":"acquire"}"#).is_ok());
        assert!(
            serde_json::from_str::<Acquire>(r#"{"type":"acquire","generation":"old"}"#).is_err()
        );
        assert!(serde_json::from_str::<Event>(r#"{"type":"release"}"#).is_ok());
        let ack = Ack {
            kind: "ack".into(),
            sequence: 1,
            accepted: true,
        };
        assert_eq!(
            serde_json::to_value(ack).unwrap(),
            serde_json::json!({"type":"ack","sequence":1,"accepted":true})
        );
    }
    #[test]
    fn reject_unknown_and_invalid_shape() {
        for json in [
            r#"{"type":"reset","extra":1}"#,
            r#"{"type":"move","x":-1,"y":0}"#,
            r#"{"type":"shell","cmd":"anything"}"#,
        ] {
            assert!(serde_json::from_str::<Event>(json).is_err());
        }
    }
    #[test]
    fn reject_stale_and_outside() {
        let mut r = Request {
            generation: "a".into(),
            sequence: 1,
            event: Event::Move { x: 1919, y: 1079 },
        };
        assert!(validate(&r, "a", 1, 1920, 1080).is_ok());
        assert!(validate(&r, "b", 1, 1920, 1080).is_err());
        assert!(validate(&r, "a", 2, 1920, 1080).is_err());
        r.event = Event::Move { x: 1920, y: 0 };
        assert!(validate(&r, "a", 1, 1920, 1080).is_err());
        r.event = Event::Wheel { vertical: -1, horizontal: 0, x: 10, y: 20 };
        assert!(validate(&r, "a", 1, 1920, 1080).is_ok());
        r.event = Event::Wheel { vertical: 0, horizontal: 0, x: 10, y: 20 };
        assert!(validate(&r, "a", 1, 1920, 1080).is_err());
        r.event = Event::Wheel { vertical: 33, horizontal: 0, x: 10, y: 20 };
        assert!(validate(&r, "a", 1, 1920, 1080).is_err());
        r.event = Event::Button {
            button: 4,
            down: true,
            x: 0,
            y: 0,
        };
        assert!(validate(&r, "a", 1, 1920, 1080).is_err());
    }
}
