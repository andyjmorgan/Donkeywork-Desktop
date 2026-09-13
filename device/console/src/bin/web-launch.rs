//! Connect the private capture socket before replacing this process with an
//! unprivileged browser bridge. The connected stream becomes standard input.
use clap::Parser;
use dwdesktop_console::validate_root_socket;
use std::{
    io,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{fs::MetadataExt, net::UnixStream, process::CommandExt},
    },
    path::PathBuf,
    process::{Command, Stdio},
};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    socket: PathBuf,
    /// Optional root-private input connection, inherited by bridge as fd 3.
    #[arg(long)]
    input_socket: Option<PathBuf>,
    /// Narrow capture reconnect broker, inherited as fd 4.
    #[arg(long)]
    capture_broker_socket: Option<PathBuf>,
    #[arg(long)]
    executable: PathBuf,
    #[arg(long)]
    uid: u32,
    #[arg(long)]
    gid: u32,
    #[arg(last = true)]
    bridge_args: Vec<String>,
}

fn main() {
    if let Err(error) = run(Args::parse()) {
        eprintln!("dwconsole-web-launch: {error}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> io::Result<()> {
    if unsafe { libc::geteuid() } != 0 || args.uid == 0 || args.gid == 0 {
        return Err(io::Error::other(
            "root launcher requires a non-root target UID and GID",
        ));
    }
    validate_root_socket(&args.socket)?;
    let executable = std::fs::symlink_metadata(&args.executable)?;
    if !args.executable.is_absolute()
        || !executable.is_file()
        || executable.uid() != 0
        || executable.mode() & 0o022 != 0
        || executable.mode() & 0o111 == 0
    {
        return Err(io::Error::other(
            "bridge executable must be root-owned and not writable by others",
        ));
    }
    let stream = UnixStream::connect(&args.socket)?;
    let input = if let Some(path) = &args.input_socket {
        validate_root_socket(path)?;
        let connection = UnixStream::connect(path)?;
        dwdesktop_console::input::peer_root(&connection)?;
        Some(connection)
    } else {
        None
    };
    let broker = if let Some(path) = &args.capture_broker_socket {
        validate_root_socket(path)?;
        let connection = UnixStream::connect(path)?;
        dwdesktop_console::input::peer_root(&connection)?;
        Some(connection)
    } else {
        None
    };
    // Move inherited sources above all destination descriptors before stdio
    // setup, avoiding aliasing when optional fd3/fd4 connections are absent.
    let mut inherited = Vec::new();
    for (connection, target) in [(input, 3), (broker, 4)] {
        if let Some(connection) = connection {
            let raw = unsafe { libc::fcntl(connection.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 10) };
            if raw < 0 {
                return Err(io::Error::last_os_error());
            }
            inherited.push((unsafe { OwnedFd::from_raw_fd(raw) }, target));
        }
    }
    let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
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
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    if length as usize != std::mem::size_of::<libc::ucred>() || credentials.uid != 0 {
        return Err(io::Error::other("capture peer is not root"));
    }
    // No threads exist here. Drop all privilege before parsing any browser data.
    if unsafe { libc::setgroups(0, std::ptr::null()) } != 0
        || unsafe { libc::setgid(args.gid) } != 0
        || unsafe { libc::setuid(args.uid) } != 0
        || unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
    {
        return Err(io::Error::last_os_error());
    }
    let fd: OwnedFd = stream.into();
    let mut command = Command::new(args.executable);
    command
        .args(args.bridge_args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .current_dir("/")
        .stdin(Stdio::from(fd));
    unsafe {
        command.pre_exec(move || {
            for (source, target) in &inherited {
                if libc::dup2(source.as_raw_fd(), *target) < 0 {
                    return Err(io::Error::last_os_error());
                }
                if libc::fcntl(*target, libc::F_SETFD, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    Err(command.exec())
}
