#!/usr/bin/env python3
"""Lab session factory over password-authenticated SSH/PAM, not a fleet broker."""
import argparse
import getpass
import json
from pathlib import Path
import shlex
import subprocess
import sys
import time
import os
import select
import termios
import tty
import re
import paramiko

p = argparse.ArgumentParser()
p.add_argument("--host", required=True, choices=["192.168.69.28", "192.168.69.21", "192.168.69.17"])
p.add_argument("--username", default="localuser")
p.add_argument("--vault", action="store_true")
p.add_argument("--desktop", help="Broker desktop ID for cli, terminal, status or destroy")
p.add_argument("action", choices=["create", "status", "destroy", "exec", "cli", "terminal", "view", "view-stop"])
p.add_argument("args", nargs=argparse.REMAINDER)
a = p.parse_args()
password = (subprocess.check_output(["dwvault", "credentials", "get", "lab-localuser"],
            text=True).rstrip("\r\n") if a.vault else getpass.getpass())
c = paramiko.SSHClient()
c.load_system_host_keys()
c.connect(a.host, username=a.username, password=password,
          allow_agent=False, look_for_keys=False, timeout=15)
del password
def run(command, data=None):
    inp, out, err = c.exec_command(command)
    if data is not None:
        inp.write(data)
        inp.flush()
    inp.channel.shutdown_write()
    result = out.read()
    errors = err.read()
    code = out.channel.recv_exit_status()
    if code:
        sys.stderr.buffer.write(errors)
        raise RuntimeError(f"remote operation failed ({code})")
    return result.decode()
try:
    home = run("printf %s \"$HOME\"")
    uid = run("id -u").strip()
    root = home + "/.local/state/dwdesktop-managed"
    q = shlex.quote
    env = f"XDG_RUNTIME_DIR=/run/user/{uid} DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{uid}/bus "
    unit = "dwdesktop-managed-poc.service"
    if a.desktop:
        if not re.fullmatch('[0-9a-f]{32}', a.desktop):
            raise RuntimeError("invalid desktop ID")
        if a.action not in ("cli", "terminal", "status", "destroy"):
            raise RuntimeError("--desktop supports cli, terminal, status and destroy only")
        root += "/sessions/" + a.desktop
        run("test -f " + q(root + "/instance.json"))
        unit = "dwdesktop-session-" + a.desktop + ".service"
    if a.action == "create":
        # Persistent definition, deliberately not enabled at boot. Logout stays
        # stopped; only create/the explicit portal button can start it again.
        unitdir = home + "/.config/systemd/user"
        run("mkdir -p " + q(unitdir + "/" + unit + ".d"))
        sftp = c.open_sftp()
        with sftp.open(unitdir + "/" + unit, "w") as f:
            f.write("[Unit]\nDescription=DonkeyWork managed desktop\n[Service]\nType=simple\n"
                    "ExecStart=/usr/bin/python3 " + q(root + "/runtime.py") + " " + q(root) +
                    "\nRestart=no\nKillMode=control-group\nTimeoutStopSec=10\n")
        with sftp.open(unitdir + "/" + unit + ".d/10-lifecycle.conf", "w") as f:
            f.write("[Service]\nRestart=no\n")
        sftp.close()
        run(env + "systemctl --user daemon-reload")
        # Reuse an existing desktop; never replace its cookie or applications.
        state = run(env + "systemctl --user is-active " + unit + " || true").strip()
        if state in ("active", "activating"):
            for attempt in range(30):
                try:
                    print(run(shlex.join([root + "/bin/dwdesktop", "--socket", root + "/cli.sock", "--server-uid", uid, "describe"])))
                    print("Reconnected to existing managed desktop.")
                    sys.exit(0)
                except RuntimeError:
                    time.sleep(.5)
            raise RuntimeError("existing managed desktop did not become ready; not replacing it")
        run("umask 077; mkdir -p " + q(root + "/bin"))
        sftp = c.open_sftp()
        source = Path(__file__).resolve().parent
        sftp.put(str(source / "runtime.py"), root + "/runtime.py")
        config = (source.parent / "headless/xorg-dummy.conf").read_text()
        config = config.replace('PreferredMode" "3840x2160', 'PreferredMode" "1920x1080')
        config = config.replace('Modes "3840x2160" "1920x1080"', 'Modes "1920x1080" "3840x2160"')
        with sftp.open(root + "/xorg.conf", "w") as f:
            f.write(config)
        if not a.host.endswith(".28"):
            for name, rel in [("dwdesktop-core", "device/core"), ("dwdesktop", "cli")]:
                sftp.put(str(source.parent.parent / rel / "target/release" / name), root + "/bin/" + name)
                sftp.chmod(root + "/bin/" + name, 0o700)
        else:
            run("cp " + q(home + "/source/Donkeywork-Desktop/device/core/target/release/dwdesktop-core") + " " + q(root + "/bin/") +
                " && cp " + q(home + "/source/Donkeywork-Desktop/cli/target/release/dwdesktop") + " " + q(root + "/bin/"))
        sftp.close()
        print(run(env + "systemctl --user start " + unit))
        for attempt in range(30):
            ready = run("test -S " + q(root + "/cli.sock") + " && echo ready || true").strip()
            if ready == "ready":
                print(run(shlex.join([root + "/bin/dwdesktop", "--socket", root + "/cli.sock", "--server-uid", uid, "describe"])))
                break
            time.sleep(.5)
        else:
            run(env + "systemctl --user stop " + unit)
            raise RuntimeError("session failed readiness; inspect private runtime logs")
    elif a.action == "view":
        source = Path(__file__).resolve().parent
        arch = "arm64" if a.host.endswith(".28") else "amd64"
        binary = source.parent.parent / "artifacts/managed-web" / arch / "dwconsole-web"
        assets = source.parent.parent / "console-ui/dist"
        if not binary.is_file() or not (assets / "index.html").is_file():
            raise RuntimeError("build managed-web binaries and console-ui first")
        run(env + "systemctl --user stop dwdesktop-managed-web.service || true")
        sftp = c.open_sftp()
        sftp.put(str(source / "web.py"), root + "/web.py")
        sftp.put(str(source / "portal.py"), root + "/portal.py")
        sftp.put(str(source / "broker.py"), root + "/broker.py")
        sftp.put(str(source / "runtime.py"), root + "/runtime.py")
        sftp.put(str(binary), root + "/bin/dwconsole-web")
        sftp.chmod(root + "/bin/dwconsole-web", 0o700)
        for entry in sorted(assets.rglob("*")):
            remote = root + "/web-assets/" + str(entry.relative_to(assets))
            run("mkdir -p " + q(str(Path(remote).parent)))
            if entry.is_file():
                sftp.put(str(entry), remote)
        sftp.close()
        broker_socket = " --socket /run/dwdesktop-session/broker.sock" if run("test -w /run/dwdesktop-session && echo ready || true").strip() == "ready" else ""
        print(run(env + "systemd-run --user --collect --unit=dwdesktop-managed-web.service "
                  "--property=KillMode=control-group --property=TimeoutStopSec=10 "
                  "--property=Restart=always --property=RestartSec=2 --property=StartLimitIntervalSec=60 --property=StartLimitBurst=15 "
                  " python3 " + q(root + "/broker.py") + " " + q(root) + " --host " + q(a.host) + broker_socket))
        print("http://" + a.host + ":8095/?managed")
    elif a.action == "view-stop":
        print(run(env + "systemctl --user stop dwdesktop-managed-web.service"))
    elif a.action == "destroy":
        if not a.desktop:
            run(env + "systemctl --user stop dwdesktop-managed-web.service || true")
        print(run(env + "systemctl --user stop " + unit))
    elif a.action == "status":
        print(run(env + "systemctl --user show " + unit + " -p ActiveState -p SubState -p MainPID"))
    elif a.action == "cli":
        data = sys.stdin.read(65537) if a.args and a.args[0] == "text" else None
        if data is not None and len(data.encode()) > 65536:
            raise RuntimeError("text exceeds input bound")
        print(run(shlex.join([root + "/bin/dwdesktop", "--socket", root + "/cli.sock", "--server-uid", uid] + a.args), data))
    elif a.action == "terminal":
        if not sys.stdin.isatty():
            raise RuntimeError("terminal requires a local TTY")
        ch = c.get_transport().open_session()
        size = os.get_terminal_size()
        ch.get_pty(term="xterm-256color", width=size.columns, height=size.lines)
        ch.exec_command("tmux -S " + q(root + "/terminal.sock") + " attach-session -t desktop")
        saved = termios.tcgetattr(sys.stdin)
        try:
            tty.setraw(sys.stdin)
            while not ch.exit_status_ready():
                current = os.get_terminal_size()
                if current != size:
                    ch.resize_pty(width=current.columns, height=current.lines)
                    size = current
                ready, _, _ = select.select([ch, sys.stdin], [], [], .1)
                if ch in ready:
                    data = ch.recv(65536)
                    if not data:
                        break
                    sys.stdout.buffer.write(data)
                    sys.stdout.buffer.flush()
                if sys.stdin in ready:
                    ch.sendall(os.read(sys.stdin.fileno(), 4096))
        finally:
            termios.tcsetattr(sys.stdin, termios.TCSADRAIN, saved)
            ch.close()
    else:
        print(run("DISPLAY=:109 XAUTHORITY=" + q(root + "/Xauthority") + " " + shlex.join(a.args)))
finally:
    c.close()
