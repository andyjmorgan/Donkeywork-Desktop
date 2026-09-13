package manager

import (
	"encoding/json"
	"encoding/pem"
	"errors"
	"io"
	"net"
	"net/http"
	"net/url"
	"regexp"
	"sync"
	"time"
)

type attempt struct {
	since time.Time
	count int
}
type API struct {
	DeviceURL    string
	Media        *mediaGateway
	hub          Hub
	store        *Store
	origin, host string
	mu           sync.Mutex
	attempts     map[string]attempt
	global       attempt
}

func NewAPI(store *Store, origin string) (http.Handler, error) {
	u, err := url.Parse(origin)
	if err != nil || u.Scheme != "https" || u.Host == "" || u.Path != "" || u.RawQuery != "" || u.Fragment != "" || u.User != nil {
		return nil, errors.New("exact HTTPS origin required")
	}
	return &API{store: store, origin: origin, host: u.Host, attempts: map[string]attempt{}}, nil
}
func (a *API) permit(address string) bool {
	ip, _, err := net.SplitHostPort(address)
	if err != nil {
		return false
	}
	a.mu.Lock()
	defer a.mu.Unlock()
	now := time.Now()
	if now.Sub(a.global.since) >= time.Minute {
		a.global = attempt{since: now}
	}
	if a.global.count >= 60 {
		return false
	}
	a.global.count++
	for key, v := range a.attempts {
		if now.Sub(v.since) >= time.Minute {
			delete(a.attempts, key)
		}
	}
	v, exists := a.attempts[ip]
	if !exists {
		if len(a.attempts) >= 1024 {
			return false
		}
		v = attempt{since: now}
	}
	if v.count >= 5 {
		return false
	}
	v.count++
	a.attempts[ip] = v
	return true
}
func respond(w http.ResponseWriter, status int, value any) {
	w.Header().Set("Content-Type", "application/json")
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("X-Content-Type-Options", "nosniff")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(value)
}
func decode(w http.ResponseWriter, r *http.Request, target any) error {
	if r.Header.Get("Content-Type") != "application/json" {
		return errors.New("JSON required")
	}
	r.Body = http.MaxBytesReader(w, r.Body, 24576)
	defer r.Body.Close()
	d := json.NewDecoder(r.Body)
	d.DisallowUnknownFields()
	if err := d.Decode(target); err != nil {
		return err
	}
	if d.Decode(new(any)) != io.EOF {
		return errors.New("trailing body")
	}
	return nil
}

var deviceAction = regexp.MustCompile(`^/api/v1/devices/([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})/(code|revoke|delete)$`)

func (a *API) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	fail := func(status int, message string) { respond(w, status, map[string]string{"error": message}) }
	if r.TLS == nil || r.Host != a.host {
		fail(403, "origin rejected")
		return
	}
	origin := r.Header.Get("Origin")
	if origin != "" && origin != a.origin {
		fail(403, "origin rejected")
		return
	}
	switch {
	case brokerRoute.MatchString(r.URL.Path):
		a.session(w, r, brokerRoute.FindStringSubmatch(r.URL.Path))
	case r.Method == "POST" && deviceAction.MatchString(r.URL.Path):
		if origin != a.origin {
			fail(403, "origin required")
			return
		}
		var body struct{}
		if decode(w, r, &body) != nil {
			fail(400, "invalid request")
			return
		}
		match := deviceAction.FindStringSubmatch(r.URL.Path)
		if match[2] == "delete" {
			err := a.store.DeleteRevoked(r.Context(), match[1])
			if err == ErrEnrollment {
				fail(409, "only revoked devices can be deleted")
				return
			}
			if err != nil {
				fail(503, "operation failed")
				return
			}
			respond(w, 200, map[string]string{"state": "deleted"})
			return
		}
		if match[2] == "revoke" {
			err := a.store.Revoke(r.Context(), match[1])
			if err == ErrEnrollment {
				fail(409, "device unavailable")
				return
			}
			if err != nil {
				fail(503, "operation failed")
				return
			}
			a.hub.Disconnect(match[1])
			respond(w, 200, map[string]string{"state": "revoked"})
			return
		}
		issued, err := a.store.Regenerate(r.Context(), match[1])
		if err == ErrEnrollment {
			fail(409, "device unavailable")
			return
		}
		if err != nil {
			fail(503, "operation failed")
			return
		}
		respond(w, 201, issued)
	case r.Method == "GET" && r.URL.Path == "/api/v1/devices":
		devices, err := a.store.List(r.Context())
		if err != nil {
			fail(503, "database unavailable")
			return
		}
		type row struct {
			Device
			Presence
		}
		rows := make([]row, 0, len(devices))
		for _, d := range devices {
			rows = append(rows, row{d, a.hub.Snapshot(d.ID)})
		}
		respond(w, 200, map[string]any{"devices": rows})
	case r.Method == "POST" && r.URL.Path == "/api/v1/devices":
		if origin != a.origin {
			fail(403, "origin required")
			return
		}
		var body struct {
			Name        string `json:"name"`
			Description string `json:"description"`
		}
		if decode(w, r, &body) != nil {
			fail(400, "invalid request")
			return
		}
		if len(body.Name) == 0 || len(body.Name) > 100 || len(body.Description) > 2000 {
			fail(400, "invalid device details")
			return
		}
		result, err := a.store.Create(r.Context(), body.Name, body.Description)
		if err != nil {
			fail(503, "device creation failed")
			return
		}
		respond(w, 201, result)
	case r.Method == "POST" && r.URL.Path == "/api/v1/enroll":
		if !a.permit(r.RemoteAddr) {
			w.Header().Set("Retry-After", "60")
			fail(429, "enrollment rate limited")
			return
		}
		var body struct {
			Code string `json:"code"`
			CSR  string `json:"csrPEM"`
		}
		if decode(w, r, &body) != nil {
			fail(400, "invalid request")
			return
		}
		id, cert, err := a.store.Claim(r.Context(), body.Code, body.CSR)
		if errors.Is(err, ErrEnrollment) {
			fail(403, "enrollment rejected")
			return
		}
		if err != nil {
			fail(503, "enrollment unavailable")
			return
		}
		respond(w, 201, map[string]string{"deviceId": id, "certificatePEM": cert, "deviceEndpoint": a.DeviceURL, "deviceCAPEM": string(pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: a.store.ca.Raw}))})
	default:
		fail(404, "not found")
	}
}
