package bridgewire

import "testing"

func TestRoutes(t *testing.T) {
	for _, path := range []string{"/api/desktops", "/api/environments", "/api/desktops/0123456789abcdef0123456789abcdef/status"} {
		if !Allowed("GET", path) {
			t.Fatal(path)
		}
	}
	for _, path := range []string{"/api/desktops/../../exec", "/api/desktops?x=1", "/api/environments", "/api/desktops/0123456789abcdef0123456789abcdef/status"} {
		if Allowed("POST", path) {
			t.Fatal(path)
		}
	}
	if Allowed("DELETE", "/api/desktops") {
		t.Fatal("unexpected method")
	}
}
