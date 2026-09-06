use dwdesktop_core::{
    backend::Unavailable,
    core::{Core, Policy},
    server::{Server, effective_uid},
    wire,
};
use serde_json::{Value, json};
use std::{os::unix::fs::PermissionsExt, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};
use uuid::Uuid;

async fn start(
    uid: u32,
) -> (
    tempfile::TempDir,
    std::path::PathBuf,
    tokio::sync::watch::Sender<bool>,
    tokio::task::JoinHandle<std::io::Result<()>>,
) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = dir.path().join("cli.sock");
    let core = Core::new(
        Uuid::new_v4(),
        vec![Policy {
            uid,
            profile: "pilot".into(),
            permissions: vec![],
        }],
        Box::new(Unavailable),
    )
    .unwrap();
    let server = Server::bind(&path, core).unwrap();
    let (tx, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(server.run(rx));
    (dir, path, tx, task)
}
#[tokio::test]
async fn real_socket_verifies_uid_and_returns_strict_describe() {
    let (_dir, path, stop, task) = start(effective_uid()).await;
    let mut stream = UnixStream::connect(&path).await.unwrap();
    let request = json!({"protocol":"dwdesktop.local","version":"0.2.0","type":"describe","messageId":Uuid::new_v4(),"payload":{}});
    let bytes = serde_json::to_vec(&request).unwrap();
    stream.write_u32(bytes.len() as u32).await.unwrap();
    stream.write_all(&bytes).await.unwrap();
    let length = stream.read_u32().await.unwrap();
    let mut bytes = vec![0; length as usize];
    stream.read_exact(&mut bytes).await.unwrap();
    let response: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(wire::conforms(&response));
    assert_eq!(response["payload"]["peerUid"], effective_uid());
    assert_eq!(response["requestMessageId"], request["messageId"]);
    drop(stream);
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
    assert!(!path.exists());
}
#[tokio::test]
async fn unmapped_real_peer_is_closed_before_reading_requests() {
    let (_dir, path, stop, task) = start(effective_uid().wrapping_add(1)).await;
    let mut stream = UnixStream::connect(path).await.unwrap();
    let mut byte = [0];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), stream.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    drop(stream);
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
}
#[tokio::test]
async fn malformed_and_oversized_frames_close_without_large_allocation() {
    let (_dir, path, stop, task) = start(effective_uid()).await;
    for bytes in [
        vec![0, 1, 0, 1],
        vec![0, 0, 0, 1, b'{'],
        vec![0, 0, 0, 1, 0xff],
        vec![0, 0, 0, 0],
    ] {
        let mut stream = UnixStream::connect(&path).await.unwrap();
        stream.write_all(&bytes).await.unwrap();
        let mut byte = [0];
        let result = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut byte))
            .await
            .unwrap();
        assert!(matches!(result, Ok(0)) || result.is_err());
    }
    stop.send(true).unwrap();
    task.await.unwrap().unwrap();
}
#[tokio::test]
async fn rejects_writable_socket_directory_and_preserves_existing_target() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
    let core = Core::new(Uuid::new_v4(), vec![], Box::new(Unavailable)).unwrap();
    assert!(Server::bind(&dir.path().join("cli.sock"), core).is_err());
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = dir.path().join("cli.sock");
    std::fs::write(&path, b"preserve").unwrap();
    let core = Core::new(Uuid::new_v4(), vec![], Box::new(Unavailable)).unwrap();
    assert!(Server::bind(&path, core).is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"preserve");
}
