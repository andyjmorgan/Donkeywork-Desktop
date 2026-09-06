//! A local-only X11 stream with absolute deadlines, including reply waits.
use crate::*;
use rustix::event::{poll, PollFd, PollFlags, Timespec};
use std::{
    io::{self, IoSlice},
    sync::Mutex,
    time::{Duration, Instant},
};
use x11rb::{
    rust_connection::{DefaultStream, PollMode, RustConnection, Stream},
    utils::RawFdContainer,
};

pub(crate) struct DeadlineStream {
    inner: DefaultStream,
    deadline: Mutex<Instant>,
}
impl DeadlineStream {
    pub(crate) fn arm(&self, deadline: Instant) {
        *self.deadline.lock().expect("deadline mutex poisoned") = deadline;
    }
    fn remaining(&self) -> io::Result<Duration> {
        self.deadline
            .lock()
            .expect("deadline mutex poisoned")
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::TimedOut, "X11 operation deadline exceeded")
            })
    }
}
impl Stream for DeadlineStream {
    fn poll(&self, mode: PollMode) -> io::Result<()> {
        loop {
            let remaining = self.remaining()?;
            let timeout = Timespec {
                tv_sec: remaining.as_secs().try_into().unwrap_or(i64::MAX),
                tv_nsec: remaining.subsec_nanos().into(),
            };
            let mut flags = PollFlags::empty();
            if mode.readable() {
                flags |= PollFlags::IN;
            }
            if mode.writable() {
                flags |= PollFlags::OUT;
            }
            match poll(&mut [PollFd::new(&self.inner, flags)], Some(&timeout)) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "X11 operation deadline exceeded",
                    ))
                }
                Ok(_) => return Ok(()),
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }
    fn read(&self, bytes: &mut [u8], fds: &mut Vec<RawFdContainer>) -> io::Result<usize> {
        self.remaining()?;
        self.inner.read(bytes, fds)
    }
    fn write(&self, bytes: &[u8], fds: &mut Vec<RawFdContainer>) -> io::Result<usize> {
        self.remaining()?;
        self.inner.write(bytes, fds)
    }
    fn write_vectored(
        &self,
        bytes: &[IoSlice<'_>],
        fds: &mut Vec<RawFdContainer>,
    ) -> io::Result<usize> {
        self.remaining()?;
        self.inner.write_vectored(bytes, fds)
    }
}

pub(crate) fn connect(display: Option<&str>) -> Result<(RustConnection<DeadlineStream>, usize)> {
    let parsed = x11rb_protocol::parse_display::parse_display(display)
        .map_err(|_| BackendError::new(ErrorKind::Unavailable, "invalid configured X11 display"))?;
    let screen = usize::from(parsed.screen);
    for address in parsed.connect_instruction() {
        // M1a captures a local console; do not contact TCP displays or DNS.
        if !matches!(
            address,
            x11rb_protocol::parse_display::ConnectAddress::Socket(_)
        ) {
            continue;
        }
        let Ok((inner, (family, address))) = DefaultStream::connect(&address) else {
            continue;
        };
        let auth =
            x11rb_protocol::xauth::get_auth(family, &address, parsed.display).map_err(|_| {
                BackendError::new(ErrorKind::Unavailable, "cannot read X11 authorization")
            })?;
        let (auth_name, auth_data) = auth.unwrap_or_default();
        let stream = DeadlineStream {
            inner,
            deadline: Mutex::new(Instant::now() + Duration::from_secs(5)),
        };
        return RustConnection::connect_to_stream_with_auth_info(
            stream, screen, auth_name, auth_data,
        )
        .map(|connection| (connection, screen))
        .map_err(|_| {
            BackendError::new(
                ErrorKind::Unavailable,
                "cannot authenticate configured X11 display",
            )
        });
    }
    Err(BackendError::new(
        ErrorKind::Unavailable,
        "cannot connect to local X11 display",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unreadable_socket_times_out_without_x_server() {
        let (reader, _writer) = std::os::unix::net::UnixStream::pair().unwrap();
        let (inner, _) = DefaultStream::from_unix_stream(reader).unwrap();
        let stream = DeadlineStream {
            inner,
            deadline: Mutex::new(Instant::now() + Duration::from_millis(10)),
        };
        assert_eq!(
            stream.poll(PollMode::Readable).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
    }
    #[test]
    fn expired_deadline_blocks_even_ready_writes() {
        let (writer, _reader) = std::os::unix::net::UnixStream::pair().unwrap();
        let (inner, _) = DefaultStream::from_unix_stream(writer).unwrap();
        let stream = DeadlineStream {
            inner,
            deadline: Mutex::new(Instant::now()),
        };
        assert_eq!(
            stream.write(b"x", &mut vec![]).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
    }
}
