package main

import (
	"context"
	"donkeywork-desktop/manager/bridgewire"
	"encoding/json"
	"github.com/pion/rtcp"
	"github.com/pion/webrtc/v4"
	"log"
	"sync"
	"time"
)

type localMedia struct {
	mu        sync.Mutex
	streams   map[string]*localViewer
	cancelled map[string]time.Time
	send      func([]byte) error
}
type localViewer struct {
	pc           *webrtc.PeerConnection
	input        *webrtc.DataChannel
	mu           sync.Mutex
	ssrc         uint32
	lastFeedback time.Time
}

func (m *localMedia) closeAll() {
	m.mu.Lock()
	defer m.mu.Unlock()
	for _, v := range m.streams {
		go v.pc.Close()
	}
	m.streams = make(map[string]*localViewer)
}
func (m *localMedia) receive(b []byte) {
	if len(b) < 33 || len(b) > 4129 {
		return
	}
	id := string(b[:32])
	m.mu.Lock()
	if b[32] == bridgewire.Closed {
		if m.cancelled == nil {
			m.cancelled = make(map[string]time.Time)
		}
		for key, at := range m.cancelled {
			if time.Since(at) > 30*time.Second {
				delete(m.cancelled, key)
			}
		}
		if len(m.cancelled) < 128 {
			m.cancelled[id] = time.Now()
		}
	}
	v := m.streams[id]
	m.mu.Unlock()
	if v == nil {
		return
	}
	switch b[32] {
	case bridgewire.Closed:
		go v.pc.Close()
	case bridgewire.Input:
		if v.input.ReadyState() != webrtc.DataChannelStateOpen || v.input.BufferedAmount() > 16384 || v.input.SendText(string(b[33:])) != nil {
			go v.pc.Close()
		}
	case bridgewire.Feedback:
		v.mu.Lock()
		ssrc := v.ssrc
		due := time.Since(v.lastFeedback) > 200*time.Millisecond
		if due {
			v.lastFeedback = time.Now()
		}
		v.mu.Unlock()
		if due && ssrc != 0 {
			_ = v.pc.WriteRTCP([]rtcp.Packet{&rtcp.PictureLossIndication{MediaSSRC: ssrc}})
		}
	}
}
func (m *localMedia) open(ctx context.Context, socket string, msg bridgewire.Message) bridgewire.Message {
	reply := bridgewire.Message{Version: 1, Type: "response", ID: msg.ID, Status: 503, Body: json.RawMessage(`{"error":"local media unavailable"}`)}
	m.mu.Lock()
	_, cancelled := m.cancelled[msg.ID]
	if ctx.Err() != nil || cancelled || len(m.streams) >= 4 || m.streams[msg.ID] != nil {
		m.mu.Unlock()
		return reply
	}
	pc, err := webrtc.NewPeerConnection(webrtc.Configuration{})
	if err != nil {
		m.mu.Unlock()
		return reply
	}
	dc, err := pc.CreateDataChannel("dwconsole.input", nil)
	if err != nil {
		m.mu.Unlock()
		pc.Close()
		return reply
	}
	v := &localViewer{pc: pc, input: dc}
	m.streams[msg.ID] = v
	m.mu.Unlock()
	done := make(chan struct{})
	var once sync.Once
	cleanup := func() {
		once.Do(func() {
			close(done)
			m.mu.Lock()
			delete(m.streams, msg.ID)
			m.mu.Unlock()
			go pc.Close()
			_ = m.send(bridgewire.Packet(msg.ID, bridgewire.Closed, nil))
		})
	}
	success := false
	stage := "create local offer"
	defer func() {
		if !success {
			log.Printf("local media negotiation failed at %s", stage)
			cleanup()
		}
	}()
	send := func(kind byte, b []byte) {
		if m.send(bridgewire.Packet(msg.ID, kind, b)) != nil {
			cleanup()
		}
	}
	pc.OnConnectionStateChange(func(state webrtc.PeerConnectionState) {
		if state == webrtc.PeerConnectionStateFailed || state == webrtc.PeerConnectionStateDisconnected || state == webrtc.PeerConnectionStateClosed {
			cleanup()
		}
	})
	dc.OnMessage(func(msg webrtc.DataChannelMessage) {
		if !msg.IsString || len(msg.Data) > 4096 {
			cleanup()
			return
		}
		send(bridgewire.Input, msg.Data)
	})
	dc.OnClose(cleanup)
	pc.OnTrack(func(track *webrtc.TrackRemote, _ *webrtc.RTPReceiver) {
		if track.Codec().MimeType != webrtc.MimeTypeH264 {
			cleanup()
			return
		}
		v.mu.Lock()
		v.ssrc = uint32(track.SSRC())
		v.mu.Unlock()
		for {
			packet, _, err := track.ReadRTP()
			if err != nil {
				return
			}
			b, err := packet.Marshal()
			if err != nil {
				cleanup()
				return
			}
			send(bridgewire.Video, b)
			select {
			case <-done:
				return
			default:
			}
		}
	})
	if _, err = pc.AddTransceiverFromKind(webrtc.RTPCodecTypeVideo, webrtc.RTPTransceiverInit{Direction: webrtc.RTPTransceiverDirectionRecvonly}); err != nil {
		return reply
	}
	offer, err := pc.CreateOffer(nil)
	if err != nil {
		log.Printf("local WebRTC offer creation: %v", err)
		return reply
	}
	gather := webrtc.GatheringCompletePromise(pc)
	stage = "gather local ICE"
	if pc.SetLocalDescription(offer) != nil {
		return reply
	}
	select {
	case <-gather:
	case <-ctx.Done():
		return reply
	case <-time.After(5 * time.Second):
		return reply
	}
	body, _ := json.Marshal(pc.LocalDescription())
	request := msg
	request.Type = "request"
	request.Body = body
	response := brokerRequest(ctx, socket, request)
	stage = "local broker offer"
	if response.Status != 200 {
		log.Printf("local broker offer HTTP %d", response.Status)
		return reply
	}
	var answer webrtc.SessionDescription
	stage = "apply local answer"
	if response.Status != 200 || json.Unmarshal(response.Body, &answer) != nil || pc.SetRemoteDescription(answer) != nil {
		return reply
	}
	go func() {
		select {
		case <-ctx.Done():
			cleanup()
		case <-done:
		}
	}()
	go func() {
		timer := time.NewTimer(20 * time.Second)
		defer timer.Stop()
		select {
		case <-done:
		case <-timer.C:
			if pc.ConnectionState() != webrtc.PeerConnectionStateConnected {
				cleanup()
			}
		}
	}()
	success = true
	reply.Status = 200
	reply.Body = json.RawMessage(`{}`)
	return reply
}
