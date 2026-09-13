use crate::*;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::{
    randr::{self, ConnectionExt as _, GetCrtcInfoReply, Rotation},
    xproto::ConnectionExt as _,
};

// Transaction orchestration is independent of X11 so mutation/rollback failure
// paths are tested without connecting to or changing a real display.
trait ModeTransaction {
    fn apply(&mut self) -> Result<()>;
    fn confirm_applied(&mut self) -> Result<bool>;
    fn restore(&mut self) -> Result<()>;
    fn confirm_restored(&mut self) -> Result<bool>;
    fn actual_topology(&mut self) -> Result<Topology>;
}
fn transact(transaction: &mut impl ModeTransaction) -> Result<ResizeOutcome> {
    let applied = transaction
        .apply()
        .and_then(|_| transaction.confirm_applied())
        .inspect_err(|error| trace_resize_error("apply/confirm", error))
        .unwrap_or(false);
    let status = if applied {
        ResizeStatus::Applied
    } else {
        let restored = transaction
            .restore()
            .and_then(|_| transaction.confirm_restored())
            .inspect_err(|error| trace_resize_error("restore/confirm", error))
            .unwrap_or(false);
        ResizeStatus::Rejected(if restored {
            ResizeRejection::OsFailed
        } else {
            ResizeRejection::RollbackFailed
        })
    };
    Ok(ResizeOutcome {
        status,
        topology: transaction.actual_topology()?,
    })
}

// Explicit operator diagnostics only. Resize errors contain local X11 protocol
// details and static backend/authorization messages, never input or image bodies.
fn trace_resize_error(stage: &str, error: &BackendError) {
    if std::env::var("DW_DESKTOP_RESIZE_TRACE").as_deref() == Ok("1") {
        eprintln!("dwdesktop-capture resize {stage}: {error}");
    }
}

struct SavedMode {
    crtc_id: u32,
    crtc: GetCrtcInfoReply,
    width: u16,
    height: u16,
    mm_width: u32,
    mm_height: u32,
}
struct X11Transaction<'a> {
    backend: &'a mut X11Backend,
    saved: SavedMode,
    target: Resolution,
    mode: u32,
    guard: &'a mut dyn ActionGuard,
    deadline: Instant,
    mutated: bool,
}

impl X11Backend {
    pub fn resize(
        &mut self,
        expected_revision: u64,
        display_id: &str,
        requested: Resolution,
        guard: &mut dyn ActionGuard,
    ) -> Result<ResizeOutcome> {
        let result = self.resize_inner(expected_revision, display_id, requested, guard);
        if let Err(error) = &result {
            trace_resize_error("preflight/final-topology", error);
        }
        result
    }

    fn resize_inner(
        &mut self,
        expected_revision: u64,
        display_id: &str,
        requested: Resolution,
        guard: &mut dyn ActionGuard,
    ) -> Result<ResizeOutcome> {
        let preflight_deadline = Instant::now() + Duration::from_secs(5);
        self.connection.stream().arm(preflight_deadline);
        guard.check()?;
        let display = self.checked_display(expected_revision, display_id)?;
        let rejected = |topology| {
            Ok(ResizeOutcome {
                status: ResizeStatus::Rejected(ResizeRejection::UnsupportedMode),
                topology,
            })
        };
        if !display.can_resize
            || !display.modes.contains(&requested)
            || requested.width > 4096
            || requested.height > 4096
        {
            return rejected(self.topology.clone());
        }
        let output = display_id
            .parse::<u32>()
            .map_err(|_| BackendError::new(ErrorKind::NotFound, "invalid output identity"))?;
        let resources = self
            .connection
            .randr_get_screen_resources(self.root)
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        let output_info = self
            .connection
            .randr_get_output_info(output, resources.config_timestamp)
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        if output_info.status != randr::SetConfig::SUCCESS || output_info.crtc == 0 {
            return Err(BackendError::new(
                ErrorKind::StaleTopology,
                "output changed before resize",
            ));
        }
        let crtc = self
            .connection
            .randr_get_crtc_info(output_info.crtc, resources.config_timestamp)
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        if crtc.status != randr::SetConfig::SUCCESS
            || crtc.rotation != Rotation::ROTATE0
            || crtc.x != 0
            || crtc.y != 0
            || crtc.outputs != [output]
        {
            return rejected(self.topology()?);
        }
        for other in &resources.crtcs {
            if *other != output_info.crtc {
                let info = self
                    .connection
                    .randr_get_crtc_info(*other, resources.config_timestamp)
                    .map_err(BackendError::os)?
                    .reply()
                    .map_err(BackendError::os)?;
                if info.mode != 0 {
                    return rejected(self.topology()?);
                }
            }
        }
        if !self.simple_transform(output_info.crtc)? {
            return rejected(self.topology()?);
        }
        let Some(mode) = resources
            .modes
            .iter()
            .find(|mode| {
                output_info.modes.contains(&mode.id)
                    && u32::from(mode.width) == requested.width
                    && u32::from(mode.height) == requested.height
            })
            .map(|m| m.id)
        else {
            return rejected(self.topology()?);
        };
        // The legacy GetScreenInfo rates reply fails decoding on the pilot.
        // A fresh authenticated setup reports current root physical dimensions;
        // using the long-lived connection's setup would restore stale dimensions
        // after the first resize. No legacy API/version negotiation is needed.
        let (fresh, fresh_screen) =
            crate::transport::connect_until(self.display_name.as_deref(), preflight_deadline)?;
        let screen = &fresh.setup().roots[fresh_screen];
        let geometry = self
            .connection
            .get_geometry(self.root)
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        let (mm_width, mm_height) =
            root_physical_dimensions(screen, self.root, geometry.width, geometry.height)?;
        drop(fresh);
        self.checked_display(expected_revision, display_id)?;
        guard.check()?;
        self.release_all()?;
        self.connection
            .stream()
            .arm(Instant::now() + Duration::from_secs(5));
        let saved = SavedMode {
            crtc_id: output_info.crtc,
            crtc,
            width: geometry.width,
            height: geometry.height,
            mm_width,
            mm_height,
        };
        let mut transaction = X11Transaction {
            backend: self,
            saved,
            target: requested,
            mode,
            guard,
            deadline: Instant::now() + Duration::from_secs(5),
            mutated: false,
        };
        transact(&mut transaction)
    }
}

fn root_physical_dimensions(
    screen: &x11rb::protocol::xproto::Screen,
    expected_root: u32,
    width: u16,
    height: u16,
) -> Result<(u32, u32)> {
    if screen.root != expected_root
        || screen.width_in_pixels != width
        || screen.height_in_pixels != height
    {
        return Err(BackendError::new(
            ErrorKind::StaleTopology,
            "root geometry changed during resize preflight",
        ));
    }
    if width == 0
        || height == 0
        || screen.width_in_millimeters == 0
        || screen.height_in_millimeters == 0
    {
        return Err(BackendError::new(
            ErrorKind::Unsupported,
            "current root physical dimensions are unavailable",
        ));
    }
    Ok((
        screen.width_in_millimeters.into(),
        screen.height_in_millimeters.into(),
    ))
}

impl X11Transaction<'_> {
    fn screen_size(
        &mut self,
        width: u16,
        height: u16,
        mm_width: u32,
        mm_height: u32,
        cleanup: bool,
    ) -> Result<()> {
        if !cleanup {
            crate::x11::check_deadline(self.deadline)?;
            self.guard.check()?;
        }
        self.mutated = true;
        self.backend
            .connection
            .randr_set_screen_size(self.backend.root, width, height, mm_width, mm_height)
            .map_err(BackendError::os)?
            .check()
            .map_err(BackendError::os)
    }
    fn set_crtc(&mut self, mode: u32, cleanup: bool) -> Result<()> {
        let resources = self
            .backend
            .connection
            .randr_get_screen_resources(self.backend.root)
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        if !cleanup {
            crate::x11::check_deadline(self.deadline)?;
            self.guard.check()?;
        }
        self.mutated = true;
        let result = self
            .backend
            .connection
            .randr_set_crtc_config(
                self.saved.crtc_id,
                0,
                resources.config_timestamp,
                self.saved.crtc.x,
                self.saved.crtc.y,
                mode,
                self.saved.crtc.rotation,
                &self.saved.crtc.outputs,
            )
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        if result.status == randr::SetConfig::SUCCESS {
            Ok(())
        } else {
            Err(BackendError::new(
                ErrorKind::OsFailed,
                "RandR rejected CRTC mode",
            ))
        }
    }
    fn confirm(&self, mode: u32, width: u16, height: u16) -> Result<bool> {
        let resources = self
            .backend
            .connection
            .randr_get_screen_resources(self.backend.root)
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        let actual = self
            .backend
            .connection
            .randr_get_crtc_info(self.saved.crtc_id, resources.config_timestamp)
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        let root = self
            .backend
            .connection
            .get_geometry(self.backend.root)
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        Ok(actual.status == randr::SetConfig::SUCCESS
            && actual.mode == mode
            && actual.outputs == self.saved.crtc.outputs
            && actual.x == self.saved.crtc.x
            && actual.y == self.saved.crtc.y
            && actual.rotation == self.saved.crtc.rotation
            && root.width == width
            && root.height == height)
    }
}
impl ModeTransaction for X11Transaction<'_> {
    fn apply(&mut self) -> Result<()> {
        self.guard.check()?;
        crate::x11::check_deadline(self.deadline)?;
        let width = self.target.width as u16;
        let height = self.target.height as u16;
        let mm_width = ((u64::from(self.saved.mm_width) * u64::from(width))
            / u64::from(self.saved.width))
        .max(1) as u32;
        let mm_height = ((u64::from(self.saved.mm_height) * u64::from(height))
            / u64::from(self.saved.height))
        .max(1) as u32;
        if width > self.saved.width || height > self.saved.height {
            self.screen_size(
                width.max(self.saved.width),
                height.max(self.saved.height),
                mm_width,
                mm_height,
                false,
            )?;
        }
        self.guard.check()?;
        crate::x11::check_deadline(self.deadline)?;
        self.set_crtc(self.mode, false)?;
        self.guard.check()?;
        crate::x11::check_deadline(self.deadline)?;
        self.screen_size(width, height, mm_width, mm_height, false)
    }
    fn confirm_applied(&mut self) -> Result<bool> {
        crate::x11::check_deadline(self.deadline)?;
        self.guard.check()?;
        self.confirm(
            self.mode,
            self.target.width as u16,
            self.target.height as u16,
        )
    }
    fn restore(&mut self) -> Result<()> {
        // Restoration is cleanup, allowed even after control expiry/cancellation.
        self.backend
            .connection
            .stream()
            .arm(Instant::now() + Duration::from_secs(2));
        if !self.mutated {
            return Ok(());
        }
        let root = self
            .backend
            .connection
            .get_geometry(self.backend.root)
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        self.screen_size(
            root.width.max(self.saved.width),
            root.height.max(self.saved.height),
            self.saved.mm_width,
            self.saved.mm_height,
            true,
        )?;
        self.set_crtc(self.saved.crtc.mode, true)?;
        self.screen_size(
            self.saved.width,
            self.saved.height,
            self.saved.mm_width,
            self.saved.mm_height,
            true,
        )
    }
    fn confirm_restored(&mut self) -> Result<bool> {
        self.confirm(self.saved.crtc.mode, self.saved.width, self.saved.height)
    }
    fn actual_topology(&mut self) -> Result<Topology> {
        self.backend.topology()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn screen(
        width: u16,
        height: u16,
        mm_width: u16,
        mm_height: u16,
    ) -> x11rb::protocol::xproto::Screen {
        x11rb::protocol::xproto::Screen {
            root: 42,
            width_in_pixels: width,
            height_in_pixels: height,
            width_in_millimeters: mm_width,
            height_in_millimeters: mm_height,
            ..Default::default()
        }
    }
    #[test]
    fn resize_snapshots_current_mm_after_previous_resize_without_legacy_rates() {
        let initial = screen(3840, 2160, 1016, 571);
        let refreshed = screen(1920, 1080, 508, 285);
        assert_eq!(
            root_physical_dimensions(&initial, 42, 3840, 2160).unwrap(),
            (1016, 571)
        );
        assert_eq!(
            root_physical_dimensions(&refreshed, 42, 1920, 1080).unwrap(),
            (508, 285)
        );
        assert_eq!(
            root_physical_dimensions(&initial, 42, 1920, 1080)
                .unwrap_err()
                .kind,
            ErrorKind::StaleTopology
        );
    }
    #[test]
    fn mismatched_or_missing_root_dimensions_fail_before_mutation() {
        assert_eq!(
            root_physical_dimensions(&screen(3840, 2160, 1016, 571), 99, 3840, 2160)
                .unwrap_err()
                .kind,
            ErrorKind::StaleTopology
        );
        assert_eq!(
            root_physical_dimensions(&screen(3840, 2160, 0, 571), 42, 3840, 2160)
                .unwrap_err()
                .kind,
            ErrorKind::Unsupported
        );
        assert_eq!(
            root_physical_dimensions(&screen(3840, 2160, 1016, 0), 42, 3840, 2160)
                .unwrap_err()
                .kind,
            ErrorKind::Unsupported
        );
    }
    struct Fake {
        apply_ok: bool,
        confirm_ok: bool,
        restore_ok: bool,
        restored: bool,
        vanished: bool,
        actions: Vec<&'static str>,
    }
    impl ModeTransaction for Fake {
        fn apply(&mut self) -> Result<()> {
            self.actions.push("apply");
            if self.apply_ok {
                Ok(())
            } else {
                Err(BackendError::new(ErrorKind::OsFailed, "injected"))
            }
        }
        fn confirm_applied(&mut self) -> Result<bool> {
            self.actions.push("confirm");
            Ok(self.confirm_ok)
        }
        fn restore(&mut self) -> Result<()> {
            self.actions.push("restore");
            if self.restore_ok {
                Ok(())
            } else {
                Err(BackendError::new(ErrorKind::OsFailed, "injected"))
            }
        }
        fn confirm_restored(&mut self) -> Result<bool> {
            self.actions.push("confirm_restore");
            Ok(self.restored)
        }
        fn actual_topology(&mut self) -> Result<Topology> {
            self.actions.push("actual");
            if self.vanished {
                Err(BackendError::new(
                    ErrorKind::NotFound,
                    "display disappeared",
                ))
            } else {
                Ok(Topology {
                    revision: 7,
                    displays: Vec::new(),
                })
            }
        }
    }
    fn fake() -> Fake {
        Fake {
            apply_ok: true,
            confirm_ok: true,
            restore_ok: true,
            restored: true,
            vanished: false,
            actions: vec![],
        }
    }
    #[test]
    fn confirmed_apply_avoids_rollback() {
        let mut f = fake();
        assert_eq!(transact(&mut f).unwrap().status, ResizeStatus::Applied);
        assert_eq!(f.actions, ["apply", "confirm", "actual"]);
    }
    #[test]
    fn failure_rolls_back_and_reports_actual() {
        let mut f = fake();
        f.apply_ok = false;
        let result = transact(&mut f).unwrap();
        assert_eq!(
            result.status,
            ResizeStatus::Rejected(ResizeRejection::OsFailed)
        );
        assert_eq!(result.topology.revision, 7);
        assert_eq!(f.actions, ["apply", "restore", "confirm_restore", "actual"]);
    }
    #[test]
    fn unconfirmed_success_rolls_back() {
        let mut f = fake();
        f.confirm_ok = false;
        assert_eq!(
            transact(&mut f).unwrap().status,
            ResizeStatus::Rejected(ResizeRejection::OsFailed)
        );
    }
    #[test]
    fn failed_rollback_is_explicit() {
        let mut f = fake();
        f.apply_ok = false;
        f.restore_ok = false;
        assert_eq!(
            transact(&mut f).unwrap().status,
            ResizeStatus::Rejected(ResizeRejection::RollbackFailed)
        );
    }
    #[test]
    fn unconfirmed_rollback_is_failure() {
        let mut f = fake();
        f.confirm_ok = false;
        f.restored = false;
        assert_eq!(
            transact(&mut f).unwrap().status,
            ResizeStatus::Rejected(ResizeRejection::RollbackFailed)
        );
    }
    #[test]
    fn vanished_display_never_invents_geometry() {
        let mut f = fake();
        f.apply_ok = false;
        f.vanished = true;
        assert_eq!(transact(&mut f).unwrap_err().kind, ErrorKind::NotFound);
    }
}
