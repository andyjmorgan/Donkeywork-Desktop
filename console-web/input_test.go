package main

import (
	"encoding/binary"
	"encoding/json"
	"net"
	"testing"
	"time"
)

func testInputClient() (*inputClient, chan map[string]any) {
	out := make(chan map[string]any, 16)
	return &inputClient{done: make(chan struct{}), close: func() {}, send: func(data []byte) error {
		var v map[string]any
		if err := json.Unmarshal(data, &v); err != nil {
			return err
		}
		out <- v
		return nil
	}}, out
}
func receiveInput(t *testing.T, out <-chan map[string]any, want string) map[string]any {
	t.Helper()
	select {
	case value := <-out:
		if value["type"] != want {
			t.Fatalf("want %s, got %v", want, value)
		}
		return value
	case <-time.After(time.Second):
		t.Fatal("input reply timeout")
		return nil
	}
}
func helperRead(t *testing.T, c net.Conn) map[string]any {
	t.Helper()
	_ = c.SetReadDeadline(time.Now().Add(time.Second))
	data, err := readRecord(c, inputLimit)
	if err != nil {
		t.Fatal(err)
	}
	var v map[string]any
	if err = json.Unmarshal(data, &v); err != nil {
		t.Fatal(err)
	}
	return v
}
func helperWrite(t *testing.T, c net.Conn, v any) {
	t.Helper()
	data, err := json.Marshal(v)
	if err != nil {
		t.Fatal(err)
	}
	_ = c.SetWriteDeadline(time.Now().Add(time.Second))
	var prefix [4]byte
	binary.BigEndian.PutUint32(prefix[:], uint32(len(data)))
	if _, err = c.Write(append(prefix[:], data...)); err != nil {
		t.Fatal(err)
	}
}
func acquireInput(t *testing.T, b *inputBridge, c *inputClient, out chan map[string]any, helper net.Conn, generation string) {
	t.Helper()
	b.submit(c, []byte(`{"type":"acquire"}`))
	if helperRead(t, helper)["type"] != "acquire" {
		t.Fatal("expected acquire")
	}
	helperWrite(t, helper, map[string]any{"type": "ready", "protocol": "dwconsole.input", "version": "0.2.0", "generation": generation, "width": 1920, "height": 1080, "leaseMs": 1000})
	receiveInput(t, out, "ready")
}
func TestInputOwnershipReleaseAndReacquire(t *testing.T) {
	local, helper := net.Pipe()
	defer helper.Close()
	b := newInputBridge(local, 1920, 1080)
	defer b.stop()
	first, out := testInputClient()
	second, out2 := testInputClient()
	acquireInput(t, b, first, out, helper, "a")
	b.submit(second, []byte(`{"type":"acquire"}`))
	receiveInput(t, out2, "unavailable")
	b.submit(first, []byte(`{"type":"event","generation":"a","sequence":1,"event":{"type":"move","x":12,"y":34}}`))
	event := helperRead(t, helper)
	if _, exists := event["type"]; exists {
		t.Fatal("outer type reached helper")
	}
	if event["sequence"] != float64(1) {
		t.Fatal(event)
	}
	helperWrite(t, helper, map[string]any{"type": "ack", "sequence": 1, "accepted": true})
	receiveInput(t, out, "ack")
	b.submit(first, []byte(`{"type":"release"}`))
	release := helperRead(t, helper)
	if release["sequence"] != float64(2) || release["event"].(map[string]any)["type"] != "release" {
		t.Fatal(release)
	}
	helperWrite(t, helper, map[string]any{"type": "ack", "sequence": 2, "accepted": true})
	receiveInput(t, out, "released")
	acquireInput(t, b, second, out2, helper, "b")
	helperWrite(t, helper, map[string]any{"type": "unavailable", "reason": "control expired"})
	receiveInput(t, out2, "unavailable")
	b.submit(first, []byte(`{"type":"acquire"}`))
	helperRead(t, helper)
	// A stale event rejected after the old expiry can precede the new ready.
	helperWrite(t, helper, map[string]any{"type": "unavailable", "reason": "control expired"})
	helperWrite(t, helper, map[string]any{"type": "ready", "protocol": "dwconsole.input", "version": "0.2.0", "generation": "c", "width": 1920, "height": 1080, "leaseMs": 1000})
	receiveInput(t, out, "ready")
}
func TestInputBadSequenceReleasesOwner(t *testing.T) {
	for _, body := range []string{
		`{"type":"event","generation":"old","sequence":1,"event":{"type":"reset"}}`,
		`{"type":"event","generation":"a","sequence":2,"event":{"type":"reset"}}`,
		`{"type":"event","generation":"a","sequence":1,"event":{"type":"move","x":1920,"y":0}}`,
		`{"type":"event","generation":"a","sequence":1,"event":{"type":"key","hid":4,"down":true,"extra":1}}`,
		`{"type":"release","unexpected":true}`,
	} {
		t.Run(body, func(t *testing.T) {
			local, helper := net.Pipe()
			defer helper.Close()
			b := newInputBridge(local, 1920, 1080)
			defer b.stop()
			client, out := testInputClient()
			acquireInput(t, b, client, out, helper, "a")
			b.submit(client, []byte(body))
			release := helperRead(t, helper)
			if release["event"].(map[string]any)["type"] != "release" || release["sequence"] != float64(1) {
				t.Fatal("malformed input reached helper", release)
			}
			helperWrite(t, helper, map[string]any{"type": "ack", "sequence": 1, "accepted": true})
			select {
			case <-client.done:
			case <-time.After(time.Second):
				t.Fatal("malformed controller not closed")
			}
		})
	}
}
func TestInputTopologyMismatchClosesHelper(t *testing.T) {
	local, helper := net.Pipe()
	defer helper.Close()
	b := newInputBridge(local, 1920, 1080)
	defer b.stop()
	client, out := testInputClient()
	b.submit(client, []byte(`{"type":"acquire"}`))
	helperRead(t, helper)
	helperWrite(t, helper, map[string]any{"type": "ready", "protocol": "dwconsole.input", "version": "0.2.0", "generation": "a", "width": 1280, "height": 720, "leaseMs": 1000})
	receiveInput(t, out, "unavailable")
	select {
	case <-b.done:
	case <-time.After(time.Second):
		t.Fatal("mismatched input remained open")
	}
}
func TestInputClosedControllerReleasesWithoutRenewal(t *testing.T) {
	local, helper := net.Pipe()
	defer helper.Close()
	b := newInputBridge(local, 1920, 1080)
	defer b.stop()
	client, out := testInputClient()
	acquireInput(t, b, client, out, helper, "a")
	client.stop()
	release := helperRead(t, helper)
	if release["event"].(map[string]any)["type"] != "release" {
		t.Fatal(release)
	}
	helperWrite(t, helper, map[string]any{"type": "ack", "sequence": 1, "accepted": true})
}

func TestInputConcurrentAcquisitionAndStaleQueue(t *testing.T) {
	local, helper := net.Pipe()
	defer helper.Close()
	b := newInputBridge(local, 1920, 1080)
	defer b.stop()
	first, out := testInputClient()
	second, out2 := testInputClient()
	b.submit(first, []byte(`{"type":"acquire"}`))
	helperRead(t, helper)
	b.submit(second, []byte(`{"type":"acquire"}`))
	helperWrite(t, helper, map[string]any{"type": "ready", "protocol": "dwconsole.input", "version": "0.2.0", "generation": "a", "width": 1920, "height": 1080, "leaseMs": 1000})
	receiveInput(t, out, "ready")
	receiveInput(t, out2, "unavailable")
	b.queue <- inputCommand{client: first, data: []byte(`{"type":"event","generation":"a","sequence":1,"event":{"type":"renew"}}`), at: time.Now().Add(-time.Second)}
	release := helperRead(t, helper)
	if release["event"].(map[string]any)["type"] != "release" {
		t.Fatal("stale renew reached helper", release)
	}
	helperWrite(t, helper, map[string]any{"type": "ack", "sequence": 1, "accepted": true})
}

func TestInputUnsolicitedAckFailsClosed(t *testing.T) {
	local, helper := net.Pipe()
	defer helper.Close()
	b := newInputBridge(local, 1920, 1080)
	defer b.stop()
	helperWrite(t, helper, map[string]any{"type": "ack", "sequence": 1, "accepted": true})
	select {
	case <-b.done:
	case <-time.After(time.Second):
		t.Fatal("unsolicited ACK accepted")
	}
}

func TestInputDeniedAcquireKeepsConnectionForRetry(t *testing.T) {
	local, helper := net.Pipe()
	defer helper.Close()
	b := newInputBridge(local, 1920, 1080)
	defer b.stop()
	client, out := testInputClient()
	b.submit(client, []byte(`{"type":"acquire"}`))
	request := helperRead(t, helper)
	requestID, ok := request["requestId"].(string)
	if !ok || len(requestID) != 32 {
		t.Fatal("missing bounded correlation ID", request)
	}
	// An unrelated old expiry must not be mistaken for the actual denial.
	helperWrite(t, helper, map[string]any{"type": "unavailable", "reason": "control expired", "requestId": "old"})
	helperWrite(t, helper, map[string]any{"type": "unavailable", "reason": "control is in use", "requestId": requestID})
	receiveInput(t, out, "unavailable")
	select {
	case <-b.done:
		t.Fatal("denial closed reusable helper connection")
	default:
	}
	b.submit(client, []byte(`{"type":"acquire"}`))
	next := helperRead(t, helper)
	if next["requestId"] == requestID {
		t.Fatal("acquire request ID reused")
	}
	helperWrite(t, helper, map[string]any{"type": "ready", "protocol": "dwconsole.input", "version": "0.2.0", "generation": "new", "width": 1920, "height": 1080, "leaseMs": 1000})
	receiveInput(t, out, "ready")
}

func TestInputCapturePauseReleasesButKeepsChannel(t *testing.T) {
	local, helper := net.Pipe()
	defer helper.Close()
	b := newInputBridge(local, 1920, 1080)
	defer b.stop()
	client, out := testInputClient()
	acquireInput(t, b, client, out, helper, "a")
	b.pause()
	release := helperRead(t, helper)
	if release["event"].(map[string]any)["type"] != "release" {
		t.Fatal(release)
	}
	helperWrite(t, helper, map[string]any{"type": "ack", "sequence": 1, "accepted": true})
	receiveInput(t, out, "unavailable")
	b.submit(client, []byte(`{"type":"acquire"}`))
	receiveInput(t, out, "unavailable")
	select {
	case <-client.done:
		t.Fatal("pause closed data channel")
	default:
	}
	b.resume()
	acquireInput(t, b, client, out, helper, "b")
}
