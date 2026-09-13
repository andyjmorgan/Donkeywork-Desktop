package main

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"net/url"
	"os"
	"os/signal"
	"path/filepath"
	"sync"
	"syscall"
	"time"

	"github.com/pion/ice/v4"
	"github.com/pion/webrtc/v4"
	"github.com/pion/webrtc/v4/pkg/media"
)

type viewer struct {
	pc     *webrtc.PeerConnection
	track  *webrtc.TrackLocalStaticSample
	frames chan accessUnit
	done   chan struct{}
	once   sync.Once
}

func (v *viewer) close() { v.once.Do(func() { close(v.done); _ = v.pc.Close() }) }

type bridge struct {
	recovering    bool
	generation    uint64
	captureBroker *net.UnixConn
	captureConn   net.Conn
	recoveryDone  chan struct{}
	input         *inputBridge
	mu            sync.Mutex
	header        streamHeader
	failure       string
	viewers       map[*viewer]bool
	api           *webrtc.API
}

// pruneViewersLocked removes peers that have already closed (or whose
// lifecycle goroutine has signalled done) but have not yet been collected.
// Negotiation failures can otherwise leave a short-lived stale entry behind,
// causing the next browser retry to receive a misleading HTTP 409.
func (b *bridge) pruneViewersLocked() []*viewer {
	var stale []*viewer
	for v := range b.viewers {
		state := v.pc.ConnectionState()
		select {
		case <-v.done:
			delete(b.viewers, v)
			stale = append(stale, v)
		default:
			if state == webrtc.PeerConnectionStateClosed || state == webrtc.PeerConnectionStateFailed {
				delete(b.viewers, v)
				stale = append(stale, v)
			}
		}
	}
	return stale
}

type deadlineReader struct{ net.Conn }

func (r deadlineReader) Read(p []byte) (int, error) {
	if err := r.SetReadDeadline(time.Now().Add(10 * time.Second)); err != nil {
		return 0, err
	}
	return r.Conn.Read(p)
}

func (b *bridge) fail(reason string) {
	b.stopRecovery()
	if b.input != nil {
		b.input.stop()
	}
	b.mu.Lock()
	b.failure = reason
	vs := make([]*viewer, 0, len(b.viewers))
	for v := range b.viewers {
		vs = append(vs, v)
	}
	b.mu.Unlock()
	for _, v := range vs {
		v.close()
	}
}
func (b *bridge) broadcast(au accessUnit) error {
	b.mu.Lock()
	defer b.mu.Unlock()
	if b.recovering {
		if !au.key {
			return nil
		}
		b.recovering = false
		if b.input != nil {
			b.input.resume()
		}
	}
	au.generation = b.generation
	for v, ready := range b.viewers {
		select {
		case <-v.done:
			delete(b.viewers, v)
			continue
		default:
		}
		if v.pc.ConnectionState() != webrtc.PeerConnectionStateConnected {
			continue
		}
		if !ready && !au.key {
			continue
		}
		b.viewers[v] = true
		select {
		case v.frames <- au:
		default:
			go v.close()
			delete(b.viewers, v)
		}
	}
	return nil
}
func (b *bridge) read(r io.Reader) {
	if err := b.readFeed(r); err != nil {
		log.Printf("capture feed stopped: %v", err)
		b.fail("Capture feed stopped; restart the bridge after restoring the daemon.")
	}
}
func (b *bridge) readFeed(r io.Reader) error {
	p := annexParser{}
	au := auParser{}
	for {
		chunk, err := readRecord(r, maxChunk)
		if err == nil {
			err = p.push(chunk, func(n []byte) error { return au.nal(n, b.broadcast) })
		}
		if err != nil {
			return err
		}
	}
}
func (b *bridge) status(w http.ResponseWriter, r *http.Request) {
	if r.Method != "GET" {
		w.WriteHeader(405)
		return
	}
	b.mu.Lock()
	defer b.mu.Unlock()
	w.Header().Set("Content-Type", "application/json")
	state := "streaming"
	if b.recovering {
		state = "recovering"
	}
	if b.failure != "" {
		state = "failed"
	}
	_ = json.NewEncoder(w).Encode(map[string]any{"state": state, "error": b.failure, "source": b.header})
}
func (b *bridge) offer(w http.ResponseWriter, r *http.Request) {
	if r.Method != "POST" {
		w.WriteHeader(405)
		return
	}
	var offer webrtc.SessionDescription
	d := json.NewDecoder(http.MaxBytesReader(w, r.Body, 256*1024))
	d.DisallowUnknownFields()
	if d.Decode(&offer) != nil || d.Decode(new(any)) != io.EOF || offer.Type != webrtc.SDPTypeOffer {
		http.Error(w, "Invalid offer", 400)
		return
	}
	b.mu.Lock()
	stale := b.pruneViewersLocked()
	if b.failure != "" {
		b.mu.Unlock()
		for _, v := range stale {
			v.close()
		}
		http.Error(w, "Capture feed unavailable", 503)
		return
	}
	if len(b.viewers) >= 2 {
		b.mu.Unlock()
		for _, v := range stale {
			v.close()
		}
		w.Header().Set("Retry-After", "2")
		http.Error(w, "Two viewer limit reached; close other console tabs", 409)
		return
	}
	pc, err := b.api.NewPeerConnection(webrtc.Configuration{})
	if err != nil {
		b.mu.Unlock()
		http.Error(w, "Peer creation failed", 500)
		return
	}
	track, err := webrtc.NewTrackLocalStaticSample(webrtc.RTPCodecCapability{MimeType: webrtc.MimeTypeH264, ClockRate: 90000, SDPFmtpLine: "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f"}, "console", "physical-console")
	if err != nil {
		b.mu.Unlock()
		_ = pc.Close()
		http.Error(w, "Track creation failed", 500)
		return
	}
	v := &viewer{pc: pc, track: track, frames: make(chan accessUnit, 3), done: make(chan struct{})}
	b.viewers[v] = false
	b.mu.Unlock()
	for _, staleViewer := range stale {
		staleViewer.close()
	}
	success := false
	b.attachInput(v)
	defer func() {
		if !success {
			v.close()
		}
	}()
	go func() { <-v.done; b.mu.Lock(); delete(b.viewers, v); b.mu.Unlock() }()
	sender, err := pc.AddTrack(track)
	if err != nil {
		http.Error(w, "Video track unavailable", 500)
		return
	}
	go func() {
		buf := make([]byte, 1500)
		for {
			if _, _, err := sender.Read(buf); err != nil {
				return
			}
		}
	}()
	pc.OnConnectionStateChange(func(state webrtc.PeerConnectionState) {
		if state == webrtc.PeerConnectionStateConnected {
			b.mu.Lock()
			if _, exists := b.viewers[v]; exists {
				b.viewers[v] = false
			}
			b.mu.Unlock()
		} else if state == webrtc.PeerConnectionStateFailed || state == webrtc.PeerConnectionStateClosed || state == webrtc.PeerConnectionStateDisconnected {
			go v.close()
		}
	})
	if pc.SetRemoteDescription(offer) != nil {
		http.Error(w, "Unsupported offer", 400)
		return
	}
	answer, err := pc.CreateAnswer(nil)
	if err != nil {
		http.Error(w, "Answer failed", 500)
		return
	}
	gathered := webrtc.GatheringCompletePromise(pc)
	if pc.SetLocalDescription(answer) != nil {
		http.Error(w, "Answer failed", 500)
		return
	}
	select {
	case <-gathered:
	case <-r.Context().Done():
		return
	case <-time.After(8 * time.Second):
		http.Error(w, "ICE gathering timeout", 504)
		return
	}
	go func() {
		defer v.close()
		timer := time.NewTimer(15 * time.Second)
		defer timer.Stop()
		for {
			select {
			case <-v.done:
				return
			case <-timer.C:
				if pc.ConnectionState() != webrtc.PeerConnectionStateConnected {
					return
				}
			case sample := <-v.frames:
				if pc.ConnectionState() != webrtc.PeerConnectionStateConnected {
					continue
				}
				b.mu.Lock()
				if b.recovering || sample.generation != b.generation {
					b.mu.Unlock()
					continue
				}
				writeErr := track.WriteSample(media.Sample{Data: sample.data, Duration: time.Second / time.Duration(b.header.FPS)})
				b.mu.Unlock()
				if writeErr != nil {
					return
				}
			}
		}
	}()
	w.Header().Set("Content-Type", "application/json")
	if json.NewEncoder(w).Encode(pc.LocalDescription()) != nil {
		return
	}
	success = true
}

func run() error {
	listen := flag.String("listen", "127.0.0.1:8090", "HTTP bind address (explicit private IP for lab access)")
	iceListen := flag.String("ice-listen", "127.0.0.1:8091", "WebRTC UDP host candidate address")
	origin := flag.String("origin", "http://127.0.0.1:8090", "Exact browser origin")
	fd := flag.Int("stream-fd", 3, "Inherited connected dwconsole.stream file descriptor")
	inputFD := flag.Int("input-fd", -1, "Optional inherited connected dwconsole.input descriptor")
	brokerFD := flag.Int("capture-broker-fd", -1, "Optional inherited capture descriptor broker")
	assets := flag.String("assets", "../web/dist", "Built web assets directory")
	managedRoot := flag.String("managed-root", "", "Optional trusted private managed-session directory; enables resize")
	flag.Parse()
	if os.Geteuid() == 0 {
		return errors.New("refusing to run HTTP/codec transport as root; pass connected FD then drop privileges")
	}
	u, err := url.Parse(*origin)
	if err != nil || u.Scheme != "http" || u.Host == "" || u.Path != "" {
		return errors.New("origin must be an exact http://host:port origin")
	}
	source := os.NewFile(uintptr(*fd), "console-stream")
	if source == nil {
		return errors.New("invalid stream FD")
	}
	defer source.Close()
	conn, err := net.FileConn(source)
	if err != nil {
		return fmt.Errorf("stream FD must be a connected socket: %w", err)
	}
	defer conn.Close()
	reader := deadlineReader{conn}
	header, err := readHeader(reader)
	if err != nil {
		return fmt.Errorf("capture header: %w", err)
	}
	addr, err := net.ResolveUDPAddr("udp4", *iceListen)
	if err != nil {
		return err
	}
	if addr.IP == nil || addr.IP.IsUnspecified() {
		return errors.New("ice-listen needs an explicit IPv4 host address")
	}
	udp, err := net.ListenUDP("udp4", addr)
	if err != nil {
		return err
	}
	defer udp.Close()
	mux := ice.NewUDPMuxDefault(ice.UDPMuxParams{UDPConn: udp})
	defer mux.Close()
	se := webrtc.SettingEngine{}
	se.SetICEUDPMux(mux)
	se.SetNetworkTypes([]webrtc.NetworkType{webrtc.NetworkTypeUDP4})
	se.SetIncludeLoopbackCandidate(true)
	se.SetNAT1To1IPs([]string{addr.IP.String()}, webrtc.ICECandidateTypeHost)
	engine := &webrtc.MediaEngine{}
	if err = engine.RegisterCodec(webrtc.RTPCodecParameters{RTPCodecCapability: webrtc.RTPCodecCapability{MimeType: webrtc.MimeTypeH264, ClockRate: 90000, SDPFmtpLine: "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f", RTCPFeedback: []webrtc.RTCPFeedback{{Type: "nack"}, {Type: "nack", Parameter: "pli"}}}, PayloadType: 102}, webrtc.RTPCodecTypeVideo); err != nil {
		return err
	}
	b := &bridge{header: header, viewers: map[*viewer]bool{}, api: webrtc.NewAPI(webrtc.WithMediaEngine(engine), webrtc.WithSettingEngine(se))}
	if *inputFD >= 0 {
		if *inputFD == *fd {
			return errors.New("input and stream descriptors must differ")
		}
		file := os.NewFile(uintptr(*inputFD), "console-input")
		if file == nil {
			return errors.New("invalid input descriptor")
		}
		inputConn, inputErr := net.FileConn(file)
		_ = file.Close()
		if inputErr != nil {
			return fmt.Errorf("input FD must be a connected socket: %w", inputErr)
		}
		b.input = newInputBridge(inputConn, header.Width, header.Height)
		defer b.input.stop()
	}
	sourceEnded := make(chan struct{})
	if *brokerFD >= 0 {
		if *brokerFD == *fd || *brokerFD == *inputFD {
			return errors.New("broker descriptor must be distinct")
		}
		file := os.NewFile(uintptr(*brokerFD), "capture-broker")
		if file == nil {
			return errors.New("invalid capture broker descriptor")
		}
		brokerConn, brokerErr := net.FileConn(file)
		_ = file.Close()
		if brokerErr != nil {
			return brokerErr
		}
		unixConn, ok := brokerConn.(*net.UnixConn)
		if !ok {
			brokerConn.Close()
			return errors.New("capture broker must be a Unix socket")
		}
		b.captureBroker = unixConn
		b.captureConn = conn
		b.recoveryDone = make(chan struct{})
		defer b.stopRecovery()
		go b.readRecoverable(conn)
	} else {
		go func() { b.read(reader); close(sourceEnded) }()
	}
	h := http.NewServeMux()
	h.HandleFunc("/api/status", b.status)
	h.HandleFunc("/api/offer", b.offer)
	if *managedRoot != "" {
		h.HandleFunc("/api/resize", managedResize(*managedRoot))
	}
	root, err := filepath.Abs(*assets)
	if err != nil {
		return err
	}
	if _, err = os.Stat(filepath.Join(root, "index.html")); err != nil {
		return err
	}
	h.Handle("/", http.FileServer(http.Dir(root)))
	srv := &http.Server{Addr: *listen, ReadHeaderTimeout: 5 * time.Second, ReadTimeout: 12 * time.Second, WriteTimeout: 12 * time.Second, IdleTimeout: 30 * time.Second, MaxHeaderBytes: 16 * 1024, Handler: http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Cache-Control", "no-store")
		w.Header().Set("X-Content-Type-Options", "nosniff")
		if r.Host != u.Host || (r.Header.Get("Origin") != "" && r.Header.Get("Origin") != *origin) {
			http.Error(w, "Origin rejected", 403)
			return
		}
		if r.Method == "POST" && r.Header.Get("Origin") != *origin {
			http.Error(w, "Origin required", 403)
			return
		}
		h.ServeHTTP(w, r)
	})}
	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()
	go func() {
		select {
		case <-ctx.Done():
		case <-sourceEnded:
			// The launcher must acquire the new seat's socket after a handoff.
		}
		b.fail("Bridge stopping")
		_ = source.Close()
		_ = conn.Close()
		shutdown, cancel := context.WithTimeout(context.Background(), 3*time.Second)
		defer cancel()
		_ = srv.Shutdown(shutdown)
	}()
	log.Printf("Console viewer %s; H264 %dx%d; UDP %s", *origin, header.Width, header.Height, *iceListen)
	err = srv.ListenAndServe()
	if errors.Is(err, http.ErrServerClosed) {
		return nil
	}
	return err
}
func main() {
	if err := run(); err != nil {
		log.Fatal(err)
	}
}
