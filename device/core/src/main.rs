use dwdesktop_core::{
    backend::{Backend, Unavailable},
    core::{Core, Policy},
    server::{Server, effective_uid},
    wire,
    x11_adapter::X11Adapter,
};
use serde::Deserialize;
use std::{
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::PathBuf,
};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    socket_path: PathBuf,
    worker_id: Uuid,
    policies: Vec<Policy>,
    #[serde(default)]
    backend: BackendConfig,
}

#[derive(Default, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum BackendConfig {
    #[default]
    Unavailable,
    X11 {
        display: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    if let Err(message) = run().await {
        // Only static stage diagnostics are emitted, never config values or backend details.
        eprintln!("dwdesktop-core: {message}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), &'static str> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 || args[1] != "--config" {
        return Err("usage: dwdesktop-core --config PATH");
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&args[2])
        .map_err(|_| "cannot open configuration file (symlinks are forbidden)")?;
    let metadata = file
        .metadata()
        .map_err(|_| "cannot inspect configuration file")?;
    if !metadata.is_file()
        || metadata.uid() != effective_uid()
        || metadata.mode() & 0o022 != 0
        || metadata.len() > 65_536
    {
        return Err(
            "unsafe configuration: require service-owned regular file, no group/world write, <=64 KiB",
        );
    }
    let mut bytes = Vec::new();
    file.take(65_537)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read configuration file")?;
    if bytes.len() > 65_536 {
        return Err("configuration exceeds 64 KiB");
    }
    let value = wire::parse_json(&bytes)
        .map_err(|_| "invalid configuration JSON (duplicate keys are forbidden)")?;
    let config: Config =
        serde_json::from_value(value).map_err(|_| "invalid configuration fields")?;
    let backend: Box<dyn Backend> = match config.backend {
        BackendConfig::Unavailable => Box::new(Unavailable),
        BackendConfig::X11 { display } => {
            Box::new(X11Adapter::connect(display).map_err(|_| "X11 backend unavailable: verify service DISPLAY, XAUTHORITY and required extensions")?)
        }
    };
    let core = Core::new(config.worker_id, config.policies, backend)
        .map_err(|_| "invalid UID capability policy")?;
    let server = Server::bind(&config.socket_path, core).map_err(|error| match error.kind() {
        std::io::ErrorKind::AddrInUse | std::io::ErrorKind::AlreadyExists => {
            "socket target already exists; inspect it before removing a stale socket"
        }
        std::io::ErrorKind::PermissionDenied => {
            "socket access denied or parent directory ownership/permissions are unsafe"
        }
        _ => "cannot bind local socket; verify parent directory and socket path",
    })?;
    let (sender, receiver) = tokio::sync::watch::channel(false);
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|_| "cannot initialize SIGTERM handler")?;
    tokio::spawn(async move {
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
        let _ = sender.send(true);
    });
    server
        .run(receiver)
        .await
        .map_err(|_| "local socket service failed")?;
    Ok(())
}
