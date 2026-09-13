package manager

import (
	"crypto/sha256"
	"donkeywork-desktop/manager/bridgewire"
	"encoding/json"
	"net/http"
	"sync"
	"time"

	"github.com/gorilla/websocket"
)

type Presence struct {
	Online   bool   `json:"online"`
	Hostname string `json:"hostname,omitempty"`
	Platform string `json:"platform,omitempty"`
	Broker   bool   `json:"broker"`
}
type liveDevice struct {
	conn               *websocket.Conn
	last               time.Time
	hostname, platform string
	broker             bool
	writeMu            sync.Mutex
	pending            map[string]chan bridgewire.Message
	done               chan struct{}
	streams            map[string]*relayViewer
}
type Hub struct {
	mu   sync.Mutex
	live map[string]*liveDevice
}

func (h *Hub) Snapshot(id string) Presence {
	h.mu.Lock()
	defer h.mu.Unlock()
	d := h.live[id]
	if d == nil {
		return Presence{}
	}
	return Presence{time.Since(d.last) < 45*time.Second, d.hostname, d.platform, d.broker}
}
func (h *Hub) Disconnect(id string) {
	h.mu.Lock()
	defer h.mu.Unlock()
	if d := h.live[id]; d != nil {
		delete(h.live, id)
		d.conn.Close()
	}
}
func (a *API) DeviceHandler(w http.ResponseWriter, r *http.Request) {
	if r.URL.Path != "/connect" || r.Method != "GET" || r.Header.Get("Origin") != "" || r.TLS == nil || len(r.TLS.VerifiedChains) == 0 {
		http.Error(w, "device authentication required", 403)
		return
	}
	leaf := r.TLS.PeerCertificates[0]
	fp := sha256.Sum256(leaf.Raw)
	var id string
	if a.store.pool.QueryRow(r.Context(), `SELECT id::text FROM devices WHERE certificate_sha256=$1 AND claimed_at IS NOT NULL AND revoked_at IS NULL`, fp[:]).Scan(&id) != nil {
		http.Error(w, "device denied", 403)
		return
	}
	if len(leaf.URIs) != 1 || leaf.URIs[0].String() != "spiffe://desktops.donkeywork.dev/device/"+id {
		http.Error(w, "device denied", 403)
		return
	}
	conn, err := (&websocket.Upgrader{HandshakeTimeout: 5 * time.Second}).Upgrade(w, r, nil)
	if err != nil {
		return
	}
	conn.SetReadLimit(bridgewire.Limit)
	defer conn.Close()
	// A connection is not online until a valid hello/heartbeat arrives.
	d := &liveDevice{conn: conn, pending: make(map[string]chan bridgewire.Message), done: make(chan struct{}), streams: make(map[string]*relayViewer)}
	a.hub.mu.Lock()
	if a.hub.live == nil {
		a.hub.live = map[string]*liveDevice{}
	}
	if previous := a.hub.live[id]; previous != nil {
		previous.conn.Close()
	}
	a.hub.live[id] = d
	a.hub.mu.Unlock()
	defer func() {
		close(d.done)
		a.hub.mu.Lock()
		defer a.hub.mu.Unlock()
		if a.hub.live[id] == d {
			delete(a.hub.live, id)
		}
	}()
	for {
		conn.SetReadDeadline(time.Now().Add(45 * time.Second))
		kind, data, err := conn.ReadMessage()
		if err != nil {
			return
		}
		if time.Now().After(leaf.NotAfter) {
			return
		}
		if kind == websocket.BinaryMessage {
			if len(data) < 33 || len(data) > 65569 {
				return
			}
			a.hub.mu.Lock()
			viewer := d.streams[string(data[:32])]
			current := a.hub.live[id] == d
			last := d.last
			a.hub.mu.Unlock()
			if time.Since(last) > 45*time.Second {
				return
			}
			if current && viewer != nil {
				viewer.receive(data[32], data[33:])
			}
			continue
		}
		if kind != websocket.TextMessage {
			return
		}
		if time.Now().After(leaf.NotAfter) {
			return
		}
		var msg bridgewire.Message
		if json.Unmarshal(data, &msg) != nil || msg.Version != 1 {
			return
		}
		if msg.Type == "response" {
			a.hub.mu.Lock()
			ch := d.pending[msg.ID]
			current := a.hub.live[id] == d
			a.hub.mu.Unlock()
			if current && ch != nil && msg.Status >= 200 && msg.Status <= 599 && json.Valid(msg.Body) {
				select {
				case ch <- msg:
				default:
				}
			}
			continue
		}
		if msg.Type != "heartbeat" || len(msg.Hostname) == 0 || len(msg.Hostname) > 253 || len(msg.Platform) > 80 || (!d.last.IsZero() && time.Since(d.last) < time.Second) {
			return
		}
		result, err := a.store.pool.Exec(r.Context(), `UPDATE devices SET last_seen_at=now() WHERE id=$1 AND revoked_at IS NULL AND certificate_sha256=$2`, id, fp[:])
		if err != nil || result.RowsAffected() != 1 {
			return
		}
		a.hub.mu.Lock()
		if a.hub.live[id] != d {
			a.hub.mu.Unlock()
			return
		}
		d.last = time.Now()
		d.hostname = msg.Hostname
		d.platform = msg.Platform
		d.broker = msg.Broker
		a.hub.mu.Unlock()
		d.writeMu.Lock()
		conn.SetWriteDeadline(time.Now().Add(5 * time.Second))
		err = conn.WriteJSON(map[string]any{"version": 1, "type": "ack", "deviceId": id})
		d.writeMu.Unlock()
		if err != nil {
			return
		}
	}
}
