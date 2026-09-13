package desktopportal

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"github.com/godbus/dbus/v5"
	"github.com/pion/rtp"
	"github.com/pion/webrtc/v4"
	"net"
	"net/http"
	"os"
	"os/exec"
	"os/user"
	"strings"
	"sync"
	"time"
)

type portalWeb struct {
	session       *Session
	id            string
	width, height uint32
	started       int64
	user          string
	mu            sync.Mutex
	viewer        *portalViewer
}
type portalViewer struct {
	pc     *webrtc.PeerConnection
	cancel context.CancelFunc
	done   chan struct{}
	once   sync.Once
	mu     sync.Mutex
	lease  inputLease
}

func (v *portalViewer) close() {
	v.once.Do(func() {
		v.cancel()
		v.mu.Lock()
		if err := v.lease.release(); err != nil {
			v.lease.session.Close()
		}
		v.mu.Unlock()
		close(v.done)
		go v.pc.Close()
	})
}
func reply(w http.ResponseWriter, status int, value any) {
	w.Header().Set("Content-Type", "application/json")
	w.Header().Set("Cache-Control", "no-store")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(value)
}

// Serve is an existing-session-only adapter. Its socket must live in a private,
// user-owned directory. No caller supplies network or capture destinations.
func (s *Session) Serve(ctx context.Context, socket string, gatewaySocket string) error {
	var size struct {
		Width  int32
		Height int32
	}
	if err := s.Stream.Properties["size"].Store(&size); err != nil || size.Width <= 0 || size.Height <= 0 || size.Width > 8192 || size.Height > 8192 {
		return errors.New("portal stream has no usable logical size")
	}
	id, err := token()
	if err != nil {
		return err
	}
	u, err := user.Current()
	if err != nil {
		return err
	}
	b := &portalWeb{session: s, id: strings.TrimPrefix(id, "dw"), width: uint32(size.Width), height: uint32(size.Height), started: time.Now().Unix(), user: u.Username}
	listener, err := listenUnix(socket)
	if err != nil {
		return err
	}
	defer listener.Close()
	if err = os.Chmod(socket, 0600); err != nil {
		return err
	}
	server := &http.Server{Handler: b, ReadHeaderTimeout: 3 * time.Second, ReadTimeout: 15 * time.Second, WriteTimeout: 15 * time.Second, IdleTimeout: 15 * time.Second}
	if gatewaySocket != "" {
		gateway, err := listenUnix(gatewaySocket)
		if err != nil {
			return err
		}
		defer gateway.Close()
		if err = os.Chmod(gatewaySocket, 0660); err != nil {
			return err
		}
		go func() { _ = server.Serve(gateway) }()
	}
	defer func() {
		b.mu.Lock()
		v := b.viewer
		b.mu.Unlock()
		if v != nil {
			v.close()
		}
	}()
	// The portal closes this session on logout or user revocation. Never keep
	// advertising a ready console after that signal or session-bus disconnect.
	signals := make(chan *dbus.Signal, 8)
	s.client.conn.Signal(signals)
	defer s.client.conn.RemoveSignal(signals)
	rules := []dbus.MatchOption{dbus.WithMatchSender(destination), dbus.WithMatchObjectPath(s.Handle), dbus.WithMatchInterface("org.freedesktop.portal.Session"), dbus.WithMatchMember("Closed")}
	if s.x11 == nil {
		if err = s.client.conn.AddMatchSignal(rules...); err != nil {
			return err
		}
	}
	defer s.client.conn.RemoveMatchSignal(rules...)
	stopped := make(chan struct{})
	defer close(stopped)
	go func() {
		for {
			select {
			case <-ctx.Done():
				server.Close()
				return
			case <-s.client.conn.Context().Done():
				server.Close()
				return
			case signal := <-signals:
				if signal != nil && signal.Path == s.Handle && signal.Name == "org.freedesktop.portal.Session.Closed" {
					server.Close()
					return
				}
			case <-stopped:
				return
			}
		}
	}()
	fmt.Println(`{"state":"console_adapter_ready"}`)
	err = server.Serve(listener)
	if errors.Is(err, http.ErrServerClosed) {
		return nil
	}
	return err
}
func (b *portalWeb) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if r.Host != "localhost" || (r.Method == "POST" && r.Header.Get("Origin") != "http://localhost") {
		reply(w, 403, map[string]string{"error": "origin rejected"})
		return
	}
	if r.Method == "GET" && r.URL.Path == "/api/desktops" {
		environment := "Existing Wayland console"
		if os.Getenv("XDG_SESSION_TYPE") == "x11" {
			environment = "Existing X11 console"
		}
		reply(w, 200, map[string]any{"desktops": []any{map[string]any{"id": b.id, "kind": "console", "environment": environment, "user": b.user, "state": "ready", "createdAt": b.started}}})
		return
	}
	if r.Method == "GET" && r.URL.Path == "/api/environments" {
		reply(w, 200, map[string]any{"environments": []any{}, "installedSessions": []any{}})
		return
	}
	prefix := "/api/desktops/" + b.id + "/"
	if !strings.HasPrefix(r.URL.Path, prefix) {
		reply(w, 404, map[string]string{"error": "console not found"})
		return
	}
	action := strings.TrimPrefix(r.URL.Path, prefix)
	if action == "status" && r.Method == "GET" {
		reply(w, 200, map[string]any{"state": "streaming", "kind": "console", "canResize": false, "source": map[string]uint32{"width": b.width, "height": b.height}})
		return
	}
	if action == "reconnect" && r.Method == "POST" {
		reply(w, 200, map[string]string{"id": b.id})
		return
	}
	if action == "offer" && r.Method == "POST" {
		b.offer(w, r)
		return
	}
	reply(w, 409, map[string]string{"error": "existing console cannot be resized or ended through this adapter"})
}
func (b *portalWeb) offer(w http.ResponseWriter, r *http.Request) {
	var offer webrtc.SessionDescription
	if json.NewDecoder(http.MaxBytesReader(w, r.Body, 262144)).Decode(&offer) != nil || offer.Type != webrtc.SDPTypeOffer {
		reply(w, 400, map[string]string{"error": "invalid offer"})
		return
	}
	b.mu.Lock()
	if b.viewer != nil {
		select {
		case <-b.viewer.done:
			b.viewer = nil
		default:
		}
	}
	if b.viewer != nil {
		b.mu.Unlock()
		reply(w, 409, map[string]string{"error": "console already has a viewer"})
		return
	}
	pc, err := webrtc.NewPeerConnection(webrtc.Configuration{})
	if err != nil {
		b.mu.Unlock()
		reply(w, 503, map[string]string{"error": "media unavailable"})
		return
	}
	ctx, cancel := context.WithCancel(context.Background())
	v := &portalViewer{pc: pc, cancel: cancel, done: make(chan struct{}), lease: inputLease{session: b.session, width: b.width, height: b.height, keys: map[int32]bool{}, buttons: map[int32]bool{}}}
	b.viewer = v
	b.mu.Unlock()
	success := false
	defer func() {
		if !success {
			v.close()
		}
	}()
	fail := func() { reply(w, 503, map[string]string{"error": "portal media negotiation failed"}) }
	track, err := webrtc.NewTrackLocalStaticRTP(webrtc.RTPCodecCapability{MimeType: webrtc.MimeTypeH264, ClockRate: 90000, SDPFmtpLine: "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f"}, "console", "dwdesktop")
	if err != nil {
		fail()
		return
	}
	sender, err := pc.AddTrack(track)
	if err != nil {
		fail()
		return
	}
	go func() {
		buffer := make([]byte, 1500)
		for {
			if _, _, err := sender.Read(buffer); err != nil {
				return
			}
		}
	}()
	pc.OnConnectionStateChange(func(state webrtc.PeerConnectionState) {
		if state == webrtc.PeerConnectionStateFailed || state == webrtc.PeerConnectionStateDisconnected || state == webrtc.PeerConnectionStateClosed {
			v.close()
		}
	})
	var channelOnce sync.Once
	pc.OnDataChannel(func(dc *webrtc.DataChannel) {
		if dc.Label() != "dwconsole.input" {
			dc.Close()
			return
		}
		accepted := false
		channelOnce.Do(func() { accepted = true })
		if !accepted {
			dc.Close()
			return
		}
		v.input(dc)
	})
	if pc.SetRemoteDescription(offer) != nil {
		fail()
		return
	}
	answer, err := pc.CreateAnswer(nil)
	if err != nil {
		fail()
		return
	}
	gather := webrtc.GatheringCompletePromise(pc)
	if pc.SetLocalDescription(answer) != nil {
		fail()
		return
	}
	select {
	case <-gather:
	case <-r.Context().Done():
		return
	case <-time.After(5 * time.Second):
		fail()
		return
	}
	// Start the encoder before advertising a usable answer; fixed logical output
	// dimensions keep high-DPI portal coordinates and browser input in agreement.
	if err = b.stream(ctx, v, track); err != nil {
		fail()
		return
	}
	go func() {
		select {
		case <-v.done:
		case <-time.After(15 * time.Second):
			if pc.ConnectionState() != webrtc.PeerConnectionStateConnected {
				v.close()
			}
		}
	}()
	success = true
	reply(w, 200, pc.LocalDescription())
}
func (b *portalWeb) stream(ctx context.Context, v *portalViewer, track *webrtc.TrackLocalStaticRTP) error {
	udp, err := net.ListenUDP("udp4", &net.UDPAddr{IP: net.IPv4(127, 0, 0, 1)})
	if err != nil {
		return err
	}
	var fd dbus.UnixFD
	callCtx, cancel := context.WithTimeout(ctx, 3*time.Second)
	defer cancel()
	if b.session.x11 == nil {
		if err = b.session.client.conn.Object(destination, path).CallWithContext(callCtx, cast+".OpenPipeWireRemote", 0, b.session.Handle, Options{}).Store(&fd); err != nil {
			udp.Close()
			return err
		}
	}
	var pipe *os.File
	if b.session.x11 == nil {
		pipe = os.NewFile(uintptr(fd), "portal-viewer")
		defer pipe.Close()
	}
	cmd := exec.CommandContext(ctx, "gst-launch-1.0", "-q", "pipewiresrc", "fd=3", fmt.Sprintf("path=%d", b.session.Stream.Node), "do-timestamp=true", "!", "videoconvert", "!", "videoscale", "!", "videorate", "!", fmt.Sprintf("video/x-raw,format=I420,width=%d,height=%d,framerate=30/1", b.width, b.height), "!", "x264enc", "tune=zerolatency", "speed-preset=ultrafast", "bitrate=6000", "key-int-max=30", "byte-stream=true", "!", "video/x-h264,profile=constrained-baseline", "!", "rtph264pay", "config-interval=-1", "pt=96", "mtu=1200", "!", "udpsink", "host=127.0.0.1", fmt.Sprintf("port=%d", udp.LocalAddr().(*net.UDPAddr).Port), "sync=false", "async=false")
	// Mutter emits damage-driven frames; feed videorate a bounded keepalive
	// so an idle desktop can negotiate and deliver its initial keyframe.
	cmd.Args = append(cmd.Args[:6], append([]string{"keepalive-time=100", "always-copy=true"}, cmd.Args[6:]...)...)
	if os.Getenv("XDG_SESSION_TYPE") == "x11" {
		// GNOME X11 exposes the portal but may not deliver PipeWire frames.
		// Capture only this logged-in user's X server, never GDM/root displays.
		if os.Getenv("DISPLAY") == "" {
			udp.Close()
			return errors.New("session X display unavailable")
		}
		separator := 0
		for i, arg := range cmd.Args {
			if arg == "!" {
				separator = i
				break
			}
		}
		cmd.Args = append([]string{cmd.Args[0], "-q", "ximagesrc", "use-damage=false", "show-pointer=true"}, cmd.Args[separator:]...)
	}
	if pipe != nil {
		cmd.ExtraFiles = []*os.File{pipe}
	}
	cmd.Stderr = os.Stderr
	cmd.Env = append(os.Environ(), "GST_DEBUG=2")
	directRTP := b.session.x11 != nil || exec.Command("gst-inspect-1.0", "x264enc").Run() != nil
	encoder, err := startEncoder(ctx, cmd, b.width, b.height, udp, track.WriteRTP)
	if err != nil {
		udp.Close()
		return err
	}
	go func() { _ = cmd.Wait(); udp.Close(); v.close() }()
	if encoder != nil {
		go func() { _ = encoder.Wait(); v.close() }()
	}
	if directRTP {
		// FFmpeg access units go directly to WebRTC, avoiding lossy localhost
		// UDP bursts when a large desktop keyframe exceeds the socket buffer.
		return nil
	}
	go func() {
		buffer := make([]byte, 1500)
		for {
			_ = udp.SetReadDeadline(time.Now().Add(5 * time.Second))
			n, _, err := udp.ReadFromUDP(buffer)
			if err != nil {
				v.close()
				return
			}
			var packet rtp.Packet
			if packet.Unmarshal(buffer[:n]) != nil {
				v.close()
				return
			}
			if track.WriteRTP(&packet) != nil {
				v.close()
				return
			}
		}
	}()
	return nil
}
func (v *portalViewer) input(dc *webrtc.DataChannel) {
	send := func(value any) {
		data, _ := json.Marshal(value)
		if dc.BufferedAmount() > 16384 || dc.SendText(string(data)) != nil {
			go v.close()
		}
	}
	unavailable := func(reason string) {
		if err := v.lease.release(); err != nil {
			v.lease.session.Close()
			go v.close()
		}
		send(map[string]string{"type": "unavailable", "reason": reason})
	}
	dc.OnClose(v.close)
	dc.OnMessage(func(message webrtc.DataChannelMessage) {
		if !message.IsString || len(message.Data) > 4096 {
			go v.close()
			return
		}
		var envelope struct {
			Type       string          `json:"type"`
			Generation string          `json:"generation"`
			Sequence   uint64          `json:"sequence"`
			Event      json.RawMessage `json:"event"`
		}
		v.mu.Lock()
		defer v.mu.Unlock()
		select {
		case <-v.done:
			return
		default:
		}
		if strictJSON(message.Data, &envelope) != nil {
			unavailable("Invalid input")
			return
		}
		l := &v.lease
		switch envelope.Type {
		case "acquire":
			if err := l.release(); err != nil {
				go v.close()
				return
			}
			gen, err := token()
			if err != nil {
				go v.close()
				return
			}
			l.generation = gen
			l.sequence = 0
			l.until = time.Now().Add(time.Second)
			send(map[string]any{"type": "ready", "protocol": "dwconsole.input", "version": "0.2.0", "generation": gen, "width": l.width, "height": l.height, "leaseMs": 1000})
		case "release":
			_ = l.release()
			send(map[string]string{"type": "released"})
		case "event":
			if l.generation == "" || envelope.Generation != l.generation || envelope.Sequence != l.sequence+1 || !time.Now().Before(l.until) {
				unavailable("Control expired or stale input")
				return
			}
			var event inputEvent
			if strictJSON(envelope.Event, &event) != nil || l.event(event) != nil {
				unavailable("Portal input rejected")
				return
			}
			l.sequence = envelope.Sequence
			l.until = time.Now().Add(time.Second)
			send(map[string]any{"type": "ack", "sequence": l.sequence, "accepted": true})
		default:
			unavailable("Invalid input command")
		}
	})
	go func() {
		ticker := time.NewTicker(100 * time.Millisecond)
		defer ticker.Stop()
		for {
			select {
			case <-v.done:
				return
			case <-ticker.C:
				v.mu.Lock()
				if v.lease.generation != "" && time.Now().After(v.lease.until) {
					unavailable("Control expired")
				}
				v.mu.Unlock()
			}
		}
	}()
}
