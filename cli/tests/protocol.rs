use donkeywork_desktop_cli::*;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Cursor, Read, Write},
    os::unix::{
        fs::{symlink, MetadataExt, PermissionsExt},
        net::UnixListener,
    },
    process::Command,
    sync::{Arc, Mutex},
    thread,
};

const SESSION: &str = "22222222-2222-4222-8222-222222222222";
const EPOCH: &str = "33333333-3333-4333-8333-333333333333";
const LEASE: &str = "44444444-4444-4444-8444-444444444444";
fn png_image(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&vec![123; (width * height * 4) as usize])
            .unwrap();
    }
    bytes
}
fn reply(req: &Value, kind: &str, payload: Value) -> Value {
    let mut h = request(kind, payload, None);
    h["requestMessageId"] = req["messageId"].clone();
    if kind != "error" && kind != "describe.result" {
        h["sessionId"] = json!(SESSION);
        h["sessionEpoch"] = json!(EPOCH);
    }
    h
}

#[test]
fn canonical_examples_and_unknown_fields() {
    let examples: Vec<Value> =
        serde_json::from_str(include_str!("../../contracts/fixtures/local-cli.json")).unwrap();
    for example in examples {
        validate(&example).unwrap();
    }
    let mut r = request("describe", json!({}), None);
    r["uid"] = json!(0);
    assert!(validate(&r).is_err());
    r.as_object_mut().unwrap().remove("uid");
    r["version"] = json!("0.1.0");
    assert!(validate(&r).is_err());
    r["version"] = json!("0.2.0");
    r["messageId"] = json!("not-a-uuid");
    assert!(validate(&r).is_err());
}

#[test]
fn malformed_or_oversize_framing_is_rejected() {
    let duplicate = br#"{"protocol":"dwdesktop.local","version":"0.2.0","type":"describe","messageId":"11111111-1111-4111-8111-111111111111","payload":{},"payload":{}}"#;
    let mut duplicate_frame = (duplicate.len() as u32).to_be_bytes().to_vec();
    duplicate_frame.extend(duplicate);
    assert!(read_record(&mut Cursor::new(duplicate_frame)).is_err());
    assert!(read_record(&mut Cursor::new((65_537u32).to_be_bytes())).is_err());
    assert!(read_record(&mut Cursor::new([0u8; 4])).is_err());
    assert!(read_record(&mut Cursor::new(vec![0, 0, 0, 2, b'{'])).is_err());
    assert!(read_record(&mut Cursor::new(vec![0, 0, 0, 1, 0xff])).is_err());
    let mut h = reply(
        &request("describe", json!({}), None),
        "screenshot.result",
        json!({
        "snapshotId":LEASE,"displayId":"display-0","topologyRevision":1,"width":1,"height":1,
        "format":"png","pixelFormat":"rgba8","cursorEmbedded":false,"captureTimeUs":1,"payloadBytes":PNG_LIMIT+1}),
    );
    let bytes = serde_json::to_vec(&h).unwrap();
    let mut frame = (bytes.len() as u32).to_be_bytes().to_vec();
    frame.extend(bytes);
    assert!(read_record(&mut Cursor::new(frame)).is_err());
    h["payload"]["payloadBytes"] = json!(1);
    let mut frame = Vec::new();
    write_record(&mut frame, &h).unwrap();
    assert!(read_record(&mut Cursor::new(frame)).is_err());
}

#[test]
fn png_native_4k_and_dimension_bounds() {
    let bytes = png_image(3840, 2160);
    validate_png(&bytes, &json!({"width":3840,"height":2160})).unwrap();
    assert!(validate_png(&bytes, &json!({"width":1920,"height":1080})).is_err());
    assert!(validate_png(&bytes, &json!({"width":4097,"height":1})).is_err());
    assert!(validate_png(
        &bytes[..bytes.len() - 1],
        &json!({"width":3840,"height":2160})
    )
    .is_err());
    let mut corrupt = bytes.clone();
    corrupt[29] ^= 1;
    assert!(validate_png(&corrupt, &json!({"width":3840,"height":2160})).is_err());
    let mut animated = png_image(1, 1);
    animated.splice(33..33, [0, 0, 0, 0, b'a', b'c', b'T', b'L', 0, 0, 0, 0]);
    assert!(validate_png(&animated, &json!({"width":1,"height":1})).is_err());
}

#[test]
fn private_files_are_exclusive_and_nofollow() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("context");
    let mut f = private_create(&path).unwrap();
    write_private(&mut f, b"{}").unwrap();
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    assert!(private_create(&path).is_err());
    let link = dir.path().join("link");
    symlink(&path, &link).unwrap();
    assert!(private_read(&link).is_err());
    assert!(private_create(&link).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(private_read(&path).is_err());
}

struct Fixture {
    dir: tempfile::TempDir,
    listener: UnixListener,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.path().join("agent.sock");
        let listener = UnixListener::bind(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        Self { dir, listener }
    }
    fn command(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_dwdesktop"));
        c.arg("--socket")
            .arg(self.dir.path().join("agent.sock"))
            .arg("--server-uid")
            .arg(nix::unistd::geteuid().as_raw().to_string());
        c
    }
}

#[test]
fn cli_observe_click_observe_uses_real_wire_and_private_artifacts() {
    let fixture = Fixture::new();
    let listener = fixture.listener.try_clone().unwrap();
    let actions = Arc::new(Mutex::new(Vec::<String>::new()));
    let server_actions = actions.clone();
    let server = thread::spawn(move || {
        let png = png_image(4, 3);
        for _ in 0..6 {
            let (mut s, _) = listener.accept().unwrap();
            s.set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            while let Ok(record) = read_record(&mut s) {
                let req = record.header;
                let kind = req["type"].as_str().unwrap();
                server_actions.lock().unwrap().push(kind.into());
                let (result, payload) = match kind {
                    "session.open" => ("session.opened", json!({"state":"ready"})),
                    "screenshot.request" => (
                        "screenshot.result",
                        json!({"snapshotId":LEASE,"displayId":"display-0","topologyRevision":1,"width":4,"height":3,"format":"png","pixelFormat":"rgba8","cursorEmbedded":false,"captureTimeUs":1,"payloadBytes":png.len()}),
                    ),
                    "control.acquire" => (
                        "control.granted",
                        json!({"leaseId":LEASE,"validForMs":45000}),
                    ),
                    "input.pointer" => {
                        assert_eq!(req["payload"]["x"], 3);
                        assert_eq!(req["payload"]["y"], 2);
                        assert_eq!(req["payload"]["snapshotId"], LEASE);
                        assert_eq!(req["payload"]["topologyRevision"], 1);
                        assert_eq!(req["payload"]["inputSequence"], 1);
                        ("ack", json!({}))
                    }
                    "input.key" => {
                        assert_eq!(req["payload"]["usage"], 4);
                        ("ack", json!({}))
                    }
                    "control.release" | "session.close" => ("ack", json!({})),
                    _ => panic!("unexpected fixture operation"),
                };
                write_record(&mut s, &reply(&req, result, payload)).unwrap();
                if result == "screenshot.result" {
                    s.write_all(&png).unwrap();
                }
            }
        }
    });
    let context = fixture.dir.path().join("session.json");
    let snapshot = fixture.dir.path().join("snapshot.json");
    let image = fixture.dir.path().join("screen.png");
    let run = |cmd: &mut Command| {
        let out = cmd.output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice::<Value>(&out.stdout).unwrap()
    };
    run(fixture.command().arg("open").arg("--context").arg(&context));
    run(fixture
        .command()
        .arg("screenshot")
        .arg("--context")
        .arg(&context)
        .arg("--display")
        .arg("display-0")
        .arg("--output")
        .arg(&image)
        .arg("--snapshot")
        .arg(&snapshot));
    assert_eq!(fs::metadata(&image).unwrap().mode() & 0o777, 0o600);
    run(fixture
        .command()
        .arg("click")
        .arg("--context")
        .arg(&context)
        .arg("--snapshot")
        .arg(&snapshot)
        .args(["--x", "3", "--y", "2"]));
    run(fixture
        .command()
        .arg("screenshot")
        .arg("--context")
        .arg(&context)
        .arg("--display")
        .arg("display-0")
        .arg("--output")
        .arg(fixture.dir.path().join("screen2.png"))
        .arg("--snapshot")
        .arg(fixture.dir.path().join("snapshot2.json")));
    run(fixture
        .command()
        .arg("key")
        .arg("--context")
        .arg(&context)
        .arg("--snapshot")
        .arg(&snapshot)
        .args(["--usage", "4"]));
    run(fixture
        .command()
        .arg("close")
        .arg("--context")
        .arg(&context));
    server.join().unwrap();
    assert_eq!(
        *actions.lock().unwrap(),
        [
            "session.open",
            "screenshot.request",
            "control.acquire",
            "input.pointer",
            "control.release",
            "screenshot.request",
            "control.acquire",
            "input.key",
            "input.key",
            "control.release",
            "session.close"
        ]
    );
}

#[test]
fn wrong_uid_or_unsafe_socket_fails_before_sending() {
    let (peer, _other) = std::os::unix::net::UnixStream::pair().unwrap();
    verify_peer(&peer, nix::unistd::geteuid().as_raw()).unwrap();
    assert!(verify_peer(&peer, nix::unistd::geteuid().as_raw().wrapping_add(1)).is_err());
    let f = Fixture::new();
    let path = f.dir.path().join("agent.sock");
    assert!(Client::connect(&path, nix::unistd::geteuid().as_raw().wrapping_add(1)).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
    assert!(Client::connect(&path, nix::unistd::geteuid().as_raw()).is_err());
}

#[test]
fn remote_secret_messages_are_not_emitted_and_no_retry() {
    let fixture = Fixture::new();
    let listener = fixture.listener.try_clone().unwrap();
    let server = thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let req = read_record(&mut s).unwrap().header;
        write_record(
            &mut s,
            &reply(
                &req,
                "error",
                json!({"code":"forbidden","message":"SECRET-MUST-NOT-LEAK","retryable":true}),
            ),
        )
        .unwrap();
        let mut byte = [0];
        assert_eq!(s.read(&mut byte).unwrap(), 0);
    });
    let out = fixture.command().arg("describe").output().unwrap();
    server.join().unwrap();
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&out.stderr).contains("SECRET-MUST-NOT-LEAK"));
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stderr).unwrap()["code"],
        "forbidden"
    );
}

#[test]
fn wrong_correlation_fails_closed() {
    let f = Fixture::new();
    let listener = f.listener.try_clone().unwrap();
    let server = thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let req = read_record(&mut s).unwrap().header;
        let mut h = reply(
            &req,
            "error",
            json!({"code":"forbidden","message":"redacted","retryable":false}),
        );
        h["requestMessageId"] = json!(LEASE);
        write_record(&mut s, &h).unwrap();
    });
    let result = f.command().arg("describe").output().unwrap();
    server.join().unwrap();
    assert!(!result.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stderr).unwrap()["code"],
        "invalid_record"
    );
}

#[test]
fn deferred_terminal_is_nonzero_and_argument_values_are_redacted() {
    let f = Fixture::new();
    let out = f
        .command()
        .arg("term")
        .args(["--context", "missing"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stderr).unwrap()["code"],
        "not_implemented"
    );
    let out = f
        .command()
        .args(["--secret-argument-value"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!String::from_utf8_lossy(&out.stderr).contains("secret-argument-value"));
}

fn fixture_context(f: &Fixture) -> (std::path::PathBuf, std::path::PathBuf) {
    let context = Context {
        socket: f.dir.path().join("agent.sock"),
        server_uid: nix::unistd::geteuid().as_raw(),
        session_id: SESSION.into(),
        session_epoch: EPOCH.into(),
    };
    let path = f.dir.path().join("session.json");
    write_private(
        &mut private_create(&path).unwrap(),
        &serde_json::to_vec(&context).unwrap(),
    )
    .unwrap();
    let snapshot = f.dir.path().join("snapshot.json");
    let h = reply(
        &request(
            "screenshot.request",
            json!({"displayId":"display-0","includeCursor":false}),
            Some(&context),
        ),
        "screenshot.result",
        json!({"snapshotId":LEASE,"displayId":"display-0","topologyRevision":1,"width":3840,"height":2160,"format":"png","pixelFormat":"rgba8","cursorEmbedded":false,"captureTimeUs":1,"payloadBytes":128}),
    );
    write_private(
        &mut private_create(&snapshot).unwrap(),
        &serde_json::to_vec(&h).unwrap(),
    )
    .unwrap();
    (path, snapshot)
}

#[test]
fn stale_snapshot_error_stops_input_without_retry() {
    let f = Fixture::new();
    let (context, snapshot) = fixture_context(&f);
    let listener = f.listener.try_clone().unwrap();
    let server = thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let acquire = read_record(&mut s).unwrap().header;
        assert_eq!(acquire["type"], "control.acquire");
        write_record(
            &mut s,
            &reply(
                &acquire,
                "control.granted",
                json!({"leaseId":LEASE,"validForMs":45000}),
            ),
        )
        .unwrap();
        let input = read_record(&mut s).unwrap().header;
        assert_eq!(input["type"], "input.pointer");
        assert_eq!(input["payload"]["topologyRevision"], 1);
        write_record(
            &mut s,
            &reply(
                &input,
                "error",
                json!({"code":"stale_topology","message":"safe","retryable":false}),
            ),
        )
        .unwrap();
        let mut b = [0];
        assert_eq!(s.read(&mut b).unwrap(), 0);
    });
    let out = f
        .command()
        .arg("click")
        .arg("--context")
        .arg(context)
        .arg("--snapshot")
        .arg(snapshot)
        .args(["--x", "1", "--y", "2"])
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(!out.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stderr).unwrap()["code"],
        "stale_topology"
    );
}

#[test]
fn resize_rejection_reports_actual_mode_and_releases_control() {
    let f = Fixture::new();
    let (context, snapshot) = fixture_context(&f);
    let listener = f.listener.try_clone().unwrap();
    let server = thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let acquire = read_record(&mut s).unwrap().header;
        write_record(
            &mut s,
            &reply(
                &acquire,
                "control.granted",
                json!({"leaseId":LEASE,"validForMs":45000}),
            ),
        )
        .unwrap();
        let resize = read_record(&mut s).unwrap().header;
        assert_eq!(resize["type"], "display.resize");
        assert_eq!(resize["payload"]["width"], 1920);
        write_record(&mut s,&reply(&resize,"display.resize.result",json!({"displayId":"display-0","status":"rejected","width":3840,"height":2160,"topologyRevision":1,"reason":"unsupported_mode"}))).unwrap();
        let release = read_record(&mut s).unwrap().header;
        assert_eq!(release["type"], "control.release");
        write_record(&mut s, &reply(&release, "ack", json!({}))).unwrap();
    });
    let out = f
        .command()
        .arg("resize")
        .arg("--context")
        .arg(context)
        .arg("--snapshot")
        .arg(snapshot)
        .args(["--width", "1920", "--height", "1080"])
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(!out.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["payload"]["width"],
        3840
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stderr).unwrap()["code"],
        "resize_rejected"
    );
}
