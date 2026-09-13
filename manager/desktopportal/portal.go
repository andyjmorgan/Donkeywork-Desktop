package desktopportal

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"errors"
	"fmt"
	"github.com/godbus/dbus/v5"
	"github.com/jezek/xgb"
	"os"
	"strings"
	"time"
)

const destination = "org.freedesktop.portal.Desktop"
const path = dbus.ObjectPath("/org/freedesktop/portal/desktop")
const remote = "org.freedesktop.portal.RemoteDesktop"
const cast = "org.freedesktop.portal.ScreenCast"

type Options = map[string]dbus.Variant
type Capabilities struct {
	RemoteDesktopVersion uint32 `json:"remoteDesktopVersion"`
	ScreenCastVersion    uint32 `json:"screenCastVersion"`
	Devices              uint32 `json:"devices"`
	Sources              uint32 `json:"sources"`
	CursorModes          uint32 `json:"cursorModes"`
}
type Client struct{ conn *dbus.Conn }
type Stream struct {
	Node       uint32
	Properties map[string]dbus.Variant
}
type Session struct {
	client       *Client
	Handle       dbus.ObjectPath
	Stream       Stream
	Devices      uint32
	PipeWire     *os.File
	RestoreToken string
	x11          *xgb.Conn
}

func Connect() (*Client, error) {
	c, e := dbus.ConnectSessionBus()
	if e != nil {
		return nil, errors.New("desktop session bus unavailable")
	}
	return &Client{c}, nil
}
func (c *Client) Close() { c.conn.Close() }
func (c *Client) Probe() (Capabilities, error) {
	var out Capabilities
	for _, p := range []struct {
		name   string
		target *uint32
	}{{remote + ".version", &out.RemoteDesktopVersion}, {cast + ".version", &out.ScreenCastVersion}, {remote + ".AvailableDeviceTypes", &out.Devices}, {cast + ".AvailableSourceTypes", &out.Sources}, {cast + ".AvailableCursorModes", &out.CursorModes}} {
		v, e := c.conn.Object(destination, path).GetProperty(p.name)
		if e != nil {
			return out, errors.New("required desktop portal interface unavailable")
		}
		if e = v.Store(p.target); e != nil {
			return out, errors.New("invalid portal capabilities")
		}
	}
	return out, nil
}
func token() (string, error) {
	var b [16]byte
	if _, e := rand.Read(b[:]); e != nil {
		return "", e
	}
	return "dw" + hex.EncodeToString(b[:]), nil
}
func response(body []any) (Options, error) {
	if len(body) != 2 {
		return nil, errors.New("invalid portal response")
	}
	code, ok := body[0].(uint32)
	if !ok {
		return nil, errors.New("invalid portal result code")
	}
	if code == 1 {
		return nil, errors.New("sharing cancelled by desktop user")
	}
	if code != 0 {
		return nil, errors.New("desktop portal denied or failed sharing")
	}
	values, ok := body[1].(map[string]dbus.Variant)
	if !ok {
		return nil, errors.New("invalid portal result")
	}
	return values, nil
}
func (c *Client) request(ctx context.Context, method string, args []any, options Options) (Options, error) {
	t, e := token()
	if e != nil {
		return nil, e
	}
	options["handle_token"] = dbus.MakeVariant(t)
	owner := strings.ReplaceAll(strings.TrimPrefix(c.conn.Names()[0], ":"), ".", "_")
	expected := dbus.ObjectPath("/org/freedesktop/portal/desktop/request/" + owner + "/" + t)
	signals := make(chan *dbus.Signal, 8)
	c.conn.Signal(signals)
	defer c.conn.RemoveSignal(signals)
	rules := []dbus.MatchOption{dbus.WithMatchSender(destination), dbus.WithMatchInterface("org.freedesktop.portal.Request"), dbus.WithMatchMember("Response"), dbus.WithMatchObjectPath(expected)}
	if e = c.conn.AddMatchSignal(rules...); e != nil {
		return nil, e
	}
	defer c.conn.RemoveMatchSignal(rules...)
	var handle dbus.ObjectPath
	if e = c.conn.Object(destination, path).CallWithContext(ctx, method, 0, append(args, options)...).Store(&handle); e != nil {
		return nil, errors.New("portal request could not be started")
	}
	defer func() {
		closeCtx, cancel := context.WithTimeout(context.Background(), time.Second)
		defer cancel()
		_ = c.conn.Object(destination, handle).CallWithContext(closeCtx, "org.freedesktop.portal.Request.Close", 0).Err
	}()
	if handle != expected {
		return nil, errors.New("portal returned unexpected request identity")
	}
	var portalOwner string
	if err := c.conn.BusObject().CallWithContext(ctx, "org.freedesktop.DBus.GetNameOwner", 0, destination).Store(&portalOwner); err != nil {
		return nil, errors.New("portal owner unavailable")
	}
	for {
		select {
		case <-ctx.Done():
			return nil, fmt.Errorf("portal request: %w", ctx.Err())
		case signal := <-signals:
			if signal == nil {
				return nil, errors.New("portal signal connection closed")
			}
			if signal.Path != expected || signal.Name != "org.freedesktop.portal.Request.Response" || signal.Sender != portalOwner {
				continue
			}
			return response(signal.Body)
		}
	}
}
func (s *Session) Close() {
	if s.x11 != nil {
		s.x11.Close()
	}
	if s.PipeWire != nil {
		s.PipeWire.Close()
	}
	if s.Handle != "" {
		ctx, cancel := context.WithTimeout(context.Background(), time.Second)
		defer cancel()
		_ = s.client.conn.Object(destination, s.Handle).CallWithContext(ctx, "org.freedesktop.portal.Session.Close", 0).Err
	}
}
func (c *Client) Share(ctx context.Context, progress func(string)) (*Session, error) {
	return c.ShareRestored(ctx, "", false, progress)
}
func (c *Client) ShareRestored(ctx context.Context, restore string, persist bool, progress func(string)) (*Session, error) {
	if os.Getenv("XDG_SESSION_TYPE") == "x11" {
		session, err := c.openX11Session()
		if err == nil {
			progress("session_x11_ready")
		}
		return session, err
	}
	caps, e := c.Probe()
	if e != nil {
		return nil, e
	}
	if caps.Sources&1 == 0 || caps.Devices&3 != 3 {
		return nil, errors.New("monitor, keyboard and pointer portals are required")
	}
	t, e := token()
	if e != nil {
		return nil, e
	}
	result, e := c.request(ctx, remote+".CreateSession", nil, Options{"session_handle_token": dbus.MakeVariant(t)})
	if e != nil {
		return nil, e
	}
	handle, ok := result["session_handle"].Value().(string)
	if !ok || !dbus.ObjectPath(handle).IsValid() || !strings.HasPrefix(handle, "/org/freedesktop/portal/desktop/session/") {
		return nil, errors.New("invalid portal session")
	}
	s := &Session{client: c, Handle: dbus.ObjectPath(handle)}
	success := false
	defer func() {
		if !success {
			s.Close()
		}
	}()
	deviceOptions := Options{"types": dbus.MakeVariant(uint32(3))}
	if persist {
		deviceOptions["persist_mode"] = dbus.MakeVariant(uint32(2))
	}
	if restore != "" {
		deviceOptions["restore_token"] = dbus.MakeVariant(restore)
	}
	if _, e = c.request(ctx, remote+".SelectDevices", []any{s.Handle}, deviceOptions); e != nil {
		return nil, e
	}
	options := Options{"types": dbus.MakeVariant(uint32(1)), "multiple": dbus.MakeVariant(false)}
	if caps.CursorModes&2 != 0 {
		options["cursor_mode"] = dbus.MakeVariant(uint32(2))
	}
	if _, e = c.request(ctx, cast+".SelectSources", []any{s.Handle}, options); e != nil {
		return nil, e
	}
	progress("starting_portal_session")
	result, e = c.request(ctx, remote+".Start", []any{s.Handle, ""}, Options{})
	if e != nil {
		return nil, e
	}
	if v, ok := result["restore_token"]; ok {
		_ = v.Store(&s.RestoreToken)
	}
	devices, hasDevices := result["devices"]
	if !hasDevices {
		return nil, errors.New("missing granted devices")
	}
	if e = devices.Store(&s.Devices); e != nil {
		return nil, errors.New("missing granted devices")
	}
	if s.Devices&3 != 3 {
		return nil, errors.New("portal did not grant keyboard and pointer")
	}
	var streams []Stream
	streamValues, hasStreams := result["streams"]
	if !hasStreams {
		return nil, errors.New("portal granted no screen stream")
	}
	if e = streamValues.Store(&streams); e != nil || len(streams) != 1 {
		return nil, errors.New("exactly one shared monitor required")
	}
	s.Stream = streams[0]
	var fd dbus.UnixFD
	if e = c.conn.Object(destination, path).CallWithContext(ctx, cast+".OpenPipeWireRemote", 0, s.Handle, Options{}).Store(&fd); e != nil {
		return nil, errors.New("restricted PipeWire remote unavailable")
	}
	s.PipeWire = os.NewFile(uintptr(fd), "portal-pipewire")
	if os.Getenv("XDG_SESSION_TYPE") == "x11" {
		s.x11, e = openX11Input()
		if e != nil {
			return nil, e
		}
	}
	success = true
	progress("sharing_granted")
	return s, nil
}
