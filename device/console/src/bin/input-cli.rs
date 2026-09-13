use clap::{Parser, Subcommand};
use dwdesktop_console::{input::*, read_record, validate_root_socket, write_record};
use std::{
    io,
    os::unix::net::UnixStream,
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    socket: PathBuf,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Move {
        x: u32,
        y: u32,
    },
    Click {
        x: u32,
        y: u32,
        #[arg(long, default_value_t = 1)]
        button: u8,
    },
    Key {
        hid: u16,
        #[arg(long,default_value_t=50,value_parser=clap::value_parser!(u64).range(1..=5000))]
        hold_ms: u64,
    },
    HoldButton {
        x: u32,
        y: u32,
        #[arg(long, default_value_t = 1)]
        button: u8,
        #[arg(long,default_value_t=1000,value_parser=clap::value_parser!(u64).range(1..=5000))]
        hold_ms: u64,
    },
    Reset,
}
struct Client {
    stream: UnixStream,
    generation: String,
    sequence: u64,
}
impl Client {
    fn send(&mut self, event: Event) -> io::Result<()> {
        self.sequence += 1;
        let r = Request {
            generation: self.generation.clone(),
            sequence: self.sequence,
            event,
        };
        write_record(&mut self.stream, &serde_json::to_vec(&r)?, MAX_INPUT)?;
        let ack: Ack = serde_json::from_slice(&read_record(&mut self.stream, MAX_INPUT)?)
            .map_err(|_| invalid())?;
        if ack.kind != "ack" || !ack.accepted || ack.sequence != self.sequence {
            return Err(invalid());
        }
        Ok(())
    }
    fn hold(&mut self, ms: u64) -> io::Result<()> {
        let end = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < end {
            std::thread::sleep(
                end.saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(250)),
            );
            self.send(Event::Renew {})?;
        }
        Ok(())
    }
}
fn main() -> io::Result<()> {
    let args = Args::parse();
    validate_root_socket(&args.socket)?;
    let mut stream = UnixStream::connect(&args.socket)?;
    peer_root(&stream)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    write_record(
        &mut stream,
        &serde_json::to_vec(&Acquire::Acquire { request_id: None })?,
        MAX_INPUT,
    )?;
    let hello: Hello =
        serde_json::from_slice(&read_record(&mut stream, MAX_INPUT)?).map_err(|_| invalid())?;
    if hello.kind != "ready"
        || hello.protocol != "dwconsole.input"
        || hello.version != "0.2.0"
        || hello.generation.len() != 32
        || hello.lease_ms != LEASE_MS
    {
        return Err(invalid());
    }
    let mut client = Client {
        stream,
        generation: hello.generation,
        sequence: 0,
    };
    match args.command {
        Command::Move { x, y } => client.send(Event::Move { x, y })?,
        Command::Click { x, y, button } => {
            client.send(Event::Button {
                x,
                y,
                button,
                down: true,
            })?;
            client.send(Event::Button {
                x,
                y,
                button,
                down: false,
            })?;
        }
        Command::Key { hid, hold_ms } => {
            client.send(Event::Key { hid, down: true })?;
            client.hold(hold_ms)?;
            client.send(Event::Key { hid, down: false })?;
        }
        Command::HoldButton {
            x,
            y,
            button,
            hold_ms,
        } => {
            client.send(Event::Button {
                x,
                y,
                button,
                down: true,
            })?;
            client.hold(hold_ms)?;
            client.send(Event::Button {
                x,
                y,
                button,
                down: false,
            })?;
        }
        Command::Reset => (),
    }
    client.send(Event::Release {})?;
    println!("accepted; application response requires visual verification");
    Ok(())
}
