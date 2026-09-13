use clap::Parser;
use dwdesktop_console::{
    MAX_CHUNK, contains_annex_b_start_code, private_create, read_header, read_record,
    validate_root_socket,
};
use serde_json::json;
use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const MAX_PREVIEW_PNG: u64 = 64 * 1024 * 1024;

#[derive(Parser, Debug)]
#[command(
    name = "dwconsole",
    about = "Tap a local DonkeyWork physical-console stream"
)]
struct Args {
    #[arg(long)]
    socket: PathBuf,
    #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u64).range(1..=30))]
    seconds: u64,
    #[arg(long)]
    h264: PathBuf,
    #[arg(long)]
    png: Option<PathBuf>,
    #[arg(long, default_value = "/usr/bin/ffmpeg")]
    ffmpeg: PathBuf,
}

fn main() {
    if let Err(error) = run(Args::parse()) {
        eprintln!("dwconsole: {error}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> io::Result<()> {
    validate_root_socket(&args.socket)?;
    for output in [&args.h264, args.png.as_ref().unwrap_or(&args.h264)] {
        if !output.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "output paths must be absolute",
            ));
        }
    }
    let mut encoded = private_create(&args.h264)?;
    let mut stream = UnixStream::connect(&args.socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    let header = read_header(&mut stream)?;
    let deadline = Instant::now() + Duration::from_secs(args.seconds);
    let mut total = 0_u64;
    let mut prefix = Vec::with_capacity(4096);
    while Instant::now() < deadline {
        let chunk = read_record(&mut stream, MAX_CHUNK)?;
        if prefix.len() < 4096 {
            prefix.extend_from_slice(&chunk[..chunk.len().min(4096 - prefix.len())]);
        }
        encoded.write_all(&chunk)?;
        total = total
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "stream size overflow"))?;
    }
    encoded.flush()?;
    drop(encoded);
    drop(stream);
    if total == 0 || !contains_annex_b_start_code(&prefix) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "no valid H.264 Annex-B stream received",
        ));
    }
    if let Some(path) = &args.png {
        decode_first_frame(&args.ffmpeg, &args.h264, path)?;
    }
    serde_json::to_writer(
        std::io::stdout().lock(),
        &json!({
            "state":"captured", "bytes":total, "seconds":args.seconds, "stream":header,
            "h264":args.h264, "png":args.png
        }),
    )
    .map_err(|_| io::Error::other("cannot write result"))?;
    println!();
    Ok(())
}

fn decode_first_frame(ffmpeg: &Path, input: &Path, output: &Path) -> io::Result<()> {
    if output.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "PNG output already exists",
        ));
    }
    let mut child = Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(input)
        .args([
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-vcodec",
            "png",
            "pipe:1",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let mut png = Vec::new();
    child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("decoder stdout unavailable"))?
        .take(MAX_PREVIEW_PNG + 1)
        .read_to_end(&mut png)?;
    let status = child.wait()?;
    if !status.success()
        || png.len() as u64 > MAX_PREVIEW_PNG
        || !png.starts_with(b"\x89PNG\r\n\x1a\n")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "H.264 preview decode failed",
        ));
    }
    let mut file = private_create(output)?;
    file.write_all(&png)?;
    file.flush()
}
