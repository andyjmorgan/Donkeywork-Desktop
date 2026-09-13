#!/usr/bin/env python3
"""Private Ubuntu desktop supervisor. Runs as the authenticated session user."""
import json
import os
from pathlib import Path
import secrets
import signal
import subprocess
import sys
import time
import uuid
import fcntl

root = Path(sys.argv[1]).resolve()
instance = json.loads((root / "instance.json").read_text()) if (root / "instance.json").exists() else {}
display_number = instance.get("display", 109)
desktop_environment = instance.get("environment", "xfce")
display_name = f":{display_number}"
os.umask(0o077)
root.mkdir(mode=0o700, parents=True, exist_ok=True)
lock = open(root / "runtime.lock", "a")
fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
if Path(f"/tmp/.X11-unix/X{display_number}").exists() or Path(f"/tmp/.X{display_number}-lock").exists():
    raise RuntimeError("display is occupied; refusing to attach or delete it")
os.environ.update(DISPLAY=display_name, XAUTHORITY=str(root / "Xauthority"),
                  XDG_RUNTIME_DIR=str(root), XDG_CONFIG_HOME=str(root / "config"),
                  XDG_CACHE_HOME=str(root / "cache"), XDG_CURRENT_DESKTOP="XFCE",
                  XDG_SESSION_TYPE="x11", LIBGL_ALWAYS_SOFTWARE="1")
os.environ["TMUX_TMPDIR"] = str(root)
os.environ["BYOBU_DISABLE"] = "1"
os.environ.pop("DBUS_SESSION_BUS_ADDRESS", None)
os.environ.pop("SESSION_MANAGER", None)
os.environ.pop("WAYLAND_DISPLAY", None)
for key in ("XDG_SESSION_ID", "XDG_SEAT", "XDG_VTNR", "GNOME_SETUP_DISPLAY",
            "GNOME_DESKTOP_SESSION_ID", "GDMSESSION", "DESKTOP_AUTOSTART_ID"):
    os.environ.pop(key, None)
for name in ("config", "cache", "empty-config", "empty-autostart"):
    (root / name).mkdir(exist_ok=True, mode=0o700)
os.environ["XDG_CONFIG_DIRS"] = str(root / "empty-autostart")
if desktop_environment == "gnome":
    os.environ.update(XDG_CURRENT_DESKTOP="ubuntu:GNOME", XDG_SESSION_DESKTOP="ubuntu",
                      DESKTOP_SESSION="ubuntu", GNOME_SESSION_DISABLE_SYSTEMD="1",
                      GALLIUM_DRIVER="llvmpipe", LP_NUM_THREADS="4")
    os.environ["XDG_CONFIG_DIRS"] = "/etc/xdg"
elif desktop_environment != "xfce":
    raise RuntimeError("unsupported managed desktop environment")
# Xfce also searches distro autostart paths. Override them in this desktop only;
# physical-seat lockers/polkit agents must not join an unseated managed session.
autostart = root / "config/autostart"
autostart.mkdir(exist_ok=True, mode=0o700)
for entry in Path("/etc/xdg/autostart").glob("*.desktop"):
    if desktop_environment == "gnome" and entry.name.startswith("org.gnome.SettingsDaemon."):
        continue
    (autostart / entry.name).write_text("[Desktop Entry]\nType=Application\nName=Disabled in managed session\nHidden=true\n")
if desktop_environment == "gnome":
    # Mutter can infer the physical logind seat's Wayland session type even
    # with XDG_SESSION_TYPE=x11. Override only this private session's launcher.
    applications = root / "data/applications"
    applications.mkdir(parents=True, exist_ok=True, mode=0o700)
    shell_entry = Path("/usr/share/applications/org.gnome.Shell.desktop").read_text()
    shell_entry = shell_entry.replace("Exec=/usr/bin/gnome-shell\n", "Exec=/usr/bin/gnome-shell --x11\n")
    (applications / "org.gnome.Shell.desktop").write_text(shell_entry)
    os.environ["XDG_DATA_HOME"] = str(root / "data")
children = []
def launch(args, name):
    log = open(root / (name + ".log"), "ab", buffering=0)
    p = subprocess.Popen(args, stdout=log, stderr=log, start_new_session=True)
    log.close()
    children.append(p)
    return p
def stop(*_):
    raise SystemExit(0)
signal.signal(signal.SIGTERM, stop)
signal.signal(signal.SIGINT, stop)
try:
    subprocess.run(["xauth", "-f", os.environ["XAUTHORITY"], "source", "-"],
                   input=f"add {display_name} MIT-MAGIC-COOKIE-1 {secrets.token_hex(16)}\n",
                   text=True, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    launch(["/usr/lib/xorg/Xorg", display_name, "-config", str(root / "xorg.conf"),
            "-configdir", str(root / "empty-config"), "-auth", os.environ["XAUTHORITY"],
            "-nolisten", "tcp", "-noreset", "-novtswitch", "-sharevts", "-seat",
            "dwmanaged", "-logfile", str(root / "Xorg.log")], "xorg")
    for _ in range(100):
        if subprocess.run(["xdpyinfo"], stdout=subprocess.DEVNULL,
                          stderr=subprocess.DEVNULL).returncode == 0:
            break
        if children[0].poll() is not None:
            raise RuntimeError("private X server exited")
        time.sleep(.1)
    else:
        raise RuntimeError("private X server did not become ready")
    subprocess.run(["xrandr", "--output", "DUMMY0", "--mode", "1920x1080"], check=True)
    desktop_command = ["gnome-session", "--session=ubuntu"] if desktop_environment == "gnome" else ["xfce4-session"]
    desktop = launch(["dbus-run-session", "--", *desktop_command], "desktop")
    for _ in range(100):
        props = subprocess.run(["xprop", "-root", "_NET_SUPPORTING_WM_CHECK"],
                               capture_output=True, text=True)
        if "window id # 0x" in props.stdout:
            break
        if desktop.poll() is not None:
            raise RuntimeError("desktop exited before window manager ready")
        time.sleep(.1)
    else:
        raise RuntimeError("window manager did not become ready")
    # An independent PTY survives client detachment, but belongs to this unit.
    subprocess.run(["tmux", "-S", str(root / "terminal.sock"), "-f", "/dev/null",
                    "new-session", "-d", "-s", "desktop", "bash", "--noprofile", "--norc"], check=True)
    config = {"socket_path": str(root / "cli.sock"), "worker_id": str(uuid.uuid4()),
              "backend": {"kind": "x11", "display": display_name},
              "policies": [{"uid": os.getuid(), "profile": "managed-poc",
                            "permissions": ["desktop.view", "desktop.control", "desktop.resize"]}]}
    (root / "core.json").write_text(json.dumps(config))
    launch([str(root / "bin/dwdesktop-core"), "--config", str(root / "core.json")], "core")
    while all(p.poll() is None for p in children):
        time.sleep(.5)
    if desktop.poll() == 0:
        print("Managed desktop logged out; retiring this instance before recreation.", flush=True)
        raise SystemExit(0)
    raise RuntimeError("managed component exited; closing desktop")
finally:
    failure = sys.exc_info()[1]
    (root / "lifecycle.json").write_text(json.dumps({"state": "ended" if failure is None or
        isinstance(failure, SystemExit) and failure.code in (None, 0) else "failed"}))
    subprocess.run(["tmux", "-S", str(root / "terminal.sock"), "kill-server"],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=2)
    for p in reversed(children):
        if p.poll() is None:
            os.killpg(p.pid, signal.SIGTERM)
    for p in children:
        try:
            p.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGKILL)
            p.wait()
    (root / "cli.sock").unlink(missing_ok=True)
