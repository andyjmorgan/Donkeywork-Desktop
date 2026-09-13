package desktopportal

import (
	"net/http/httptest"
	"testing"
)

func TestPortalInputValidation(t *testing.T) {
	for _, test := range []struct {
		json  string
		valid bool
	}{
		{`{"type":"move","x":1919,"y":1079}`, true},
		{`{"type":"move","x":1920,"y":0}`, false},
		{`{"type":"move","x":-1,"y":0}`, false},
		{`{"type":"button","x":100,"y":100,"button":1,"down":true}`, true},
		{`{"type":"button","x":100,"y":100,"button":4,"down":true}`, false},
		{`{"type":"wheel","x":100,"y":100,"vertical":32,"horizontal":-32}`, true},
		{`{"type":"wheel","x":100,"y":100,"vertical":33,"horizontal":0}`, false},
		{`{"type":"wheel","x":100,"y":100,"vertical":0,"horizontal":0}`, false},
		{`{"type":"key","hid":4,"down":true}`, true},
		{`{"type":"key","hid":65535,"down":true}`, false},
		{`{"type":"key","hid":4,"down":true,"x":2}`, false},
		{`{"type":"renew"}`, true},
		{`{"type":"renew","hid":4}`, false},
		{`{"type":"reset","unknown":true}`, false},
		{`{"type":"reset"} {}`, false},
	} {
		var event inputEvent
		actual := strictJSON([]byte(test.json), &event) == nil && event.valid(1920, 1080)
		if actual != test.valid {
			t.Errorf("validation %s: got %v", test.json, actual)
		}
	}
	for hid, want := range map[uint16]int32{4: 30, 29: 44, 30: 2, 39: 11, 40: 28, 79: 106, 224: 29, 231: 126} {
		got, ok := evdev(hid)
		if !ok || got != want {
			t.Errorf("HID %d got %d", hid, got)
		}
	}
}

func TestConsoleCannotTerminateOrResizeDesktop(t *testing.T) {
	b := &portalWeb{id: "0123456789abcdef0123456789abcdef", width: 1920, height: 1080}
	for _, test := range []struct {
		method, path, origin string
		code                 int
	}{
		{"GET", "/api/desktops", "", 200},
		{"GET", "/api/desktops/0123456789abcdef0123456789abcdef/status", "", 200},
		{"POST", "/api/desktops/0123456789abcdef0123456789abcdef/close", "http://localhost", 409},
		{"POST", "/api/desktops/0123456789abcdef0123456789abcdef/resize", "http://localhost", 409},
		{"POST", "/api/desktops/0123456789abcdef0123456789abcdef/reconnect", "http://evil", 403},
		{"GET", "/api/desktops/ffffffffffffffffffffffffffffffff/status", "", 404},
	} {
		req := httptest.NewRequest(test.method, "http://localhost"+test.path, nil)
		req.Header.Set("Origin", test.origin)
		out := httptest.NewRecorder()
		b.ServeHTTP(out, req)
		if out.Code != test.code {
			t.Errorf("%s: got %d", test.path, out.Code)
		}
	}
}
