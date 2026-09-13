//! Linux uinput pilot. Creation is not evidence of compositor adoption.
use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions},
    io, mem,
    os::fd::AsRawFd,
    os::unix::fs::OpenOptionsExt,
};

const EV_KEY: u16 = 1;
const EV_REL: u16 = 2;
const EV_ABS: u16 = 3;
const EV_REP: u16 = 20;
// Wheel axes only. Advertising REL_X/REL_Y makes Xorg/libinput classify this
// as a relative mouse and discard our ABS_X/ABS_Y position events.
const POINTER_REL_AXES: [u16; 2] = [6, 8];

// Linux generic ioctl ABI, used by the supported x86_64 and aarch64 hosts.
const fn request(direction: u32, number: u32, size: usize) -> libc::c_ulong {
    ((direction << 30) | ((size as u32) << 16) | (85 << 8) | number) as libc::c_ulong
}
const fn setting(number: u32) -> libc::c_ulong {
    request(1, number, 4)
}

#[repr(C)]
struct Setup {
    id: [u16; 4],
    name: [u8; 80],
    effects: u32,
}
#[repr(C)]
struct Axis {
    code: u16,
    padding: u16,
    info: [i32; 6],
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
fn checked(result: libc::c_int) -> io::Result<()> {
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

struct Device {
    file: File,
    created: bool,
    path: String,
}
impl Device {
    fn open() -> io::Result<Self> {
        Ok(Self {
            file: OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
                .open("/dev/uinput")?,
            created: false,
            path: String::new(),
        })
    }
    fn enable(&self, number: u32, value: u16) -> io::Result<()> {
        // These uinput commands take integer values, not integer pointers.
        checked(unsafe {
            libc::ioctl(
                self.file.as_raw_fd(),
                setting(number),
                libc::c_int::from(value),
            )
        })
    }
    fn axis(&self, code: u16, maximum: i32) -> io::Result<()> {
        let axis = Axis {
            code,
            padding: 0,
            info: [0, 0, maximum, 0, 0, 0],
        };
        checked(unsafe {
            libc::ioctl(
                self.file.as_raw_fd(),
                request(1, 4, mem::size_of::<Axis>()),
                &axis,
            )
        })
    }
    fn create(&mut self, name: &str) -> io::Result<()> {
        let mut setup = Setup {
            id: [0x06, 0, 0, 1],
            name: [0; 80],
            effects: 0,
        };
        if name.len() >= setup.name.len() {
            return Err(invalid("input device name too long"));
        }
        setup.name[..name.len()].copy_from_slice(name.as_bytes());
        checked(unsafe {
            libc::ioctl(
                self.file.as_raw_fd(),
                request(1, 3, mem::size_of::<Setup>()),
                &setup,
            )
        })?;
        checked(unsafe { libc::ioctl(self.file.as_raw_fd(), request(0, 1, 0)) })?;
        self.created = true;
        let mut name = [0u8; 128];
        checked(unsafe {
            libc::ioctl(
                self.file.as_raw_fd(),
                request(2, 44, name.len()),
                name.as_mut_ptr(),
            )
        })?;
        let end = name
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| invalid("unterminated uinput sysname"))?;
        let name =
            std::str::from_utf8(&name[..end]).map_err(|_| invalid("invalid uinput sysname"))?;
        if !name.starts_with("input")
            || name.len() <= 5
            || !name[5..].bytes().all(|b| b.is_ascii_digit())
        {
            return Err(invalid("unexpected uinput sysname"));
        }
        self.path = format!("/sys/devices/virtual/input/{name}");
        Ok(())
    }
    fn emit(&self, events: &[(u16, u16, i32)]) -> io::Result<()> {
        let mut packet = Vec::with_capacity(events.len() + 1);
        for &(kind, code, value) in events {
            // Zero timestamp and padding; the kernel supplies event time.
            let mut event: libc::input_event = unsafe { mem::zeroed() };
            event.type_ = kind;
            event.code = code;
            event.value = value;
            packet.push(event);
        }
        packet.push(unsafe { mem::zeroed::<libc::input_event>() }); // SYN_REPORT
        let bytes = packet.len() * mem::size_of::<libc::input_event>();
        loop {
            let written =
                unsafe { libc::write(self.file.as_raw_fd(), packet.as_ptr().cast(), bytes) };
            if written == bytes as isize {
                return Ok(());
            }
            if written >= 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "partial uinput event packet",
                ));
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
}
impl Drop for Device {
    fn drop(&mut self) {
        if self.created {
            unsafe {
                libc::ioctl(self.file.as_raw_fd(), request(0, 2, 0));
            }
        }
    }
}

/// Single-output, unrotated pixel coordinates. Seat/output routing must be
/// validated externally before enabling control; no readiness sleep is used.
pub struct InputDevice {
    keyboard: Device,
    pointer: Device,
    width: u32,
    height: u32,
    keys: BTreeSet<u16>,
    buttons: BTreeSet<u16>,
}
impl InputDevice {
    pub fn new(width: u32, height: u32) -> io::Result<Self> {
        validate_dimensions(width, height)?;
        let mut keyboard = Device::open()?;
        keyboard.enable(100, EV_KEY)?;
        keyboard.enable(100, EV_REP)?;
        for hid in 0..=255 {
            if let Some(code) = hid_key(hid) {
                keyboard.enable(101, code)?;
            }
        }
        keyboard.create("DonkeyWork Console Keyboard")?;
        let mut pointer = Device::open()?;
        pointer.enable(100, EV_KEY)?;
        for code in [272, 273, 274] {
            pointer.enable(101, code)?;
        }
        pointer.enable(100, EV_ABS)?;
        pointer.axis(0, (width - 1) as i32)?;
        pointer.axis(1, (height - 1) as i32)?;
        pointer.enable(100, EV_REL)?;
        for code in POINTER_REL_AXES {
            pointer.enable(102, code)?;
        }
        pointer.create("DonkeyWork Console Pointer")?;
        Ok(Self {
            keyboard,
            pointer,
            width,
            height,
            keys: BTreeSet::new(),
            buttons: BTreeSet::new(),
        })
    }
    pub fn device_paths(&self) -> Vec<String> {
        vec![self.keyboard.path.clone(), self.pointer.path.clone()]
    }
    pub fn move_to(&mut self, x: u32, y: u32) -> io::Result<()> {
        validate_point(self.width, self.height, x, y)?;
        self.pointer
            .emit(&[(EV_ABS, 0, x as i32), (EV_ABS, 1, y as i32)])
    }
    pub fn key(&mut self, hid: u16, down: bool) -> io::Result<()> {
        let code = hid_key(hid).ok_or_else(|| invalid("unsupported keyboard HID usage"))?;
        if self.keys.contains(&code) == down {
            return Ok(());
        }
        // Retain uncertain downs for cleanup even when the write fails.
        if down {
            self.keys.insert(code);
        }
        self.keyboard.emit(&[(EV_KEY, code, i32::from(down))])?;
        if !down {
            self.keys.remove(&code);
        }
        Ok(())
    }
    pub fn button(&mut self, button: u8, down: bool) -> io::Result<()> {
        let code = button_code(button).ok_or_else(|| invalid("unsupported pointer button"))?;
        if self.buttons.contains(&code) == down {
            return Ok(());
        }
        if down {
            self.buttons.insert(code);
        }
        self.pointer.emit(&[(EV_KEY, code, i32::from(down))])?;
        if !down {
            self.buttons.remove(&code);
        }
        Ok(())
    }
    pub fn wheel(&mut self, vertical: i16, horizontal: i16) -> io::Result<()> {
        if vertical == 0 && horizontal == 0 {
            return Err(invalid("empty pointer wheel event"));
        }
        let mut events = Vec::with_capacity(2);
        if vertical != 0 { events.push((EV_REL, 8, i32::from(vertical))); }
        if horizontal != 0 { events.push((EV_REL, 6, i32::from(horizontal))); }
        self.pointer.emit(&events)
    }
    pub fn reset(&mut self) -> io::Result<()> {
        let mut error = None;
        for code in self.keys.clone() {
            match self.keyboard.emit(&[(EV_KEY, code, 0)]) {
                Ok(()) => {
                    self.keys.remove(&code);
                }
                Err(e) => {
                    error.get_or_insert(e);
                }
            }
        }
        for code in self.buttons.clone() {
            match self.pointer.emit(&[(EV_KEY, code, 0)]) {
                Ok(()) => {
                    self.buttons.remove(&code);
                }
                Err(e) => {
                    error.get_or_insert(e);
                }
            }
        }
        error.map_or(Ok(()), Err)
    }
}
impl Drop for InputDevice {
    fn drop(&mut self) {
        let _ = self.reset();
    }
}

fn validate_dimensions(width: u32, height: u32) -> io::Result<()> {
    if !(2..=16384).contains(&width) || !(2..=16384).contains(&height) {
        Err(invalid("unsupported input dimensions"))
    } else {
        Ok(())
    }
}
fn validate_point(width: u32, height: u32, x: u32, y: u32) -> io::Result<()> {
    if x >= width || y >= height {
        Err(invalid("pointer outside display"))
    } else {
        Ok(())
    }
}
fn button_code(button: u8) -> Option<u16> {
    match button {
        1 => Some(272),
        2 => Some(273),
        3 => Some(274),
        _ => None,
    }
}

// USB HID page 7 physical positions -> Linux input-event-codes.h values.
// No character translation, consumer controls, power, PrintScreen/SysRq.
fn hid_key(hid: u16) -> Option<u16> {
    const LETTERS: [u16; 26] = [
        30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17,
        45, 21, 44,
    ];
    match hid {
        4..=29 => Some(LETTERS[(hid - 4) as usize]),
        30..=38 => Some(hid - 28),
        39 => Some(11),
        40 => Some(28),
        41 => Some(1),
        42 => Some(14),
        43 => Some(15),
        44 => Some(57),
        45 => Some(12),
        46 => Some(13),
        47 => Some(26),
        48 => Some(27),
        49 => Some(43),
        50 => Some(43),
        51 => Some(39),
        52 => Some(40),
        53 => Some(41),
        54 => Some(51),
        55 => Some(52),
        56 => Some(53),
        57 => Some(58),
        58..=67 => Some(hid + 1),
        68 => Some(87),
        69 => Some(88),
        71 => Some(70),
        72 => Some(119),
        73 => Some(110),
        74 => Some(102),
        75 => Some(104),
        76 => Some(111),
        77 => Some(107),
        78 => Some(109),
        79 => Some(106),
        80 => Some(105),
        81 => Some(108),
        82 => Some(103),
        83 => Some(69),
        84 => Some(98),
        85 => Some(55),
        86 => Some(74),
        87 => Some(78),
        88 => Some(96),
        89 => Some(79),
        90 => Some(80),
        91 => Some(81),
        92 => Some(75),
        93 => Some(76),
        94 => Some(77),
        95 => Some(71),
        96 => Some(72),
        97 => Some(73),
        98 => Some(82),
        99 => Some(83),
        100 => Some(86),
        101 => Some(127),
        103 => Some(117),
        224 => Some(29),
        225 => Some(42),
        226 => Some(56),
        227 => Some(125),
        228 => Some(97),
        229 => Some(54),
        230 => Some(100),
        231 => Some(126),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_pointer_never_advertises_relative_position_axes() {
        assert_eq!(POINTER_REL_AXES, [6, 8]);
        assert!(!POINTER_REL_AXES.contains(&0));
        assert!(!POINTER_REL_AXES.contains(&1));
    }
    #[test]
    fn mappings_and_rejections() {
        assert_eq!(hid_key(4), Some(30));
        assert_eq!(hid_key(29), Some(44));
        assert_eq!(hid_key(30), Some(2));
        assert_eq!(hid_key(39), Some(11));
        assert_eq!(hid_key(225), Some(42));
        assert_eq!(hid_key(229), Some(54));
        assert_eq!(hid_key(230), Some(100));
        for hid in [0, 1, 2, 3, 70, 102, 232, 65535] {
            assert_eq!(hid_key(hid), None);
        }
        for hid in 0..=65535 {
            assert!(!matches!(hid_key(hid), Some(99 | 116 | 142 | 143)));
        }
        assert_eq!(button_code(3), Some(274));
        assert_eq!(button_code(0), None);
        assert_eq!(button_code(4), None);
    }
    #[test]
    fn dimensions_and_points() {
        assert!(validate_dimensions(1920, 1080).is_ok());
        assert!(validate_dimensions(0, 1080).is_err());
        assert!(validate_dimensions(u32::MAX, 1080).is_err());
        assert!(validate_point(1920, 1080, 1919, 1079).is_ok());
        assert!(validate_point(1920, 1080, 1920, 0).is_err());
        assert!(validate_point(1920, 1080, 0, 1080).is_err());
    }
    #[test]
    fn kernel_abi() {
        assert_eq!(mem::size_of::<Setup>(), 92);
        assert_eq!(mem::size_of::<Axis>(), 28);
        assert_eq!(setting(100), 0x40045564);
    }
}
