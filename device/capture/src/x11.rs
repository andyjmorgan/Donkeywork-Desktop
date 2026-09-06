// Implementation follows the X11/RandR/XTEST protocol, not upstream application code.
use crate::{
    image::{rgba_len, x11_to_rgba, PixelFormat},
    *,
};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};
use x11rb::{
    connection::Connection,
    protocol::{
        randr::{self, Connection as OutputConnection, ConnectionExt as _, Rotation},
        xkb,
        xproto::{self, ConnectionExt as _, ImageFormat, ImageOrder, VisualClass},
        xtest::{self, ConnectionExt as _},
    },
    rust_connection::RustConnection,
};

pub struct X11Backend {
    pub(crate) connection: RustConnection<crate::transport::DeadlineStream>,
    pub(crate) root: u32,
    pub(crate) limits: FrameLimits,
    screen: usize,
    pub(crate) topology: Topology,
    pub(crate) config_timestamp: u32,
    pub(crate) randr13: bool,
    input_supported: bool,
    pub(crate) held_keys: BTreeSet<u8>,
    held_buttons: BTreeSet<u8>,
    input_context: Option<(u64, String)>,
}

impl X11Backend {
    pub fn connect(options: BackendOptions) -> Result<Self> {
        let (connection, screen) = crate::transport::connect(options.display.as_deref())?;
        let root = connection.setup().roots[screen].root;
        let version = connection
            .randr_query_version(1, 3)
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        if (version.major_version, version.minor_version) < (1, 2) {
            return Err(BackendError::new(
                ErrorKind::Unsupported,
                "RandR 1.2 or newer is required",
            ));
        }
        let input_supported = xtest::get_version(&connection, 2, 2)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some()
            && xkb::use_extension(&connection, 1, 0)
                .ok()
                .and_then(|c| c.reply().ok())
                .is_some_and(|r| r.supported);
        connection
            .randr_select_input(
                root,
                randr::NotifyMask::SCREEN_CHANGE
                    | randr::NotifyMask::CRTC_CHANGE
                    | randr::NotifyMask::OUTPUT_CHANGE,
            )
            .map_err(BackendError::os)?
            .check()
            .map_err(BackendError::os)?;
        let mut backend = Self {
            connection,
            root,
            screen,
            limits: options.limits,
            topology: Topology {
                revision: 0,
                displays: Vec::new(),
            },
            config_timestamp: 0,
            randr13: (version.major_version, version.minor_version) >= (1, 3),
            input_supported,
            held_keys: BTreeSet::new(),
            held_buttons: BTreeSet::new(),
            input_context: None,
        };
        backend.topology()?;
        Ok(backend)
    }

    pub fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            capture: true,
            input: self.input_supported,
            cursor_composition: false,
        }
    }

    pub fn topology(&mut self) -> Result<Topology> {
        self.connection
            .stream()
            .arm(Instant::now() + Duration::from_secs(5));
        self.refresh_topology()
    }

    pub(crate) fn refresh_topology(&mut self) -> Result<Topology> {
        // Drain notifications so a change away and back invalidates observations too.
        let mut changed = false;
        let mut events = 0usize;
        while let Some(event) = self.connection.poll_for_event().map_err(BackendError::os)? {
            events += 1;
            if events > 1024 {
                return Err(BackendError::new(
                    ErrorKind::ResourceExhausted,
                    "excessive X11 event backlog",
                ));
            }
            if matches!(
                event,
                x11rb::protocol::Event::RandrScreenChangeNotify(_)
                    | x11rb::protocol::Event::RandrNotify(_)
            ) {
                changed = true;
            }
        }
        let resources = self
            .connection
            .randr_get_screen_resources(self.root)
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        let primary = if self.randr13 {
            self.connection
                .randr_get_output_primary(self.root)
                .map_err(BackendError::os)?
                .reply()
                .map_err(BackendError::os)?
                .output
        } else {
            0
        };
        let mut displays = Vec::new();
        for output in &resources.outputs {
            let info = self
                .connection
                .randr_get_output_info(*output, resources.config_timestamp)
                .map_err(BackendError::os)?
                .reply()
                .map_err(BackendError::os)?;
            if info.status != randr::SetConfig::SUCCESS {
                return Err(BackendError::new(
                    ErrorKind::StaleTopology,
                    "RandR output changed during enumeration",
                ));
            }
            if info.connection != OutputConnection::CONNECTED || info.crtc == 0 {
                continue;
            }
            let crtc = self
                .connection
                .randr_get_crtc_info(info.crtc, resources.config_timestamp)
                .map_err(BackendError::os)?
                .reply()
                .map_err(BackendError::os)?;
            if crtc.status != randr::SetConfig::SUCCESS {
                return Err(BackendError::new(
                    ErrorKind::StaleTopology,
                    "RandR CRTC changed during enumeration",
                ));
            }
            if crtc.mode == 0 || crtc.width == 0 || crtc.height == 0 {
                continue;
            }
            let mut modes = Vec::new();
            let simple = crtc.rotation == Rotation::ROTATE0
                && crtc.x == 0
                && crtc.y == 0
                && crtc.outputs == [*output]
                && self.simple_transform(info.crtc)?;
            for mode in &resources.modes {
                if info.modes.contains(&mode.id) {
                    let resolution = if crtc.rotation.contains(Rotation::ROTATE90)
                        || crtc.rotation.contains(Rotation::ROTATE270)
                    {
                        Resolution {
                            width: mode.height.into(),
                            height: mode.width.into(),
                        }
                    } else {
                        Resolution {
                            width: mode.width.into(),
                            height: mode.height.into(),
                        }
                    };
                    if !modes.contains(&resolution) {
                        modes.push(resolution);
                    }
                }
            }
            displays.push(Display {
                id: output.to_string(),
                x: crtc.x.into(),
                y: crtc.y.into(),
                width: crtc.width.into(),
                height: crtc.height.into(),
                primary: *output == primary,
                can_resize: simple,
                modes,
            });
        }
        displays.sort_by(|a, b| a.id.cmp(&b.id));
        if displays.len() != 1 {
            for display in &mut displays {
                display.can_resize = false;
            }
        }
        if changed
            || resources.config_timestamp != self.config_timestamp
            || displays != self.topology.displays
            || self.topology.revision == 0
        {
            self.topology.revision = self
                .topology
                .revision
                .checked_add(1)
                .filter(|r| *r < (1u64 << 53))
                .ok_or_else(|| {
                    BackendError::new(ErrorKind::ResourceExhausted, "topology revision exhausted")
                })?;
        }
        self.config_timestamp = resources.config_timestamp;
        self.topology.displays = displays;
        Ok(self.topology.clone())
    }

    pub(crate) fn checked_display(&mut self, revision: u64, id: &str) -> Result<Display> {
        let topology = self.refresh_topology()?;
        if revision != topology.revision {
            return Err(BackendError::new(
                ErrorKind::StaleTopology,
                "display topology has changed",
            ));
        }
        topology
            .displays
            .into_iter()
            .find(|d| d.id == id)
            .ok_or_else(|| BackendError::new(ErrorKind::NotFound, "display is not active"))
    }

    pub(crate) fn simple_transform(&self, crtc: u32) -> Result<bool> {
        if !self.randr13 {
            return Ok(true);
        }
        let reply = self
            .connection
            .randr_get_crtc_transform(crtc)
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        let transform = reply.current_transform;
        let identity = transform.matrix11 == 65536
            && transform.matrix22 == 65536
            && transform.matrix33 == 65536
            && [
                transform.matrix12,
                transform.matrix13,
                transform.matrix21,
                transform.matrix23,
                transform.matrix31,
                transform.matrix32,
            ]
            .iter()
            .all(|v| *v == 0);
        let panning = self
            .connection
            .randr_get_panning(crtc)
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        Ok(identity && panning.width == 0 && panning.height == 0)
    }

    pub fn capture_next(
        &mut self,
        display_id: &str,
        include_cursor: bool,
        deadline: Instant,
    ) -> Result<NativeFrame> {
        if include_cursor {
            return Err(BackendError::new(
                ErrorKind::Unsupported,
                "cursor composition is not implemented",
            ));
        }
        check_deadline(deadline)?;
        self.connection
            .stream()
            .arm(deadline.min(Instant::now() + Duration::from_secs(5)));
        let topology = self.refresh_topology()?;
        let display = topology
            .displays
            .iter()
            .find(|d| d.id == display_id)
            .ok_or_else(|| BackendError::new(ErrorKind::NotFound, "display is not active"))?;
        rgba_len(display.width, display.height, self.limits)?;
        let output = display_id
            .parse::<u32>()
            .map_err(|_| BackendError::new(ErrorKind::NotFound, "invalid output identity"))?;
        let info = self
            .connection
            .randr_get_output_info(output, self.config_timestamp)
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        if info.status != randr::SetConfig::SUCCESS || info.crtc == 0 {
            return Err(BackendError::new(
                ErrorKind::StaleTopology,
                "output changed before capture",
            ));
        }
        if !self.simple_transform(info.crtc)? {
            return Err(BackendError::new(
                ErrorKind::Unsupported,
                "transformed or panning displays are unsupported",
            ));
        }
        let setup = self.connection.setup();
        let screen = &setup.roots[self.screen];
        let visual = screen
            .allowed_depths
            .iter()
            .flat_map(|d| &d.visuals)
            .find(|v| v.visual_id == screen.root_visual)
            .ok_or_else(|| BackendError::new(ErrorKind::Unsupported, "root visual is unknown"))?;
        if visual.class != VisualClass::TRUE_COLOR {
            return Err(BackendError::new(
                ErrorKind::Unsupported,
                "capture requires a TrueColor root visual",
            ));
        }
        let format = setup
            .pixmap_formats
            .iter()
            .find(|f| f.depth == screen.root_depth)
            .ok_or_else(|| {
                BackendError::new(ErrorKind::Unsupported, "root pixmap format is unknown")
            })?;
        if ![16, 24, 32].contains(&format.bits_per_pixel)
            || ![8, 16, 32].contains(&format.scanline_pad)
        {
            return Err(BackendError::new(
                ErrorKind::Unsupported,
                "unsupported root pixel packing",
            ));
        }
        let packing = PixelFormat {
            bits_per_pixel: format.bits_per_pixel,
            scanline_pad: format.scanline_pad,
            little_endian: setup.image_byte_order == ImageOrder::LSB_FIRST,
            red: visual.red_mask,
            green: visual.green_mask,
            blue: visual.blue_mask,
        };
        let captured_at = Instant::now();
        check_deadline(deadline)?;
        let reply = self
            .connection
            .get_image(
                ImageFormat::Z_PIXMAP,
                self.root,
                i16::try_from(display.x).map_err(|_| {
                    BackendError::new(
                        ErrorKind::Unsupported,
                        "display origin outside X11 capture range",
                    )
                })?,
                i16::try_from(display.y).map_err(|_| {
                    BackendError::new(
                        ErrorKind::Unsupported,
                        "display origin outside X11 capture range",
                    )
                })?,
                display.width as u16,
                display.height as u16,
                u32::MAX,
            )
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        if reply.depth != screen.root_depth || reply.visual != screen.root_visual {
            return Err(BackendError::new(
                ErrorKind::CaptureFailed,
                "capture visual changed",
            ));
        }
        let rgba = x11_to_rgba(
            &reply.data,
            display.width,
            display.height,
            packing,
            self.limits,
        )?;
        let width = display.width;
        let height = display.height;
        let after = self.refresh_topology()?;
        if topology != after {
            return Err(BackendError::new(
                ErrorKind::StaleTopology,
                "display changed during capture",
            ));
        }
        check_deadline(deadline)?;
        Ok(NativeFrame {
            display_id: display_id.into(),
            topology_revision: topology.revision,
            captured_at,
            width,
            height,
            cursor_embedded: false,
            rgba,
        })
    }

    pub fn inject(
        &mut self,
        expected_revision: u64,
        display_id: &str,
        action: InputAction,
        guard: &mut dyn ActionGuard,
    ) -> Result<()> {
        self.connection
            .stream()
            .arm(Instant::now() + Duration::from_secs(5));
        self.input_context = Some((expected_revision, display_id.into()));
        let result = self.inject_inner(expected_revision, display_id, action, guard);
        self.input_context = None;
        if result.is_err() {
            self.release_all()?;
        }
        result
    }

    fn inject_inner(
        &mut self,
        revision: u64,
        display_id: &str,
        action: InputAction,
        guard: &mut dyn ActionGuard,
    ) -> Result<()> {
        if !self.input_supported {
            return Err(BackendError::new(
                ErrorKind::Unsupported,
                "XTEST/XKB input support is unavailable",
            ));
        }
        let display = self.checked_display(revision, display_id)?;
        match action {
            InputAction::Pointer { x, y, action } => {
                let (x, y) = pointer_coordinates(&display, x, y)?;
                self.before_input(guard)?;
                self.fake(xproto::MOTION_NOTIFY_EVENT, 0, x, y)?;
                let button = match action {
                    PointerAction::Move => return Ok(()),
                    PointerAction::Down(b) | PointerAction::Up(b) | PointerAction::Click(b) => {
                        button_code(b)
                    }
                };
                if matches!(action, PointerAction::Down(_) | PointerAction::Click(_)) {
                    self.before_input(guard)?;
                    self.held_buttons.insert(button); // Track before sending: failed delivery is ambiguous.
                    self.fake(xproto::BUTTON_PRESS_EVENT, button, 0, 0)?;
                }
                if matches!(action, PointerAction::Up(_) | PointerAction::Click(_)) {
                    self.before_input(guard)?;
                    self.fake(xproto::BUTTON_RELEASE_EVENT, button, 0, 0)?;
                    self.held_buttons.remove(&button);
                }
                Ok(())
            }
            InputAction::Key { usage, down } => {
                let keycode = self.hid_keycode(usage)?;
                self.key_event(keycode, down, guard)
            }
            InputAction::Text(text) => self.type_text(&text, guard),
        }
    }

    pub(crate) fn key_event(
        &mut self,
        key: u8,
        down: bool,
        guard: &mut dyn ActionGuard,
    ) -> Result<()> {
        self.before_input(guard)?;
        if down {
            self.held_keys.insert(key);
        }
        self.fake(
            if down {
                xproto::KEY_PRESS_EVENT
            } else {
                xproto::KEY_RELEASE_EVENT
            },
            key,
            0,
            0,
        )?;
        if !down {
            self.held_keys.remove(&key);
        }
        Ok(())
    }

    fn before_input(&mut self, guard: &mut dyn ActionGuard) -> Result<()> {
        if let Some((revision, id)) = self.input_context.clone() {
            self.checked_display(revision, &id)?;
        }
        guard.check()
    }

    fn fake(&self, event: u8, detail: u8, x: i16, y: i16) -> Result<()> {
        self.connection
            .xtest_fake_input(event, detail, 0, self.root, x, y, 0)
            .map_err(BackendError::os)?
            .check()
            .map_err(BackendError::os)
    }

    pub fn release_all(&mut self) -> Result<()> {
        self.connection
            .stream()
            .arm(Instant::now() + Duration::from_secs(2));
        let mut first_error = None;
        for key in self.held_keys.clone() {
            match self.fake(xproto::KEY_RELEASE_EVENT, key, 0, 0) {
                Ok(()) => {
                    self.held_keys.remove(&key);
                }
                Err(e) => {
                    first_error.get_or_insert(e);
                }
            }
        }
        for button in self.held_buttons.clone() {
            match self.fake(xproto::BUTTON_RELEASE_EVENT, button, 0, 0) {
                Ok(()) => {
                    self.held_buttons.remove(&button);
                }
                Err(e) => {
                    first_error.get_or_insert(e);
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

impl Drop for X11Backend {
    fn drop(&mut self) {
        let _ = self.release_all();
    }
}

pub(crate) fn check_deadline(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        Err(BackendError::new(
            ErrorKind::DeadlineExceeded,
            "desktop operation deadline exceeded",
        ))
    } else {
        Ok(())
    }
}
fn button_code(button: Button) -> u8 {
    match button {
        Button::Left => 1,
        Button::Middle => 2,
        Button::Right => 3,
    }
}
pub(crate) fn pointer_coordinates(display: &Display, x: u32, y: u32) -> Result<(i16, i16)> {
    if x >= display.width || y >= display.height {
        return Err(BackendError::new(
            ErrorKind::InvalidArgument,
            "pointer lies outside display",
        ));
    }
    let convert = |origin: i32, offset: u32| {
        i16::try_from(i64::from(origin) + i64::from(offset)).map_err(|_| {
            BackendError::new(
                ErrorKind::Unsupported,
                "pointer lies outside XTEST coordinate range",
            )
        })
    };
    Ok((convert(display.x, x)?, convert(display.y, y)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn display(width: u32, height: u32) -> Display {
        Display {
            id: "1".into(),
            x: 0,
            y: 0,
            width,
            height,
            primary: true,
            can_resize: true,
            modes: vec![],
        }
    }
    #[test]
    fn corner_pixels_are_exact_and_edges_rejected() {
        for (width, height) in [(3840, 2160), (1920, 1080)] {
            let d = display(width, height);
            for (x, y) in [
                (0, 0),
                (width - 1, 0),
                (0, height - 1),
                (width - 1, height - 1),
            ] {
                assert_eq!(pointer_coordinates(&d, x, y).unwrap(), (x as i16, y as i16));
            }
            assert_eq!(
                pointer_coordinates(&d, width, 0).unwrap_err().kind,
                ErrorKind::InvalidArgument
            );
            assert_eq!(
                pointer_coordinates(&d, 0, height).unwrap_err().kind,
                ErrorKind::InvalidArgument
            );
        }
    }
    #[test]
    fn origin_and_xtest_range_are_checked() {
        let mut d = display(3840, 2160);
        d.x = -3840;
        d.y = 100;
        assert_eq!(pointer_coordinates(&d, 3839, 2159).unwrap(), (-1, 2259));
        d.x = i32::MAX;
        assert_eq!(
            pointer_coordinates(&d, 1, 1).unwrap_err().kind,
            ErrorKind::Unsupported
        );
    }
}
