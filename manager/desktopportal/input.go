package desktopportal

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"time"
)

// USB keyboard usages mapped to Linux evdev codes, as required by the portal.
func evdev(h uint16) (int32, bool) {
	letters := []int32{30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17, 45, 21, 44}
	if h >= 4 && h <= 29 {
		return letters[h-4], true
	}
	if h >= 30 && h <= 38 {
		return int32(h - 28), true
	}
	codes := map[uint16]int32{39: 11, 40: 28, 41: 1, 42: 14, 43: 15, 44: 57, 45: 12, 46: 13, 47: 26, 48: 27, 49: 43, 51: 39, 52: 40, 53: 41, 54: 51, 55: 52, 56: 53, 57: 58, 73: 110, 74: 102, 75: 104, 76: 111, 77: 107, 78: 109, 79: 106, 80: 105, 81: 108, 82: 103, 224: 29, 225: 42, 226: 56, 227: 125, 228: 97, 229: 54, 230: 100, 231: 126}
	c, ok := codes[h]
	return c, ok
}

type inputEvent struct {
	Type       string  `json:"type"`
	X          *uint32 `json:"x,omitempty"`
	Y          *uint32 `json:"y,omitempty"`
	Button     *uint8  `json:"button,omitempty"`
	Down       *bool   `json:"down,omitempty"`
	HID        *uint16 `json:"hid,omitempty"`
	Vertical   *int16  `json:"vertical,omitempty"`
	Horizontal *int16  `json:"horizontal,omitempty"`
}

func strictJSON(b []byte, v any) error {
	d := json.NewDecoder(bytes.NewReader(b))
	d.DisallowUnknownFields()
	if err := d.Decode(v); err != nil {
		return err
	}
	if d.Decode(new(any)) != io.EOF {
		return errors.New("trailing JSON")
	}
	return nil
}
func (e inputEvent) valid(w, h uint32) bool {
	point := e.X != nil && e.Y != nil && *e.X < w && *e.Y < h
	noPoint := e.X == nil && e.Y == nil
	noWheel := e.Vertical == nil && e.Horizontal == nil
	switch e.Type {
	case "move":
		return point && e.Button == nil && e.Down == nil && e.HID == nil && noWheel
	case "button":
		return point && e.Button != nil && *e.Button >= 1 && *e.Button <= 3 && e.Down != nil && e.HID == nil && noWheel
	case "key":
		if e.HID == nil {
			return false
		}
		_, ok := evdev(*e.HID)
		return ok && noPoint && e.Button == nil && e.Down != nil && noWheel
	case "wheel":
		return point && e.Button == nil && e.Down == nil && e.HID == nil && e.Vertical != nil && e.Horizontal != nil && *e.Vertical >= -32 && *e.Vertical <= 32 && *e.Horizontal >= -32 && *e.Horizontal <= 32 && (*e.Vertical != 0 || *e.Horizontal != 0)
	case "renew", "reset":
		return noPoint && e.Button == nil && e.Down == nil && e.HID == nil && noWheel
	}
	return false
}

type inputLease struct {
	session       *Session
	generation    string
	sequence      uint64
	until         time.Time
	keys          map[int32]bool
	buttons       map[int32]bool
	width, height uint32
}

func (s *Session) notify(method string, args ...any) error {
	if s.x11 != nil {
		return x11Notify(s.x11, method, args...)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 250*time.Millisecond)
	defer cancel()
	return s.client.conn.Object(destination, path).CallWithContext(ctx, remote+"."+method, 0, append([]any{s.Handle, Options{}}, args...)...).Err
}
func (l *inputLease) reset() error {
	var failure error
	for key := range l.keys {
		if err := l.session.notify("NotifyKeyboardKeycode", key, uint32(0)); err != nil {
			failure = err
		}
	}
	for button := range l.buttons {
		if err := l.session.notify("NotifyPointerButton", button, uint32(0)); err != nil {
			failure = err
		}
	}
	l.keys = map[int32]bool{}
	l.buttons = map[int32]bool{}
	return failure
}
func (l *inputLease) release() error { l.generation = ""; l.until = time.Time{}; return l.reset() }
func (l *inputLease) event(e inputEvent) error {
	if !e.valid(l.width, l.height) {
		return errors.New("invalid input")
	}
	state := uint32(0)
	if e.Down != nil && *e.Down {
		state = 1
	}
	if e.X != nil {
		if err := l.session.notify("NotifyPointerMotionAbsolute", l.session.Stream.Node, float64(*e.X), float64(*e.Y)); err != nil {
			return err
		}
	}
	switch e.Type {
	case "key":
		code, _ := evdev(*e.HID)
		if state == 1 {
			l.keys[code] = true
		} else {
			delete(l.keys, code)
		}
		return l.session.notify("NotifyKeyboardKeycode", code, state)
	case "button":
		code := map[uint8]int32{1: 272, 2: 273, 3: 274}[*e.Button]
		if state == 1 {
			l.buttons[code] = true
		} else {
			delete(l.buttons, code)
		}
		return l.session.notify("NotifyPointerButton", code, state)
	case "wheel":
		if *e.Vertical != 0 {
			if err := l.session.notify("NotifyPointerAxisDiscrete", uint32(0), int32(*e.Vertical)); err != nil {
				return err
			}
		}
		if *e.Horizontal != 0 {
			return l.session.notify("NotifyPointerAxisDiscrete", uint32(1), int32(*e.Horizontal))
		}
	case "reset":
		return l.reset()
	}
	return nil
}
