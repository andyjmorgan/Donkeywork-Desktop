"""Bounded existing-compositor virtual-output capture probe; no input injection."""
import os
import sys
import dbus
import dbus.mainloop.glib
import gi
gi.require_version('Gst', '1.0')
from gi.repository import GLib, Gst

dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
Gst.init(None)
bus = dbus.bus.BusConnection(os.environ['DBUS_SESSION_BUS_ADDRESS'])
service = 'org.gnome.Mutter.ScreenCast'
root = dbus.Interface(bus.get_object(service, '/org/gnome/Mutter/ScreenCast'), service)
path = root.CreateSession(dbus.Dictionary({}, signature='sv'))
session = dbus.Interface(bus.get_object(service, path), service + '.Session')
loop = GLib.MainLoop()
pipeline = None
captured = False
output = sys.argv[1]
if os.path.exists(output):
    raise RuntimeError('Refusing to overwrite evidence')

def on_bus(_, message):
    global captured
    if message.type == Gst.MessageType.ERROR:
        error, debug = message.parse_error()
        print('pipeline error:', error.message, flush=True)
        loop.quit()
    elif message.type == Gst.MessageType.EOS:
        captured = True
        loop.quit()

def added(node):
    global pipeline
    print('PipeWire node:', int(node), flush=True)
    pipeline = Gst.parse_launch(
        f'pipewiresrc path={int(node)} num-buffers=1 ! '
        'video/x-raw,width=1920,height=1080,framerate=30/1 ! '
        'videoconvert ! pngenc ! filesink name=evidence')
    pipeline.get_by_name('evidence').set_property('location', output)
    messages = pipeline.get_bus()
    messages.add_signal_watch()
    messages.connect('message', on_bus)
    pipeline.set_state(Gst.State.PLAYING)

try:
    stream = session.RecordVirtual(dbus.Dictionary({
        'cursor-mode': dbus.UInt32(1), 'is-platform': dbus.Boolean(True),
    }, signature='sv'))
    bus.add_signal_receiver(added, signal_name='PipeWireStreamAdded',
                            dbus_interface=service + '.Stream', path=str(stream))
    session.Start()
    GLib.timeout_add_seconds(20, lambda: (loop.quit(), False)[1])
    loop.run()
finally:
    if pipeline:
        pipeline.set_state(Gst.State.NULL)
    session.Stop()
    bus.close()
if not captured:
    raise RuntimeError('No completed frame within probe deadline')
print('Captured one frame; capture session closed', flush=True)
