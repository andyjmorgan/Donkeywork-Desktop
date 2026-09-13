package desktopportal

import (
	"context"
	"errors"
	"github.com/godbus/dbus/v5"
	"os/exec"
	"strings"
)

// GNOMEMonitorMatch is used only by the explicitly enabled initial provisioning
// path. It never changes monitor configuration or silently chooses among displays.
func (c *Client) GNOMEMonitorMatch(ctx context.Context) (string, error) {
	type spec struct{ Connector, Vendor, Product, Serial string }
	type logical struct {
		X, Y       int32
		Scale      float64
		Transform  uint32
		Primary    bool
		Monitors   []spec
		Properties Options
	}
	call := c.conn.Object("org.gnome.Mutter.DisplayConfig", dbus.ObjectPath("/org/gnome/Mutter/DisplayConfig")).CallWithContext(ctx, "org.gnome.Mutter.DisplayConfig.GetCurrentState", 0)
	if call.Err != nil || len(call.Body) != 4 {
		return "", errors.New("GNOME monitor discovery unavailable")
	}
	var monitors []logical
	if dbus.Store(call.Body[2:3], &monitors) != nil || len(monitors) != 1 || len(monitors[0].Monitors) != 1 {
		return "", errors.New("initial setup requires exactly one active monitor")
	}
	m := monitors[0].Monitors[0]
	return m.Vendor + ":" + m.Product + ":" + m.Serial, nil
}

func SupportedGNOMEBootstrap() bool {
	out, err := exec.Command("dpkg-query", "-W", "-f=${Version}", "xdg-desktop-portal-gnome").Output()
	if err == nil {
		return strings.HasPrefix(string(out), "46.2-")
	}
	out, err = exec.Command("rpm", "-q", "--qf", "%{VERSION}", "xdg-desktop-portal-gnome").Output()
	return err == nil && strings.TrimSpace(string(out)) == "47.2"
}
