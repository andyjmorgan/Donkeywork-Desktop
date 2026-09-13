package desktopportal

import (
	"github.com/godbus/dbus/v5"
	"testing"
)

func TestResponse(t *testing.T) {
	if _, e := response([]any{uint32(0), map[string]dbus.Variant{}}); e != nil {
		t.Fatal(e)
	}
	for _, b := range [][]any{nil, {uint32(1), map[string]dbus.Variant{}}, {uint32(2), map[string]dbus.Variant{}}, {0, map[string]dbus.Variant{}}, {uint32(0), "bad"}} {
		if _, e := response(b); e == nil {
			t.Fatal("invalid response accepted")
		}
	}
}
func TestToken(t *testing.T) {
	a, e := token()
	if e != nil {
		t.Fatal(e)
	}
	b, _ := token()
	if a == b || !dbus.ObjectPath("/"+a).IsValid() {
		t.Fatal("bad request identity")
	}
}
