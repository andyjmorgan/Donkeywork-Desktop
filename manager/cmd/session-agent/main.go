package main

import (
	"context"
	"donkeywork-desktop/manager/desktopportal"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"image/png"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"time"
)

func emit(v any) { _ = json.NewEncoder(os.Stdout).Encode(v) }
func run() error {
	if os.Geteuid() == 0 {
		return errors.New("run as the logged-in desktop user, not root")
	}
	if len(os.Args) < 2 {
		return errors.New("usage: dwdesktop-session-agent probe | share --snapshot PATH [--timeout 2m]")
	}
	f := flag.NewFlagSet(os.Args[1], flag.ContinueOnError)
	output := f.String("snapshot", "", "new private PNG output file")
	timeout := f.Duration("timeout", 2*time.Minute, "consent and capture deadline")
	tokenFile := f.String("restore-file", "", "private restore token file in an owned 0700 directory")
	bootstrap := f.String("bootstrap-gnome46-monitor", "", "EXPERIMENTAL: explicitly provision GNOME 46 monitor vendor:product:serial")
	provision := f.Bool("provision-if-missing", false, "operator opt-in: provision a single GNOME monitor only when no token file exists")
	socket := f.String("socket", "", "private Unix socket for console adapter")
	gatewaySocket := f.String("gateway-socket", "", "optional fixed runtime socket for console-only device gateway")
	if e := f.Parse(os.Args[2:]); e != nil {
		return e
	}
	if *timeout <= 0 || *timeout > 15*time.Minute {
		return errors.New("timeout must be between zero and 15 minutes")
	}
	c, e := desktopportal.Connect()
	if e != nil {
		return e
	}
	defer c.Close()
	if os.Args[1] == "revoke" {
		if *tokenFile == "" {
			return errors.New("restore file required")
		}
		if e = checkTokenDir(*tokenFile); e != nil {
			return e
		}
		data, err := os.ReadFile(*tokenFile)
		if err != nil {
			return err
		}
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		if err = c.RemoveBootstrap(ctx, strings.TrimSpace(string(data))); err != nil {
			return err
		}
		emit(map[string]string{"state": "agent_permission_revoked"})
		return nil
	}
	switch os.Args[1] {
	case "probe":
		caps, e := c.Probe()
		if e != nil {
			return e
		}
		emit(caps)
		return nil
	case "share", "serve":
		var restore string
		missingToken := false
		if *tokenFile != "" {
			cgroup, err := os.ReadFile("/proc/self/cgroup")
			if err != nil || !strings.Contains(string(cgroup), "/app-"+desktopportal.AppID+".service\n") {
				return errors.New("persistent sharing must run as app-dev.donkeywork.Desktop.service")
			}
			if e := checkTokenDir(*tokenFile); e != nil {
				return e
			}
			if info, err := os.Lstat(*tokenFile); err == nil {
				if !info.Mode().IsRegular() || info.Mode().Perm() != 0600 {
					return errors.New("restore file must be regular mode 0600")
				}
				data, err := os.ReadFile(*tokenFile)
				if err != nil {
					return err
				}
				restore = strings.TrimSpace(string(data))
			} else if !os.IsNotExist(err) {
				return err
			} else {
				missingToken = true
			}
		}
		if *bootstrap != "" && (*tokenFile == "" || restore != "") {
			return errors.New("bootstrap requires a new restore file")
		}
		if *provision && missingToken && os.Getenv("XDG_SESSION_TYPE") != "x11" {
			*bootstrap = "auto"
		}
		if os.Args[1] == "serve" {
			if *socket == "" || *tokenFile == "" {
				return errors.New("serve requires socket and restore-file")
			}
			if e := checkTokenDir(*socket); e != nil {
				return e
			}
			for _, plugin := range []string{"pipewiresrc", "videoscale", "videorate"} {
				if err := exec.Command("gst-inspect-1.0", plugin).Run(); err != nil {
					return errors.New("required GStreamer plugin unavailable: " + plugin)
				}
			}
			if !desktopportal.HasEncoder() {
				return errors.New("H.264 encoder unavailable")
			}
		} else if *output == "" {
			return errors.New("snapshot output is required for this capture slice")
		}
		gst, e := exec.LookPath("gst-launch-1.0")
		if e != nil {
			return errors.New("GStreamer runtime required")
		}
		if _, e = os.Lstat(*output); os.Args[1] == "share" && !os.IsNotExist(e) {
			return errors.New("snapshot path already exists or cannot be checked")
		}
		interrupted, stop := signal.NotifyContext(context.Background(), syscall.SIGTERM, syscall.SIGINT)
		defer stop()
		ctx, cancel := context.WithTimeout(interrupted, *timeout)
		defer cancel()
		if *bootstrap != "" {
			if !desktopportal.SupportedGNOMEBootstrap() {
				return errors.New("unsupported GNOME permission bootstrap version")
			}
			if *bootstrap == "auto" {
				*bootstrap, e = c.GNOMEMonitorMatch(ctx)
				if e != nil {
					return e
				}
			}
			restore, e = c.BootstrapGNOME46(ctx, *bootstrap)
			if e != nil {
				return e
			}
			if e = saveToken(*tokenFile, restore); e != nil {
				_ = c.RemoveBootstrap(ctx, restore)
				return e
			}
			emit(map[string]string{"state": "agent_permission_provisioned"})
		}
		session, e := c.ShareRestored(ctx, restore, *tokenFile != "", func(state string) { emit(map[string]string{"state": state}) })
		if e != nil {
			return e
		}
		defer session.Close()
		if *tokenFile != "" && os.Getenv("XDG_SESSION_TYPE") != "x11" {
			if session.RestoreToken == "" {
				return errors.New("portal did not return persistent permission")
			}
			if e = saveToken(*tokenFile, session.RestoreToken); e != nil {
				return e
			}
		}
		if os.Args[1] == "serve" {
			if *gatewaySocket != "" && *gatewaySocket != "/run/dwdesktop-session/broker.sock" {
				return errors.New("unsupported gateway socket")
			}
			return session.Serve(interrupted, *socket, *gatewaySocket)
		}
		out, e := os.OpenFile(*output, os.O_RDWR|os.O_CREATE|os.O_EXCL, 0600)
		if e != nil {
			return e
		}
		defer out.Close()
		cmd := exec.CommandContext(ctx, gst, "-q", "pipewiresrc", "fd=3", "path="+strconv.FormatUint(uint64(session.Stream.Node), 10), "do-timestamp=true", "!", "videoconvert", "!", "pngenc", "snapshot=true", "!", "fdsink", "fd=1")
		cmd.ExtraFiles = []*os.File{session.PipeWire}
		cmd.Stdout = out
		if e = cmd.Run(); e != nil {
			return errors.New("portal capture failed; partial output retained for diagnosis")
		}
		if _, e = out.Seek(0, 0); e != nil {
			return e
		}
		config, e := png.DecodeConfig(out)
		if e != nil {
			return errors.New("capture did not produce a PNG")
		}
		emit(map[string]any{"state": "snapshot_complete", "width": config.Width, "height": config.Height, "grantedDevices": session.Devices, "snapshot": *output})
		return nil
	default:
		return errors.New("unknown action")
	}
}
func checkTokenDir(path string) error {
	info, err := os.Lstat(filepath.Dir(path))
	if err != nil {
		return err
	}
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !info.IsDir() || info.Mode().Perm() != 0700 || !ok || stat.Uid != uint32(os.Geteuid()) {
		return errors.New("restore directory must be owned by this user and mode 0700")
	}
	return nil
}
func saveToken(path, token string) error {
	if err := checkTokenDir(path); err != nil {
		return err
	}
	f, err := os.CreateTemp(filepath.Dir(path), ".restore-*")
	if err != nil {
		return err
	}
	defer os.Remove(f.Name())
	if _, err = f.WriteString(token); err != nil {
		f.Close()
		return err
	}
	if err = f.Sync(); err != nil {
		f.Close()
		return err
	}
	if err = f.Close(); err != nil {
		return err
	}
	return os.Rename(f.Name(), path)
}
func main() {
	if e := run(); e != nil {
		fmt.Fprintln(os.Stderr, e)
		os.Exit(1)
	}
}
