use dwdesktop_core::{
    backend::Unavailable,
    core::{Core, Policy},
    server::{Server, effective_uid},
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
    let core = Core::new(config.worker_id, config.policies, Box::new(Unavailable))
        .map_err(std::io::Error::other)?;
    let server = Server::bind(&config.socket_path, core)?;
    let (sender, receiver) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = sender.send(true);
    });
    server.run(receiver).await?;
    Ok(())
}
