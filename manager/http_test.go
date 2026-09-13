package manager

import (
	"crypto/tls"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestHTTPBoundary(t *testing.T) {
	handler, err := NewAPI(nil, "https://desktops.donkeywork.dev")
	if err != nil {
		t.Fatal(err)
	}
	for _, tc := range []struct {
		method, path, origin, body string
		tls                        bool
		want                       int
	}{
		{"GET", "/api/v1/devices", "", "", false, 403},
		{"GET", "/api/v1/devices", "https://evil.example", "", true, 403},
		{"POST", "/api/v1/devices", "", "{}", true, 403},
		{"POST", "/api/v1/devices", "https://desktops.donkeywork.dev", `{"unexpected":true}`, true, 400},
		{"POST", "/api/v1/enroll", "", `{"code":"ABCD-EFGH","csrPEM":"x","extra":true}`, true, 400},
		{"POST", "/api/v1/enroll", "", `{} {}`, true, 400},
	} {
		r := httptest.NewRequest(tc.method, "https://desktops.donkeywork.dev"+tc.path, strings.NewReader(tc.body))
		r.TLS = nil
		if tc.tls {
			r.TLS = &tls.ConnectionState{}
		}
		r.Header.Set("Content-Type", "application/json")
		r.Header.Set("Origin", tc.origin)
		w := httptest.NewRecorder()
		handler.ServeHTTP(w, r)
		if w.Code != tc.want {
			t.Errorf("%s: got %d want %d", tc.path, w.Code, tc.want)
		}
	}
}
func TestEnrollmentRateLimit(t *testing.T) {
	handler, _ := NewAPI(nil, "https://desktops.donkeywork.dev")
	api := handler.(*API)
	for n := 0; n < 5; n++ {
		if !api.permit("192.0.2.1:1234") {
			t.Fatal("early limit")
		}
	}
	if api.permit("192.0.2.1:9999") {
		t.Fatal("port change bypassed rate limit")
	}
	if !api.permit("192.0.2.2:1234") {
		t.Fatal("other address incorrectly limited")
	}
}
