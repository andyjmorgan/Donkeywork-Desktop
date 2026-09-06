use dwdesktop_core::{
    backend::{Backend, Unavailable},
    core::{Core, Policy},
    server::{Server, effective_uid},
    x11_adapter::X11Adapter,
};
use serde::Deserialize;
use std::{os::unix::fs::MetadataExt, path::PathBuf};
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
    if run().await.is_err() {
        eprintln!("dwdesktop-core: configuration or service failure");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 || args[1] != "--config" {
        return Err("usage: dwdesktop-core --config PATH".into());
    }
    let metadata = std::fs::symlink_metadata(&args[2])?;
    if !metadata.is_file()
        || metadata.uid() != effective_uid()
        || metadata.mode() & 0o022 != 0
        || metadata.len() > 65_536
    {
        return Err("unsafe configuration".into());
    }
    let config: Config = serde_json::from_slice(&std::fs::read(&args[2])?)?;
    let backend: Box<dyn Backend> = match config.backend {
        BackendConfig::Unavailable => Box::new(Unavailable),
        BackendConfig::X11 { display } => {
            Box::new(X11Adapter::connect(display).map_err(std::io::Error::other)?)
        }
    };
    let core =
        Core::new(config.worker_id, config.policies, backend).map_err(std::io::Error::other)?;
    let server = Server::bind(&config.socket_path, core)?;
    let (sender, receiver) = tokio::sync::watch::channel(false);
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::spawn(async move {
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
        let _ = sender.send(true);
    });
    server.run(receiver).await?;
    Ok(())
}
