"""Invoke normal Xfce logout, scoped to the managed unit's own session bus."""
from pathlib import Path
import os
import subprocess

group = subprocess.check_output(["systemctl", "--user", "show", "dwdesktop-managed-poc", "-p", "ControlGroup", "--value"], text=True).strip()
if not group.endswith("/dwdesktop-managed-poc.service"):
    raise RuntimeError("unexpected managed cgroup")
pids = (Path("/sys/fs/cgroup") / group.lstrip("/") / "cgroup.procs").read_text().split()
for pid in pids:
    proc = Path("/proc") / pid
    try:
        if proc.joinpath("comm").read_text().strip() != "xfce4-session":
            continue
        env = dict(item.decode().split("=", 1) for item in proc.joinpath("environ").read_bytes().split(b"\0") if b"=" in item)
        if env.get("DISPLAY") not in (":109", ":109.0"):
            raise RuntimeError("not the private managed display")
        subprocess.run(["xfce4-session-logout", "--logout", "--fast"], env=env, check=True, timeout=10)
        break
    except FileNotFoundError:
        continue
else:
    raise RuntimeError("managed session manager not found")
