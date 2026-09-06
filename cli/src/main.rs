use clap::{Parser, Subcommand};
use donkeywork_desktop_cli::{
    private_create, private_read, snapshot, write_private, Client, Context, Failure, Result,
};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[command(
    name = "dwdesktop",
    about = "Local authenticated desktop CLI (protocol 0.2.0)",
    disable_help_subcommand = true
)]
struct Args {
    /// Dedicated local daemon socket; never the broker IPC socket.
    #[arg(long)]
    socket: PathBuf,
    /// Expected kernel UID of the service, supplied explicitly.
    #[arg(long)]
    server_uid: u32,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Describe,
    /// Create session context privately; never overwrite an existing file.
    Open {
        #[arg(long)]
        context: PathBuf,
    },
    Close {
        #[arg(long)]
        context: PathBuf,
    },
    Screenshot {
        #[arg(long)]
        context: PathBuf,
        #[arg(long)]
        display: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        snapshot: PathBuf,
        #[arg(long)]
        include_cursor: bool,
    },
    Click {
        #[arg(long)]
        context: PathBuf,
        #[arg(long)]
        snapshot: PathBuf,
        #[arg(long)]
        x: u32,
        #[arg(long)]
        y: u32,
        #[arg(long, default_value="left", value_parser=["left","middle","right"])]
        button: String,
    },
    /// Send a USB HID page-7 key down/up pair within one control lease.
    Key {
        #[arg(long)]
        context: PathBuf,
        #[arg(long)]
        snapshot: PathBuf,
        #[arg(long, value_parser=clap::value_parser!(u16).range(1..=255))]
        usage: u16,
    },
    /// Read Unicode text only from stdin, never from command-line arguments.
    Text {
        #[arg(long)]
        context: PathBuf,
        #[arg(long)]
        snapshot: PathBuf,
    },
    Resize {
        #[arg(long)]
        context: PathBuf,
        #[arg(long)]
        snapshot: PathBuf,
        #[arg(long, value_parser=clap::value_parser!(u32).range(1..=4096))]
        width: u32,
        #[arg(long, value_parser=clap::value_parser!(u32).range(1..=4096))]
        height: u32,
    },
    /// Acquire/release control, optionally reset held input. No persistent lease.
    Control {
        #[arg(long)]
        context: PathBuf,
        #[arg(long)]
        reset: bool,
    },
    /// Reserved; interactive PTY is explicitly not implemented in this slice.
    Term {
        #[arg(long)]
        context: PathBuf,
    },
}

fn load_context(path: &Path, args: &Args) -> Result<Context> {
    let context: Context = serde_json::from_slice(&private_read(path)?)
        .map_err(|_| Failure::new("invalid_context", "Context file failed validation."))?;
    context.check_endpoint(&args.socket, args.server_uid)?;
    Ok(context)
}
fn output(value: &Value) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, value)
        .map_err(|_| Failure::new("output_failed", "Cannot write structured result."))?;
    stdout.write_all(b"\n")?;
    Ok(())
}
fn input_payload(snapshot: &Value, lease: &str, sequence: u64) -> Value {
    json!({"leaseId":lease,"snapshotId":snapshot["snapshotId"],"displayId":snapshot["displayId"],
        "topologyRevision":snapshot["topologyRevision"],"inputSequence":sequence})
}

fn run(args: &Args) -> Result<()> {
    if !args.socket.is_absolute() {
        return Err(Failure::new(
            "invalid_argument",
            "Socket path must be absolute.",
        ));
    }
    if matches!(args.command, Command::Term { .. }) {
        return Err(Failure::new(
            "not_implemented",
            "Interactive PTY is deferred; this command does not contact the daemon.",
        ));
    }
    let mut client = Client::connect(&args.socket, args.server_uid)?;
    match &args.command {
        Command::Describe => {
            let reply = client.call("describe", json!({}), None, "describe.result")?;
            if reply.header["payload"]["peerUid"] != nix::unistd::geteuid().as_raw() {
                return Err(Failure::new(
                    "invalid_record",
                    "Daemon reported an inconsistent client UID.",
                ));
            }
            output(&reply.header)
        }
        Command::Open { context: path } => {
            // Reserve before creating daemon resources: failure never replaces prior context.
            let mut file = private_create(path)?;
            let reply = client.call("session.open", json!({}), None, "session.opened")?;
            let context = Context {
                socket: args.socket.clone(),
                server_uid: args.server_uid,
                session_id: reply.header["sessionId"].as_str().unwrap().into(),
                session_epoch: reply.header["sessionEpoch"].as_str().unwrap().into(),
            };
            let bytes = serde_json::to_vec(&context)
                .map_err(|_| Failure::new("invalid_context", "Cannot encode session context."))?;
            if let Err(error) = write_private(&mut file, &bytes) {
                let _ = client.call("session.close", json!({}), Some(&context), "ack");
                return Err(error);
            }
            output(&reply.header)
        }
        Command::Close { context } => {
            let c = load_context(context, args)?;
            output(
                &client
                    .call("session.close", json!({}), Some(&c), "ack")?
                    .header,
            )
        }
        Command::Screenshot {
            context,
            display,
            output: image_path,
            snapshot: snapshot_path,
            include_cursor,
        } => {
            let c = load_context(context, args)?;
            // Both artifacts are exclusive 0600 files. Failed operations can leave
            // empty/partial reserved files; they are never accepted as metadata.
            let mut image_file = private_create(image_path)?;
            let mut snapshot_file = private_create(snapshot_path)?;
            let reply = client.call(
                "screenshot.request",
                json!({"displayId":display,"includeCursor":include_cursor}),
                Some(&c),
                "screenshot.result",
            )?;
            if reply.header["payload"]["displayId"] != *display
                || reply.header["payload"]["cursorEmbedded"] != *include_cursor
            {
                return Err(Failure::new(
                    "invalid_record",
                    "Screenshot does not honor the requested display or cursor policy.",
                ));
            }
            write_private(&mut image_file, &reply.binary)?;
            let metadata = serde_json::to_vec(&reply.header).map_err(|_| {
                Failure::new("invalid_record", "Cannot encode screenshot metadata.")
            })?;
            write_private(&mut snapshot_file, &metadata)?;
            output(&reply.header)
        }
        Command::Click {
            context,
            snapshot: path,
            x,
            y,
            button,
        } => {
            let c = load_context(context, args)?;
            let s = snapshot(path, &c)?;
            if *x as u64 >= s["width"].as_u64().unwrap()
                || *y as u64 >= s["height"].as_u64().unwrap()
            {
                return Err(Failure::new(
                    "invalid_argument",
                    "Pointer coordinates are outside the observed native raster.",
                ));
            }
            let reply = client.controlled(&c, |client, lease| {
                let mut p = input_payload(&s, lease, 1);
                p["x"] = json!(x);
                p["y"] = json!(y);
                p["action"] = json!("click");
                p["button"] = json!(button);
                client.call("input.pointer", p, Some(&c), "ack")
            })?;
            output(&reply.header)
        }
        Command::Key {
            context,
            snapshot: path,
            usage,
        } => {
            let c = load_context(context, args)?;
            let s = snapshot(path, &c)?;
            let reply = client.controlled(&c, |client, lease| {
                let mut p = input_payload(&s, lease, 1);
                p["usage"] = json!(usage);
                p["action"] = json!("down");
                client.call("input.key", p.clone(), Some(&c), "ack")?;
                p["inputSequence"] = json!(2);
                p["action"] = json!("up");
                client.call("input.key", p, Some(&c), "ack")
            })?;
            output(&reply.header)
        }
        Command::Text {
            context,
            snapshot: path,
        } => {
            let c = load_context(context, args)?;
            let s = snapshot(path, &c)?;
            let mut bytes = Vec::new();
            std::io::stdin().take(65_537).read_to_end(&mut bytes)?;
            if bytes.len() > 65_536 {
                return Err(Failure::new(
                    "invalid_argument",
                    "Input text exceeds the bounded command size.",
                ));
            }
            let text = String::from_utf8(bytes)
                .map_err(|_| Failure::new("invalid_argument", "Input text must be UTF-8."))?;
            let reply = client.controlled(&c, |client, lease| {
                let mut p = input_payload(&s, lease, 1);
                p["text"] = json!(text);
                client.call("input.text", p, Some(&c), "ack")
            })?;
            output(&reply.header)
        }
        Command::Resize {
            context,
            snapshot: path,
            width,
            height,
        } => {
            let c = load_context(context, args)?;
            let s = snapshot(path, &c)?;
            let reply=client.controlled(&c,|client,lease| client.call("display.resize",json!({
                "leaseId":lease,"displayId":s["displayId"],"topologyRevision":s["topologyRevision"],"width":width,"height":height}),Some(&c),"display.resize.result"))?;
            let p = &reply.header["payload"];
            if p["displayId"] != s["displayId"] {
                return Err(Failure::new(
                    "invalid_record",
                    "Resize response names a different display.",
                ));
            }
            if p["status"] == "applied"
                && (p["width"] != *width
                    || p["height"] != *height
                    || p["reason"] != "none"
                    || p["topologyRevision"].as_u64() <= s["topologyRevision"].as_u64())
            {
                return Err(Failure::new(
                    "invalid_record",
                    "Applied resize response has inconsistent geometry.",
                ));
            }
            output(&reply.header)?;
            if p["status"] == "rejected" {
                return Err(Failure::new(
                    "resize_rejected",
                    "Display mode was not applied; structured result reports the actual state.",
                ));
            }
            Ok(())
        }
        Command::Control { context, reset } => {
            let c = load_context(context, args)?;
            client.controlled(&c, |client, lease| {
                if *reset {
                    client.call(
                        "input.reset",
                        json!({"leaseId":lease,"inputSequence":1}),
                        Some(&c),
                        "ack",
                    )?;
                }
                Ok(())
            })?;
            output(&json!({"ok":true,"controlReleased":true}))
        }
        Command::Term { .. } => unreachable!(),
    }
}
fn main() {
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(e)
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            let _ = e.print();
            return;
        }
        Err(_) => {
            eprintln!(
                "{}",
                json!({"code":"invalid_arguments","message":"Invalid arguments. Use --help; argument values are not echoed.","retryable":false})
            );
            std::process::exit(2);
        }
    };
    if let Err(error) = run(&args) {
        eprintln!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }
}
