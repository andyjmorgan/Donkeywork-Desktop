#!/usr/bin/env python3
"""Real-host CLI regression proof; writes private screenshot evidence locally."""
import argparse
import json
from pathlib import Path
import shlex
import subprocess
import time
import uuid
import paramiko

p = argparse.ArgumentParser()
p.add_argument("host", choices=["192.168.69.28", "192.168.69.21"])
p.add_argument("--browser", action="store_true")
a = p.parse_args()
c = paramiko.SSHClient()
c.load_system_host_keys()
secret = subprocess.check_output(["dwvault", "credentials", "get", "lab-localuser"], text=True).rstrip("\r\n")
c.connect(a.host, username="localuser", password=secret, allow_agent=False, look_for_keys=False)
del secret
root = "/home/localuser/.local/state/dwdesktop-managed"
tag = str(uuid.uuid4())
context = root + "/" + tag + ".context.json"
outdir = Path("/tmp") / ("dwmanaged-proof-" + a.host + "-" + tag)
outdir.mkdir(mode=0o700)
def command(args, data=None):
    i, o, e = c.exec_command(shlex.join(args))
    if data is not None:
        i.write(data)
        i.flush()
    i.channel.shutdown_write()
    output = o.read()
    error = e.read()
    code = o.channel.recv_exit_status()
    if code:
        raise RuntimeError(error.decode())
    return output.decode()
def cli(*args, data=None):
    return json.loads(command([root + "/bin/dwdesktop", "--socket", root + "/cli.sock", "--server-uid", "1000", *args], data))
def shot(name):
    snap = root + "/" + tag + "-" + name + ".json"
    png = root + "/" + tag + "-" + name + ".png"
    result = cli("screenshot", "--context", context, "--display", display, "--output", png, "--snapshot", snap)
    sftp = c.open_sftp()
    sftp.get(png, str(outdir / (name + ".png")))
    sftp.close()
    return snap, result
try:
    display = cli("describe")["payload"]["displays"][0]["displayId"]
    opened = cli("open", "--context", context)
    before, first = shot("desktop")
    assert first["payload"]["width"] == 1920
    # The default Xfce dock terminal launcher, confirmed visually on the pilot.
    cli("click", "--context", context, "--snapshot", before, "--x", "887", "--y", "1058")
    time.sleep(2)
    before, _ = shot("terminal")
    cli("text", "--context", context, "--snapshot", before,
        data="printf 'DW_MANAGED_INPUT_OK\\n'; id -un; echo DISPLAY=$DISPLAY")
    cli("key", "--context", context, "--snapshot", before, "--usage", "40")
    time.sleep(.5)
    before, _ = shot("typed")
    if a.browser:
        browser = "/opt/firefox/firefox" if a.host.endswith(".28") else root + "/firefox/firefox"
        profile = root + "/browser-profile"
        cli("text", "--context", context, "--snapshot", before,
            data=f"mkdir -p {profile}; {browser} --no-remote --profile {profile} https://example.com &")
        cli("key", "--context", context, "--snapshot", before, "--usage", "40")
        time.sleep(10)
        before, _ = shot("browser")
    for width, height in [(3840, 2160), (1920, 1080)]:
        resized = cli("resize", "--context", context, "--snapshot", before,
                      "--width", str(width), "--height", str(height))
        assert resized["payload"]["status"] == "applied", resized
        before, actual = shot(str(width))
        assert (actual["payload"]["width"], actual["payload"]["height"]) == (width, height)
        assert actual["sessionId"] == opened["sessionId"]
    cli("close", "--context", context)
    clip = root + "/" + tag + ".h264"
    command(["env", "DISPLAY=:109", "XAUTHORITY=" + root + "/Xauthority",
             "ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error",
             "-f", "x11grab", "-framerate", "30", "-video_size", "1920x1080",
             "-i", ":109", "-t", "3", "-an", "-c:v", "libx264", "-preset", "ultrafast",
             "-tune", "zerolatency", "-pix_fmt", "yuv420p", "-f", "h264", clip])
    video = json.loads(command(["ffprobe", "-v", "error", "-count_frames", "-show_entries",
                "stream=codec_name,width,height,nb_read_frames", "-of", "json", clip]))
    assert video["streams"][0]["nb_read_frames"] == "90", video
    context = root + "/" + tag + ".reconnect.json"
    cli("open", "--context", context)
    shot("reconnected")
    cli("close", "--context", context)
    report = {"host": a.host, "passwordAuthentication": True,
              "resize": "1920x1080 -> 3840x2160 -> 1920x1080",
              "sameAttachmentDuringResize": True, "reconnect": True,
              "video": video,
              "input": "submitted; inspect typed.png for rendered confirmation",
              "evidence": str(outdir)}
    (outdir / "report.json").write_text(json.dumps(report, indent=2))
    print(json.dumps(report))
finally:
    c.close()
