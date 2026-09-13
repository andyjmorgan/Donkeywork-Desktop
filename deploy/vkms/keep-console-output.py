"""Pilot: hold an idle inhibitor while the monitorless viewer is served."""
import os
import dbus
import dbus.mainloop.glib
from gi.repository import GLib

dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
bus = dbus.bus.BusConnection(os.environ['DBUS_SESSION_BUS_ADDRESS'])
manager = dbus.Interface(bus.get_object('org.gnome.SessionManager',
    '/org/gnome/SessionManager'), 'org.gnome.SessionManager')
cookie = manager.Inhibit('DonkeyWork console', dbus.UInt32(0),
    'Remote console output', dbus.UInt32(8))
# Do not deactivate ScreenSaver: GDM's greeter itself uses the screen shield.
# Hold idle inhibition and power the output without dismissing that UI.
display = bus.get_object('org.gnome.Mutter.DisplayConfig', '/org/gnome/Mutter/DisplayConfig')
dbus.Interface(display, 'org.freedesktop.DBus.Properties').Set(
    'org.gnome.Mutter.DisplayConfig', 'PowerSaveMode', dbus.Int32(0))
config = dbus.Interface(display, 'org.gnome.Mutter.DisplayConfig')
serial, monitors, logical, properties = config.GetCurrentState()
for spec, modes, props in monitors:
    if str(spec[0]) == 'Virtual-1':
        selected = next(m for m in modes if int(m[1]) == 1920 and int(m[2]) == 1080)
        outputs = dbus.Array([dbus.Struct((str(spec[0]), str(selected[0]),
            dbus.Dictionary({}, signature='sv')), signature='ssa{sv}')], signature='(ssa{sv})')
        layout = dbus.Array([dbus.Struct((dbus.Int32(0), dbus.Int32(0), dbus.Double(1),
            dbus.UInt32(0), dbus.Boolean(True), outputs), signature='iiduba(ssa{sv})')],
            signature='(iiduba(ssa{sv}))')
        config.ApplyMonitorsConfig(serial, dbus.UInt32(1), layout, dbus.Dictionary({}, signature='sv'))
        break
else:
    raise RuntimeError('VKMS output is not present in the compositor')
print('VKMS 1080p output ready; idle inhibitor held', flush=True)
try:
    GLib.MainLoop().run()
finally:
    manager.Uninhibit(cookie)
    bus.close()
