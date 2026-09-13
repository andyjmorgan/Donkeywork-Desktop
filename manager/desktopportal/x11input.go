package desktopportal

import (
	"errors"
	"github.com/godbus/dbus/v5"
	"github.com/jezek/xgb"
	"github.com/jezek/xgb/xproto"
	"github.com/jezek/xgb/xtest"
)

func (c *Client) openX11Session() (*Session, error) {
	conn, err := openX11Input()
	if err != nil {
		return nil, err
	}
	geometry, err := xproto.GetGeometry(conn, xproto.Drawable(xproto.Setup(conn).DefaultScreen(conn).Root)).Reply()
	if err != nil {
		conn.Close()
		return nil, errors.New("session X geometry unavailable")
	}
	size := struct{ Width, Height int32 }{int32(geometry.Width), int32(geometry.Height)}
	return &Session{client: c, x11: conn, Devices: 3, Stream: Stream{Properties: Options{"size": dbus.MakeVariant(size)}}}, nil
}

func openX11Input() (*xgb.Conn, error) {
	conn, err := xgb.NewConn()
	if err != nil {
		return nil, errors.New("session X authentication failed")
	}
	if err = xtest.Init(conn); err != nil {
		conn.Close()
		return nil, errors.New("session XTest extension unavailable")
	}
	return conn, nil
}
func x11Notify(conn *xgb.Conn, method string, args ...any) error {
	root := xproto.Setup(conn).DefaultScreen(conn).Root
	fake := func(kind, detail byte, x, y int16) error {
		return xtest.FakeInputChecked(conn, kind, detail, 0, root, x, y, 0).Check()
	}
	switch method {
	case "NotifyPointerMotionAbsolute":
		return fake(xproto.MotionNotify, 0, int16(args[1].(float64)), int16(args[2].(float64)))
	case "NotifyKeyboardKeycode":
		code := args[0].(int32) + 8
		if code < 8 || code > 255 {
			return errors.New("invalid X keycode")
		}
		kind := byte(xproto.KeyRelease)
		if args[1].(uint32) == 1 {
			kind = xproto.KeyPress
		}
		return fake(kind, byte(code), 0, 0)
	case "NotifyPointerButton":
		button := map[int32]byte{272: 1, 273: 3, 274: 2}[args[0].(int32)]
		if button == 0 {
			return errors.New("invalid X button")
		}
		kind := byte(xproto.ButtonRelease)
		if args[1].(uint32) == 1 {
			kind = xproto.ButtonPress
		}
		return fake(kind, button, 0, 0)
	case "NotifyPointerAxisDiscrete":
		steps := args[1].(int32)
		button := byte(5)
		if steps < 0 {
			button = 4
			steps = -steps
		}
		if args[0].(uint32) == 1 {
			button += 2
		}
		if steps > 32 {
			return errors.New("invalid X wheel")
		}
		for i := int32(0); i < steps; i++ {
			if err := fake(xproto.ButtonPress, button, 0, 0); err != nil {
				return err
			}
			if err := fake(xproto.ButtonRelease, button, 0, 0); err != nil {
				return err
			}
		}
		return nil
	}
	return errors.New("unsupported X input")
}
