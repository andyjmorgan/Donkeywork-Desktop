package manager

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"log"
	"net"
	"net/http"
	"sync"
	"time"

	"donkeywork-desktop/manager/bridgewire"
	"github.com/gorilla/websocket"
	"github.com/pion/ice/v4"
	"github.com/pion/rtcp"
	"github.com/pion/rtp"
	"github.com/pion/webrtc/v4"
)

type mediaGateway struct{ api *webrtc.API }

// The published UDP port must equal the bound port (including at a NodePort).
func (a *API) ConfigureMedia(bind, advertisedIP string) error {
	addr, err := net.ResolveUDPAddr("udp4", bind)
	if err != nil {
		return err
	}
	conn, err := net.ListenUDP("udp4", addr)
	if err != nil {
		return err
	}
	se := webrtc.SettingEngine{}
	se.SetICEUDPMux(ice.NewUDPMuxDefault(ice.UDPMuxParams{UDPConn: conn}))
	se.SetNetworkTypes([]webrtc.NetworkType{webrtc.NetworkTypeUDP4})
	se.SetNAT1To1IPs([]string{advertisedIP}, webrtc.ICECandidateTypeHost)
	a.Media = &mediaGateway{api: webrtc.NewAPI(webrtc.WithSettingEngine(se))}
	return nil
}

type relayViewer struct {
	pc     *webrtc.PeerConnection
	track  *webrtc.TrackLocalStaticRTP
	mu     sync.Mutex
	input  *webrtc.DataChannel
	queue  chan []byte
	done   chan struct{}
	once   sync.Once
	device *liveDevice
	id     string
}

func (v *relayViewer) send(kind byte, b []byte) error {
	v.device.writeMu.Lock()
	defer v.device.writeMu.Unlock()
	v.device.conn.SetWriteDeadline(time.Now().Add(2 * time.Second))
	return v.device.conn.WriteMessage(websocket.BinaryMessage, bridgewire.Packet(v.id, kind, b))
}
func (v *relayViewer) close() { v.once.Do(func() { close(v.done); go v.pc.Close() }) }
func (v *relayViewer) receive(kind byte, b []byte) {
	switch kind {
	case bridgewire.Video:
		select {
		case v.queue <- b:
		case <-v.done:
		default:
			v.close()
		}
	case bridgewire.Input:
		v.mu.Lock()
		dc := v.input
		v.mu.Unlock()
		if len(b) > 4096 {
			v.close()
			return
		}
		if dc != nil && dc.ReadyState() == webrtc.DataChannelStateOpen {
			if dc.BufferedAmount() > 16384 || dc.SendText(string(b)) != nil {
				v.close()
			}
		}
	case bridgewire.Closed:
		v.close()
	default:
		v.close()
	}
}

func (a *API) mediaOffer(w http.ResponseWriter, r *http.Request, device, path string, body json.RawMessage) {
	stage := "browser peer setup"
	fail := func() {
		log.Printf("media relay negotiation failed at %s", stage)
		respond(w, 503, map[string]string{"error": "media relay unavailable"})
	}
	var offer webrtc.SessionDescription
	if json.Unmarshal(body, &offer) != nil || offer.Type != webrtc.SDPTypeOffer {
		respond(w, 400, map[string]string{"error": "invalid offer"})
		return
	}
	pc, err := a.Media.api.NewPeerConnection(webrtc.Configuration{})
	if err != nil {
		fail()
		return
	}
	track, err := webrtc.NewTrackLocalStaticRTP(webrtc.RTPCodecCapability{MimeType: webrtc.MimeTypeH264, ClockRate: 90000, SDPFmtpLine: "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f"}, "desktop", "managed-desktop")
	if err != nil {
		pc.Close()
		fail()
		return
	}
	var nonce [16]byte
	if _, err = rand.Read(nonce[:]); err != nil {
		pc.Close()
		fail()
		return
	}
	v := &relayViewer{pc: pc, track: track, id: hex.EncodeToString(nonce[:]), queue: make(chan []byte, 256), done: make(chan struct{})}
	a.hub.mu.Lock()
	d := a.hub.live[device]
	if d == nil || !d.broker || len(d.streams) >= 4 || time.Since(d.last) > 45*time.Second {
		a.hub.mu.Unlock()
		pc.Close()
		fail()
		return
	}
	v.device = d
	d.streams[v.id] = v
	a.hub.mu.Unlock()
	success := false
	defer func() {
		if !success {
			v.close()
		}
	}()
	go func() {
		select {
		case <-d.done:
			v.close()
		case <-v.done:
		}
		a.hub.mu.Lock()
		delete(d.streams, v.id)
		a.hub.mu.Unlock()
		_ = v.send(bridgewire.Closed, nil)
	}()
	pc.OnConnectionStateChange(func(state webrtc.PeerConnectionState) {
		if state == webrtc.PeerConnectionStateFailed || state == webrtc.PeerConnectionStateDisconnected || state == webrtc.PeerConnectionStateClosed {
			v.close()
		}
	})
	pc.OnDataChannel(func(dc *webrtc.DataChannel) {
		v.mu.Lock()
		if v.input != nil || dc.Label() != "dwconsole.input" {
			v.mu.Unlock()
			dc.Close()
			return
		}
		v.input = dc
		v.mu.Unlock()
		dc.OnMessage(func(msg webrtc.DataChannelMessage) {
			if !msg.IsString || len(msg.Data) > 4096 {
				v.close()
				return
			}
			if v.send(bridgewire.Input, msg.Data) != nil {
				v.close()
			}
		})
		dc.OnClose(v.close)
	})
	sender, err := pc.AddTrack(track)
	if err != nil {
		fail()
		return
	}
	go func() {
		for {
			packets, _, err := sender.ReadRTCP()
			if err != nil {
				return
			}
			for _, packet := range packets {
				switch packet.(type) {
				case *rtcp.PictureLossIndication, *rtcp.FullIntraRequest:
					if v.send(bridgewire.Feedback, nil) != nil {
						v.close()
						return
					}
				}
			}
		}
	}()
	go func() {
		for {
			select {
			case <-v.done:
				return
			case b := <-v.queue:
				var packet rtp.Packet
				if packet.Unmarshal(b) != nil || track.WriteRTP(&packet) != nil {
					v.close()
					return
				}
			}
		}
	}()
	go func() {
		timer := time.NewTimer(20 * time.Second)
		defer timer.Stop()
		select {
		case <-v.done:
			return
		case <-timer.C:
			if pc.ConnectionState() != webrtc.PeerConnectionStateConnected {
				v.close()
			}
		}
	}()
	if pc.SetRemoteDescription(offer) != nil {
		fail()
		return
	}
	answer, err := pc.CreateAnswer(nil)
	if err != nil {
		fail()
		return
	}
	gathered := webrtc.GatheringCompletePromise(pc)
	if pc.SetLocalDescription(answer) != nil {
		fail()
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 12*time.Second)
	defer cancel()
	stage = "device media open"
	reply, err := a.hub.request(ctx, device, bridgewire.Message{ID: v.id, Type: "media.open", Method: "POST", Path: path})
	if err != nil || reply.Status != 200 {
		fail()
		return
	}
	stage = "browser ICE gathering"
	select {
	case <-gathered:
	case <-ctx.Done():
		fail()
		return
	case <-v.done:
		fail()
		return
	}
	success = true
	respond(w, 200, pc.LocalDescription())
}
