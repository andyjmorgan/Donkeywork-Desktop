package desktopportal

import (
	"context"
	"crypto/rand"
	"errors"
	"fmt"
	"github.com/godbus/dbus/v5"
	"time"
)

const AppID = "dev.donkeywork.Desktop"
const permissionService = "org.freedesktop.impl.portal.PermissionStore"
const permissionPath = dbus.ObjectPath("/org/freedesktop/impl/portal/PermissionStore")

// BootstrapGNOME46 is an explicitly operator-invoked compatibility experiment,
// not a portable portal API. match must identify the administrator-selected monitor.
func (c *Client) BootstrapGNOME46(ctx context.Context, match string) (string, error) {
	if match == "" {
		return "", errors.New("explicit monitor match required")
	}
	var random [16]byte
	if _, err := rand.Read(random[:]); err != nil {
		return "", err
	}
	random[6] = (random[6] & 15) | 64
	random[8] = (random[8] & 63) | 128
	id := fmt.Sprintf("%x-%x-%x-%x-%x", random[0:4], random[4:6], random[6:8], random[8:10], random[10:16])
	type stream struct {
		ID     uint32
		Source uint32
		Data   dbus.Variant
	}
	type payload struct {
		Created   int64
		Used      int64
		Devices   uint32
		Clipboard bool
		Streams   []stream
	}
	type restore struct {
		Provider string
		Version  uint32
		Data     dbus.Variant
	}
	now := time.Now().UnixMicro()
	data := restore{"GNOME", 1, dbus.MakeVariant(payload{now, now, 3, false, []stream{{0, 1, dbus.MakeVariant(match)}}})}
	err := c.conn.Object(permissionService, permissionPath).CallWithContext(ctx, permissionService+".Set", 0,
		"remote-desktop", true, id, map[string][]string{AppID: {"yes"}}, dbus.MakeVariant(data)).Err
	if err != nil {
		return "", errors.New("could not provision agent permission")
	}
	return id, nil
}

func (c *Client) RemoveBootstrap(ctx context.Context, id string) error {
	var permissions map[string][]string
	var data dbus.Variant
	if err := c.conn.Object(permissionService, permissionPath).CallWithContext(ctx, permissionService+".Lookup", 0, "remote-desktop", id).Store(&permissions, &data); err != nil {
		return errors.New("agent permission not found")
	}
	if len(permissions) != 1 || len(permissions[AppID]) == 0 {
		return errors.New("refusing to remove another application's permission")
	}
	return c.conn.Object(permissionService, permissionPath).CallWithContext(ctx, permissionService+".Delete", 0, "remote-desktop", id).Err
}
