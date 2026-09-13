package main

import (
	"bytes"
	"crypto/rand"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"errors"
	"io"
	"net"
	"sync"
	"sync/atomic"
	"time"

	"github.com/pion/webrtc/v4"
)

const inputLimit = 4096
const inputDeadline = 500 * time.Millisecond

type inputClient struct {
	send  func([]byte) error
	close func()
	done  chan struct{}
	once  sync.Once
}

func (c *inputClient) stop() { c.once.Do(func() { close(c.done); go c.close() }) }
func (c *inputClient) reply(v any) {
	data, _ := json.Marshal(v)
	if c.send(data) != nil {
		c.stop()
	}
}
func (c *inputClient) unavailable(reason string) {
	c.reply(map[string]any{"type": "unavailable", "reason": reason})
}

type inputCommand struct {
	epoch  uint64
	client *inputClient
	data   []byte
	at     time.Time
}
type helperMessage struct {
	data []byte
	err  error
}
type inputEnvelope struct {
	Type       string          `json:"type"`
	Generation string          `json:"generation,omitempty"`
	Sequence   uint64          `json:"sequence,omitempty"`
	Event      json.RawMessage `json:"event,omitempty"`
}
type inputReply struct {
	RequestID  string `json:"requestId,omitempty"`
	Type       string `json:"type"`
	Protocol   string `json:"protocol"`
	Version    string `json:"version"`
	Generation string `json:"generation"`
	Width      uint32 `json:"width"`
	Height     uint32 `json:"height"`
	LeaseMS    uint32 `json:"leaseMs"`
	Sequence   uint64 `json:"sequence"`
	Accepted   bool   `json:"accepted"`
	Reason     string `json:"reason"`
}
type inputBridge struct {
	paused        atomic.Bool
	pauseEpoch    atomic.Uint64
	conn          net.Conn
	width, height uint32
	queue         chan inputCommand
	replies       chan helperMessage
	done          chan struct{}
	once          sync.Once
	// Only run accesses these fields.
	owner      *inputClient
	generation string
	sequence   uint64
}

func newInputBridge(conn net.Conn, w, h uint32) *inputBridge {
	b := &inputBridge{conn: conn, width: w, height: h, queue: make(chan inputCommand, 32), replies: make(chan helperMessage, 1), done: make(chan struct{})}
	go b.read()
	go b.run()
	return b
}
func (b *inputBridge) stop()   { b.once.Do(func() { close(b.done); _ = b.conn.Close() }) }
func (b *inputBridge) pause()  { b.paused.Store(true); b.pauseEpoch.Add(1) }
func (b *inputBridge) resume() { b.paused.Store(false) }
func (b *inputBridge) read() {
	for {
		data, err := readRecord(b.conn, inputLimit)
		select {
		case b.replies <- helperMessage{data, err}:
		case <-b.done:
			return
		}
		if err != nil {
			return
		}
	}
}
func (b *inputBridge) submit(c *inputClient, data []byte) {
	if len(data) == 0 || len(data) > inputLimit {
		c.stop()
		return
	}
	command := inputCommand{client: c, data: append([]byte(nil), data...), at: time.Now(), epoch: b.pauseEpoch.Load()}
	select {
	case <-b.done:
		c.unavailable("Input unavailable")
	case b.queue <- command:
	default:
		c.stop()
	}
}
func strictInput(data []byte, out any) error {
	d := json.NewDecoder(bytes.NewReader(data))
	d.DisallowUnknownFields()
	if err := d.Decode(out); err != nil {
		return err
	}
	if d.Decode(new(any)) != io.EOF {
		return errors.New("trailing JSON")
	}
	return nil
}
func validateInputEvent(raw json.RawMessage, w, h uint32) bool {
	var e struct {
		Type   string  `json:"type"`
		X      *uint32 `json:"x"`
		Y      *uint32 `json:"y"`
		Button *uint8  `json:"button"`
		Vertical *int16 `json:"vertical"`
		Horizontal *int16 `json:"horizontal"`
		Down   *bool   `json:"down"`
		HID    *uint16 `json:"hid"`
	}
	if strictInput(raw, &e) != nil {
		return false
	}
	point := e.X != nil && e.Y != nil && *e.X < w && *e.Y < h
	switch e.Type {
	case "move":
		return point && e.Button == nil && e.Down == nil && e.HID == nil
	case "button":
		return point && e.Button != nil && *e.Button >= 1 && *e.Button <= 3 && e.Down != nil && e.HID == nil
	case "wheel":
		return point && e.Vertical != nil && e.Horizontal != nil &&
			(*e.Vertical != 0 || *e.Horizontal != 0) && abs16(*e.Vertical) <= 32 && abs16(*e.Horizontal) <= 32 &&
			e.Button == nil && e.Down == nil && e.HID == nil
	case "key":
		return e.HID != nil && e.Down != nil && e.X == nil && e.Y == nil && e.Button == nil
	case "renew", "reset":
		return e.X == nil && e.Y == nil && e.Button == nil && e.Down == nil && e.HID == nil
	}
	return false
}

func abs16(v int16) int32 { if v < 0 { return -int32(v) }; return int32(v) }
func (b *inputBridge) write(v any) error {
	data, err := json.Marshal(v)
	if err != nil {
		return err
	}
	if len(data) > inputLimit {
		return errors.New("input too large")
	}
	var prefix [4]byte
	binary.BigEndian.PutUint32(prefix[:], uint32(len(data)))
	if err = b.conn.SetWriteDeadline(time.Now().Add(inputDeadline)); err != nil {
		return err
	}
	// io.Copy handles short writes; a failed partial record permanently closes the helper connection.
	_, err = io.Copy(b.conn, bytes.NewReader(append(prefix[:], data...)))
	return err
}
func (b *inputBridge) response() (inputReply, []byte, error) {
	return b.responseUntil(time.Now().Add(inputDeadline))
}
func (b *inputBridge) responseUntil(deadline time.Time) (inputReply, []byte, error) {
	timer := time.NewTimer(time.Until(deadline))
	defer timer.Stop()
	select {
	case m := <-b.replies:
		var reply inputReply
		if m.err != nil {
			return reply, nil, m.err
		}
		if strictInput(m.data, &reply) != nil {
			return reply, nil, errors.New("invalid helper reply")
		}
		return reply, m.data, nil
	case <-b.done:
		return inputReply{}, nil, io.EOF
	case <-timer.C:
		return inputReply{}, nil, errors.New("helper timeout")
	}
}
func (b *inputBridge) clear(reason string) {
	if b.owner != nil && reason != "" {
		b.owner.unavailable(reason)
	}
	b.owner = nil
	b.generation = ""
	b.sequence = 0
}
func (b *inputBridge) release() error {
	if b.owner == nil {
		return nil
	}
	if err := b.write(map[string]any{"generation": b.generation, "sequence": b.sequence + 1, "event": map[string]string{"type": "release"}}); err != nil {
		return err
	}
	reply, _, err := b.response()
	if err != nil {
		return err
	}
	if reply.Type != "unavailable" && (reply.Type != "ack" || !reply.Accepted || reply.Sequence != b.sequence+1) {
		return errors.New("invalid release acknowledgement")
	}
	b.clear("")
	return nil
}
func (b *inputBridge) handle(command inputCommand) error {
	c := command.client
	select {
	case <-c.done:
		return nil
	default:
	}
	if time.Since(command.at) > inputDeadline {
		c.stop()
		return nil
	}
	var envelope inputEnvelope
	if strictInput(command.data, &envelope) != nil {
		c.stop()
		return nil
	}
	if envelope.Type != "release" && (b.paused.Load() || command.epoch != b.pauseEpoch.Load()) {
		c.unavailable("Capture is recovering")
		return nil
	}
	switch envelope.Type {
	case "acquire":
		if envelope.Generation != "" || envelope.Sequence != 0 || envelope.Event != nil {
			c.stop()
			return nil
		}
		if b.owner != nil {
			c.unavailable("Control is in use")
			return nil
		}
		b.owner = c
		var nonce [16]byte
		if _, err := rand.Read(nonce[:]); err != nil {
			return err
		}
		requestID := hex.EncodeToString(nonce[:])
		if err := b.write(map[string]string{"type": "acquire", "requestId": requestID}); err != nil {
			return err
		}
		// Expiry and rejection of an already-written stale event can each emit
		// unavailable. Consume both before the new acquire's ready, without
		// resetting the absolute budget or replaying any event.
		deadline := time.Now().Add(inputDeadline)
		var reply inputReply
		var data []byte
		var err error
		for {
			reply, data, err = b.responseUntil(deadline)
			if err != nil {
				return err
			}
			if reply.Type == "unavailable" && reply.RequestID == requestID {
				b.clear("Input unavailable")
				return nil
			}
			if reply.Type != "unavailable" {
				break
			}
		}
		if reply.Type != "ready" || reply.Protocol != "dwconsole.input" || reply.Version != "0.2.0" || reply.Generation == "" || reply.Width != b.width || reply.Height != b.height || reply.LeaseMS != 1000 {
			return errors.New("helper topology/protocol mismatch")
		}
		b.generation = reply.Generation
		b.sequence = 0
		if b.paused.Load() || command.epoch != b.pauseEpoch.Load() {
			if err := b.release(); err != nil {
				return err
			}
			c.unavailable("Capture is recovering")
			return nil
		}
		if c.send(data) != nil {
			c.stop()
		}
	case "event":
		if c != b.owner {
			c.unavailable("Control is not acquired")
			return nil
		}
		if envelope.Generation != b.generation || envelope.Sequence != b.sequence+1 || envelope.Sequence > 9007199254740991 || !validateInputEvent(envelope.Event, b.width, b.height) {
			c.stop()
			return nil
		}
		if err := b.write(map[string]any{"generation": envelope.Generation, "sequence": envelope.Sequence, "event": envelope.Event}); err != nil {
			return err
		}
		b.sequence = envelope.Sequence
		reply, data, err := b.response()
		if err != nil {
			return err
		}
		if reply.Type == "unavailable" {
			b.clear("Control expired")
			return nil
		}
		if reply.Type != "ack" || !reply.Accepted || reply.Sequence != b.sequence {
			return errors.New("invalid event acknowledgement")
		}
		if c.send(data) != nil {
			c.stop()
		}
	case "release":
		if envelope.Generation != "" || envelope.Sequence != 0 || envelope.Event != nil {
			c.stop()
			return nil
		}
		if c == b.owner {
			if err := b.release(); err != nil {
				return err
			}
		}
		c.reply(map[string]string{"type": "released"})
	default:
		c.stop()
	}
	return nil
}
func (b *inputBridge) run() {
	observedPause := b.pauseEpoch.Load()
	ticker := time.NewTicker(20 * time.Millisecond)
	defer ticker.Stop()
	defer b.stop()
	defer b.clear("Input unavailable")
	for {
		if epoch := b.pauseEpoch.Load(); epoch != observedPause {
			observedPause = epoch
			owner := b.owner
			if b.release() != nil {
				return
			}
			if owner != nil {
				owner.unavailable("Capture is recovering")
			}
		}
		if b.owner != nil {
			select {
			case <-b.owner.done:
				if b.release() != nil {
					return
				}
			default:
			}
		}
		select {
		case <-b.done:
			return
		case <-ticker.C:
		case command := <-b.queue:
			if b.handle(command) != nil {
				return
			}
		case message := <-b.replies:
			var reply inputReply
			if message.err != nil || strictInput(message.data, &reply) != nil || reply.Type != "unavailable" {
				return
			}
			b.clear("Control expired")
		}
	}
}
func (b *bridge) attachInput(v *viewer) {
	var mu sync.Mutex
	attached := false
	v.pc.OnDataChannel(func(dc *webrtc.DataChannel) {
		mu.Lock()
		valid := !attached && b.input != nil && dc.Label() == "dwconsole.input" && dc.Ordered() && dc.MaxRetransmits() == nil && dc.MaxPacketLifeTime() == nil
		if valid {
			attached = true
		}
		mu.Unlock()
		if !valid {
			_ = dc.Close()
			return
		}
		c := &inputClient{send: func(data []byte) error {
			if dc.BufferedAmount() > 16*1024 {
				return errors.New("input feedback congested")
			}
			return dc.SendText(string(data))
		}, close: func() { _ = dc.Close() }, done: make(chan struct{})}
		dc.OnClose(c.stop)
		dc.OnMessage(func(message webrtc.DataChannelMessage) {
			if !message.IsString {
				c.stop()
				return
			}
			b.input.submit(c, message.Data)
		})
		go func() {
			select {
			case <-v.done:
				c.stop()
			case <-c.done:
			}
		}()
	})
}
