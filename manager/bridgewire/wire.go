// Package bridgewire defines the bounded, allowlisted local broker adapter.
package bridgewire

import (
	"encoding/json"
	"regexp"
)

const Limit = 512 * 1024

// Binary records: 32 ASCII hex stream ID, one kind byte, opaque payload.
const Video byte = 1
const Input byte = 2
const Feedback byte = 3
const Closed byte = 4

func Packet(id string, kind byte, payload []byte) []byte {
	b := make([]byte, 33+len(payload))
	copy(b, id)
	b[32] = kind
	copy(b[33:], payload)
	return b
}

type Message struct {
	Version  int             `json:"version"`
	Type     string          `json:"type"`
	ID       string          `json:"id,omitempty"`
	Method   string          `json:"method,omitempty"`
	Path     string          `json:"path,omitempty"`
	Body     json.RawMessage `json:"body,omitempty"`
	Status   int             `json:"status,omitempty"`
	Hostname string          `json:"hostname,omitempty"`
	Platform string          `json:"platform,omitempty"`
	DeviceID string          `json:"deviceId,omitempty"`
	Broker   bool            `json:"broker,omitempty"`
}

var action = regexp.MustCompile(`^/api/desktops/[0-9a-f]{32}/(status|offer|resize|reconnect|close)$`)

func Allowed(method, path string) bool {
	if path == "/api/environments" {
		return method == "GET"
	}
	if path == "/api/desktops" {
		return method == "GET" || method == "POST"
	}
	m := action.FindStringSubmatch(path)
	return m != nil && ((m[1] == "status" && method == "GET") || (m[1] != "status" && method == "POST"))
}
