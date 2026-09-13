package manager

import (
	"context"
	"crypto/rand"
	"donkeywork-desktop/manager/bridgewire"
	"encoding/hex"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"regexp"
	"strings"
	"time"
)

var brokerRoute = regexp.MustCompile(`^/api/v1/devices/([0-9a-f-]{36})/broker/(.+)$`)

func (h *Hub) request(ctx context.Context, device string, msg bridgewire.Message) (bridgewire.Message, error) {
	var nonce [16]byte
	if _, err := rand.Read(nonce[:]); err != nil {
		return bridgewire.Message{}, err
	}
	if msg.ID == "" {
		msg.ID = hex.EncodeToString(nonce[:])
	}
	msg.Version = 1
	if msg.Type == "" {
		msg.Type = "request"
	}
	h.mu.Lock()
	d := h.live[device]
	if d == nil || !d.broker || time.Since(d.last) >= 45*time.Second || len(d.pending) >= 8 {
		h.mu.Unlock()
		return bridgewire.Message{}, errors.New("broker unavailable")
	}
	if msg.Type == "media.open" && d.streams[msg.ID] == nil {
		h.mu.Unlock()
		return bridgewire.Message{}, errors.New("media connection replaced")
	}
	ch := make(chan bridgewire.Message, 1)
	d.pending[msg.ID] = ch
	h.mu.Unlock()
	defer func() { h.mu.Lock(); delete(d.pending, msg.ID); h.mu.Unlock() }()
	d.writeMu.Lock()
	d.conn.SetWriteDeadline(time.Now().Add(5 * time.Second))
	err := d.conn.WriteJSON(msg)
	d.writeMu.Unlock()
	if err != nil {
		return bridgewire.Message{}, err
	}
	select {
	case reply := <-ch:
		h.mu.Lock()
		current := h.live[device] == d
		h.mu.Unlock()
		if current {
			return reply, nil
		}
	case <-d.done:
	case <-ctx.Done():
	}
	return bridgewire.Message{}, errors.New("broker request interrupted")
}

func (a *API) session(w http.ResponseWriter, r *http.Request, match []string) {
	path := "/api/" + match[2]
	if !bridgewire.Allowed(r.Method, path) {
		respond(w, 404, map[string]string{"error": "unsupported broker operation"})
		return
	}
	if r.Method == "POST" && r.Header.Get("Origin") != a.origin {
		respond(w, 403, map[string]string{"error": "origin required"})
		return
	}
	var body json.RawMessage
	if r.Method == "POST" {
		if r.Header.Get("Content-Type") != "application/json" {
			respond(w, 400, map[string]string{"error": "JSON required"})
			return
		}
		b, err := io.ReadAll(http.MaxBytesReader(w, r.Body, 262144))
		defer r.Body.Close()
		if err != nil || !json.Valid(b) {
			respond(w, 400, map[string]string{"error": "invalid body"})
			return
		}
		body = b
	}
	ctx, cancel := context.WithTimeout(r.Context(), 14*time.Second)
	defer cancel()
	if strings.HasSuffix(path, "/offer") {
		if a.Media == nil {
			respond(w, 503, map[string]string{"error": "media relay not configured"})
			return
		}
		a.mediaOffer(w, r, match[1], path, body)
		return
	}
	reply, err := a.hub.request(ctx, match[1], bridgewire.Message{Method: r.Method, Path: path, Body: body})
	if err != nil {
		respond(w, 503, map[string]string{"error": "device broker unavailable; request was not replayed"})
		return
	}
	respond(w, reply.Status, reply.Body)
}
