# In-desktop agent — portal capture slice

This is an existing logged-in desktop adapter, not managed-session creation or
greeter access. Run as the graphical desktop user on that desktop's session bus.
No root, uinput, KMS, compositor-private D-Bus API or display-manager changes.

`dwdesktop-session-agent probe` reports public portal capabilities without
requesting capture. `share --snapshot PATH` requests one monitor plus keyboard
and pointer via the standard RemoteDesktop/ScreenCast portals. The desktop
user approves the portal dialog by default. Denial/cancellation/timeouts fail
closed and close the portal session. An explicit operator-only GNOME 46.2
bootstrap experiment can seed one random, app-scoped permission record for
`dev.donkeywork.Desktop`, never a global or empty-app permission. Run under
`app-dev.donkeywork.Desktop.service` so the portal identifies the native app.
`--restore-file` saves the returned token privately (0600, owned 0700 parent).
`--bootstrap-gnome46-monitor vendor:product:serial` requires a new token file
and explicit selection from the actual monitor metadata. This is backend-version
specific, not portable provisioning and not automatically run on consent denial.

Subscribe before calling asynchronous portal methods; bind each response to its
request object and portal sender. Keep the session-bus connection and returned
PipeWire FD alive. Pass that FD to the capture child, never use the unrestricted
default PipeWire remote as a fallback. Accept exactly one granted stream.
Snapshot output is a new exclusive0600 file; never overwrite an existing image.

This first executable captures one PNG after consent. It does not advertise a
working manager console connection, inject input, resize the physical display,
install autostart, or replace the managed broker. Portal control grants are
reported separately from capture; backend absence is not inferred as support.
Next integration: consented PipeWire→H.264 adapter and peer-UID-checked local
registration with the device service, then control leases and input forwarding.

References (API documentation, not copied implementation):
https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html
https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html
