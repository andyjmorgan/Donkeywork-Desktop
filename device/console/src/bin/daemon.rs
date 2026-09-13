use clap::{Parser, ValueEnum};
use dwdesktop_console::{
    MAX_CHUNK, PROTOCOL, StreamHeader, VERSION, repack_bgr0, write_header, write_record,
};
use libdrmtap::{Config, DrmTap};
use std::{
    fs::Permissions,
    io::{self, Read, Write},
    os::unix::{
        fs::{FileTypeExt, MetadataExt, PermissionsExt},
        io::AsRawFd,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const DRM_FORMAT_XRGB8888: u32 = u32::from_le_bytes(*b"XR24");
const DRM_FORMAT_ARGB8888: u32 = u32::from_le_bytes(*b"AR24");

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Encoder {
    Libx264,
    H264Nvenc,
}
impl Encoder {
    fn name(self) -> &'static str {
        match self {
            Self::Libx264 => "libx264",
            Self::H264Nvenc => "h264_nvenc",
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "dwconsole-daemon",
    about = "Physical Linux console H.264 stream daemon"
)]
struct Args {
    /// Explicit existing physical X11 console; never creates a session.
    #[arg(long, requires = "xauthority")]
    x11_display: Option<String>,
    #[arg(long, requires = "x11_display")]
    xauthority: Option<PathBuf>,
    #[arg(long, default_value_t = 1000)]
    x11_uid: u32,
    #[arg(long, default_value_t = 1000)]
    x11_gid: u32,
    #[arg(long, default_value_t = 1920, value_parser = clap::value_parser!(u32).range(1..=4096))]
    width: u32,
    #[arg(long, default_value_t = 1080, value_parser = clap::value_parser!(u32).range(1..=4096))]
    height: u32,
    #[arg(long)]
    socket: PathBuf,
    #[arg(long)]
    device: PathBuf,
    #[arg(long, default_value_t = 0)]
    crtc: u32,
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u32).range(1..=60))]
    fps: u32,
    #[arg(long, value_enum, default_value = "libx264")]
    encoder: Encoder,
    #[arg(long, default_value = "/usr/bin/ffmpeg")]
    ffmpeg: PathBuf,
}

fn main() {
    if let Err(error) = run(Args::parse()) {
        eprintln!("dwconsole-daemon: {error}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> io::Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "must run as root for the capture POC",
        ));
    }
    validate_device(&args.device)?;
    validate_socket_parent(&args.socket)?;
    validate_executable(&args.ffmpeg)?;
    let listener = UnixListener::bind(&args.socket)?;
    std::fs::set_permissions(&args.socket, Permissions::from_mode(0o600))?;
    let inode = std::fs::symlink_metadata(&args.socket)?.ino();
    let _socket = SocketGuard {
        path: args.socket.clone(),
        inode,
    };
    eprintln!("dwconsole-daemon: ready; local root-only stream socket");
    for accepted in listener.incoming() {
        let stream = match accepted {
            Ok(value) => value,
            Err(error) => {
                eprintln!("dwconsole-daemon: accept failed: {error}");
                continue;
            }
        };
        if peer_uid(&stream)? != 0 {
            eprintln!("dwconsole-daemon: rejected non-root local peer");
            continue;
        }
        if let Err(error) = serve(stream, &args) {
            eprintln!("dwconsole-daemon: stream ended: {error}");
        }
    }
    Ok(())
}

fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    let mut credentials = libc::ucred {
        pid: 0,
        uid: u32::MAX,
        gid: u32::MAX,
    };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut length,
        )
    };
    if result != 0 || length as usize != std::mem::size_of::<libc::ucred>() {
        return Err(io::Error::last_os_error());
    }
    Ok(credentials.uid)
}

fn validate_device(path: &Path) -> io::Result<()> {
    if !path.is_absolute() || !path.starts_with("/dev/dri") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "DRM device must be under /dev/dri",
        ));
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.file_type().is_char_device() || metadata.uid() != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe DRM device",
        ));
    }
    Ok(())
}

fn validate_socket_parent(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "socket requires parent"))?;
    let metadata = std::fs::symlink_metadata(parent)?;
    if !path.is_absolute()
        || !metadata.is_dir()
        || metadata.uid() != 0
        || metadata.mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "socket parent must be root-owned 0700",
        ));
    }
    Ok(())
}

fn validate_executable(path: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !path.is_absolute()
        || !metadata.is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o111 == 0
        || metadata.mode() & 0o022 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe encoder executable",
        ));
    }
    Ok(())
}

fn serve(mut stream: UnixStream, args: &Args) -> io::Result<()> {
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    if args.x11_display.is_some() {
        return serve_x11(stream, args);
    }
    let config = Config {
        device_path: Some(args.device.to_string_lossy().into_owned()),
        crtc_id: args.crtc,
        helper_path: None,
        debug: false,
    };
    let mut capture = DrmTap::open(Some(config)).map_err(other)?;
    let displays = capture.list_displays().map_err(other)?;
    let display = if args.crtc == 0 {
        let mut active = displays.iter().filter(|d| d.active);
        let selected = active
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no active physical display"))?;
        if active.next().is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "multiple active displays require --crtc",
            ));
        }
        selected
    } else {
        displays
            .iter()
            .find(|d| d.active && d.crtc_id == args.crtc)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "configured CRTC is not active")
            })?
    };
    if display.width == 0 || display.height == 0 || display.width > 4096 || display.height > 4096 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported physical display dimensions",
        ));
    }
    let width = display.width;
    let height = display.height;
    let header = StreamHeader {
        protocol: PROTOCOL.into(),
        version: VERSION.into(),
        codec: "h264".into(),
        bitstream: "annex-b".into(),
        encoder: args.encoder.name().into(),
        display_id: format!("crtc-{}", display.crtc_id),
        device: args.device.to_string_lossy().into_owned(),
        width,
        height,
        frames_per_second: args.fps,
        pixel_format: "yuv420p".into(),
    };
    write_header(&mut stream, &header)?;
    let mut encoder = start_encoder(args, width, height)?;
    let mut input = encoder
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("encoder stdin unavailable"))?;
    let mut output = encoder
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("encoder stdout unavailable"))?;
    let stopped = Arc::new(AtomicBool::new(false));
    let output_stopped = stopped.clone();
    let writer = thread::spawn(move || -> io::Result<()> {
        let mut buffer = vec![0_u8; MAX_CHUNK];
        while !output_stopped.load(Ordering::Acquire) {
            let count = output.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            if let Err(error) = write_record(&mut stream, &buffer[..count], MAX_CHUNK) {
                output_stopped.store(true, Ordering::Release);
                return Err(error);
            }
        }
        output_stopped.store(true, Ordering::Release);
        Ok(())
    });
    let interval = Duration::from_nanos(1_000_000_000_u64 / u64::from(args.fps));
    let mut next = Instant::now();
    let capture_result = (|| -> io::Result<()> {
        while !stopped.load(Ordering::Acquire) {
            let frame = capture.grab_mapped().map_err(other)?;
            if frame.width() != width || frame.height() != height {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "display topology changed; reconnect required",
                ));
            }
            if !matches!(frame.format(), DRM_FORMAT_XRGB8888 | DRM_FORMAT_ARGB8888) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unsupported mapped DRM pixel format",
                ));
            }
            let data = frame.data().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "capture returned no mapped pixels",
                )
            })?;
            let packed = repack_bgr0(data, width, height, frame.stride())?;
            if let Err(error) = input.write_all(&packed) {
                stopped.store(true, Ordering::Release);
                return Err(error);
            }
            drop(frame);
            next += interval;
            let now = Instant::now();
            if next > now {
                thread::sleep(next - now);
            } else {
                next = now;
            }
        }
        Ok(())
    })();
    stopped.store(true, Ordering::Release);
    drop(input);
    stop_encoder(&mut encoder);
    let writer_result = writer
        .join()
        .map_err(|_| io::Error::other("stream writer panicked"))?;
    capture_result.and(writer_result)
}

fn start_encoder(args: &Args, width: u32, height: u32) -> io::Result<Child> {
    let mut command = Command::new(&args.ffmpeg);
    if let Some(display) = &args.x11_display {
        use std::os::unix::process::CommandExt;
        if args.x11_uid == 0 || args.x11_gid == 0 || !display.starts_with(':') {
            return Err(io::Error::other(
                "X11 capture requires local display and non-root account",
            ));
        }
        command.env_clear().env("PATH", "/usr/bin:/bin").env(
            "XAUTHORITY",
            args.xauthority
                .as_ref()
                .ok_or_else(|| io::Error::other("missing Xauthority"))?,
        );
        let uid = args.x11_uid;
        let gid = args.x11_gid;
        // The X11 provider uses no worker threads before spawning the child.
        unsafe {
            command.pre_exec(move || {
                if libc::setgroups(0, std::ptr::null()) != 0
                    || libc::setgid(gid) != 0
                    || libc::setuid(uid) != 0
                {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        command.args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "x11grab",
            "-draw_mouse",
            "1",
            "-video_size",
            &format!("{width}x{height}"),
            "-framerate",
            &args.fps.to_string(),
            "-i",
            display,
        ]);
    } else {
        command.args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pixel_format",
            "bgr0",
            "-video_size",
            &format!("{width}x{height}"),
            "-framerate",
            &args.fps.to_string(),
            "-i",
            "pipe:0",
        ]);
    }
    command.args([
        "-an",
        "-bf",
        "0",
        "-g",
        &(args.fps / 2).max(1).to_string(),
        "-keyint_min",
        &(args.fps / 2).max(1).to_string(),
        "-sc_threshold",
        "0",
    ]);
    match args.encoder {
        Encoder::Libx264 => {
            command.args([
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-tune",
                "zerolatency",
                "-crf",
                "18",
            ]);
        }
        Encoder::H264Nvenc => {
            command.args([
                "-c:v",
                "h264_nvenc",
                "-preset",
                "p1",
                "-tune",
                "ull",
                "-rc",
                "vbr",
                "-cq",
                "19",
            ]);
        }
    }
    command
        .args([
            "-pix_fmt",
            "yuv420p",
            "-flush_packets",
            "1",
            "-f",
            "h264",
            "pipe:1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
}

fn serve_x11(mut stream: UnixStream, args: &Args) -> io::Result<()> {
    let header = StreamHeader {
        protocol: PROTOCOL.into(),
        version: VERSION.into(),
        codec: "h264".into(),
        bitstream: "annex-b".into(),
        encoder: args.encoder.name().into(),
        display_id: "x11-console".into(),
        device: format!("x11:{}", args.x11_display.as_deref().unwrap_or_default()),
        width: args.width,
        height: args.height,
        frames_per_second: args.fps,
        pixel_format: "yuv420p".into(),
    };
    let mut child = start_encoder(args, args.width, args.height)?;
    let result = (|| -> io::Result<()> {
        let mut output = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("missing encoder stdout"))?;
        let mut buffer = vec![0_u8; MAX_CHUNK];
        // Only declare a stream after capture/encoding has produced data.
        let first = output.read(&mut buffer)?;
        if first == 0 {
            return Err(io::Error::other("X11 capture produced no video"));
        }
        write_header(&mut stream, &header)?;
        write_record(&mut stream, &buffer[..first], MAX_CHUNK)?;
        loop {
            let count = output.read(&mut buffer)?;
            if count == 0 {
                return Err(io::Error::other("X11 capture ended"));
            }
            write_record(&mut stream, &buffer[..count], MAX_CHUNK)?;
        }
    })();
    stop_encoder(&mut child);
    result
}

fn stop_encoder(child: &mut Child) {
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
}

fn other(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

struct SocketGuard {
    path: PathBuf,
    inode: u64,
}
impl Drop for SocketGuard {
    fn drop(&mut self) {
        if std::fs::symlink_metadata(&self.path).is_ok_and(|m| m.ino() == self.inode) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
