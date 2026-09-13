package manager

import (
	"bytes"
	"context"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/tls"
	"crypto/x509"
	"donkeywork-desktop/manager/bridgewire"
	"encoding/json"
	"encoding/pem"
	"fmt"
	"github.com/gorilla/websocket"
	"github.com/pion/webrtc/v4"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"
)

func testDeviceConnection(t *testing.T, store *Store) {
	t.Helper()
	ctx := context.Background()
	registered, err := store.Create(ctx, "mTLS test", "")
	if err != nil {
		t.Fatal(err)
	}
	key, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	csr, err := x509.CreateCertificateRequest(rand.Reader, &x509.CertificateRequest{}, key)
	if err != nil {
		t.Fatal(err)
	}
	id, cert, err := store.Claim(ctx, registered.Code, string(pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE REQUEST", Bytes: csr})))
	if err != nil {
		t.Fatal(err)
	}
	keyDER, _ := x509.MarshalPKCS8PrivateKey(key)
	pair, err := tls.X509KeyPair([]byte(cert), pem.EncodeToMemory(&pem.Block{Type: "PRIVATE KEY", Bytes: keyDER}))
	if err != nil {
		t.Fatal(err)
	}
	handler, _ := NewAPI(store, "https://desktops.donkeywork.dev")
	api := handler.(*API)
	server := httptest.NewUnstartedServer(http.HandlerFunc(api.DeviceHandler))
	ca := x509.NewCertPool()
	ca.AddCert(store.ca)
	server.TLS = &tls.Config{MinVersion: tls.VersionTLS13, ClientAuth: tls.RequireAndVerifyClientCert, ClientCAs: ca}
	server.StartTLS()
	defer server.Close()
	trust := x509.NewCertPool()
	trust.AddCert(server.Certificate())
	endpoint := "wss" + strings.TrimPrefix(server.URL, "https") + "/connect"
	noCert := websocket.Dialer{TLSClientConfig: &tls.Config{RootCAs: trust}, HandshakeTimeout: time.Second}
	if c, _, err := noCert.Dial(endpoint, nil); err == nil {
		c.Close()
		t.Fatal("missing certificate accepted")
	}
	dial := websocket.Dialer{TLSClientConfig: &tls.Config{RootCAs: trust, Certificates: []tls.Certificate{pair}}, HandshakeTimeout: time.Second}
	conn, _, err := dial.Dial(endpoint, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer conn.Close()
	if api.hub.Snapshot(id).Online {
		t.Fatal("online before heartbeat")
	}
	err = conn.WriteJSON(map[string]any{"version": 1, "type": "heartbeat", "hostname": "test-device", "platform": "linux/amd64", "broker": true})
	if err != nil {
		t.Fatal(err)
	}
	conn.SetReadDeadline(time.Now().Add(time.Second))
	var ack map[string]any
	if conn.ReadJSON(&ack) != nil || ack["deviceId"] != id {
		t.Fatal("heartbeat failed")
	}
	if !api.hub.Snapshot(id).Online {
		t.Fatal("not online after heartbeat")
	}
	result := make(chan error, 1)
	go func() {
		ctx, cancel := context.WithTimeout(ctx, 2*time.Second)
		defer cancel()
		reply, err := api.hub.request(ctx, id, bridgewire.Message{Method: "GET", Path: "/api/desktops"})
		if err == nil && (reply.Status != 200 || string(reply.Body) != `{"desktops":[]}`) {
			err = fmt.Errorf("invalid RPC response")
		}
		result <- err
	}()
	var request bridgewire.Message
	if conn.ReadJSON(&request) != nil || request.Type != "request" || request.Path != "/api/desktops" {
		t.Fatal("missing RPC request")
	}
	if err := conn.WriteJSON(bridgewire.Message{Version: 1, Type: "response", ID: request.ID, Status: 200, Body: json.RawMessage(`{"desktops":[]}`)}); err != nil {
		t.Fatal(err)
	}
	if err := <-result; err != nil {
		t.Fatal(err)
	}
	if err := api.ConfigureMedia("127.0.0.1:0", "127.0.0.1"); err != nil {
		t.Fatal(err)
	}
	pc, err := webrtc.NewPeerConnection(webrtc.Configuration{})
	if err != nil {
		t.Fatal(err)
	}
	defer pc.Close()
	pc.CreateDataChannel("dwconsole.input", nil)
	pc.AddTransceiverFromKind(webrtc.RTPCodecTypeVideo, webrtc.RTPTransceiverInit{Direction: webrtc.RTPTransceiverDirectionRecvonly})
	offer, err := pc.CreateOffer(nil)
	if err != nil {
		t.Fatal(err)
	}
	body, _ := json.Marshal(offer)
	mediaResult := make(chan int, 1)
	go func() {
		rec := httptest.NewRecorder()
		req := httptest.NewRequest("POST", "https://desktops.donkeywork.dev/api/v1/devices/"+id+"/broker/desktops/00112233445566778899aabbccddeeff/offer", bytes.NewReader(body))
		req.Header.Set("Origin", "https://desktops.donkeywork.dev")
		req.Header.Set("Content-Type", "application/json")
		api.ServeHTTP(rec, req)
		mediaResult <- rec.Code
	}()
	conn.SetReadDeadline(time.Now().Add(3 * time.Second))
	if conn.ReadJSON(&request) != nil || request.Type != "media.open" {
		t.Fatal("missing media open request")
	}
	if err := conn.WriteJSON(bridgewire.Message{Version: 1, Type: "response", ID: request.ID, Status: 200, Body: json.RawMessage(`{}`)}); err != nil {
		t.Fatal(err)
	}
	if code := <-mediaResult; code != 200 {
		t.Fatalf("media offer HTTP %d", code)
	}
	api.hub.mu.Lock()
	viewer := api.hub.live[id].streams[request.ID]
	api.hub.mu.Unlock()
	if viewer == nil {
		t.Fatal("media stream not bound to device")
	}
	if err = store.Revoke(ctx, id); err != nil {
		t.Fatal(err)
	}
	api.hub.Disconnect(id)
	select {
	case <-viewer.done:
	case <-time.After(time.Second):
		t.Fatal("revocation did not close media")
	}
	if api.hub.Snapshot(id).Online {
		t.Fatal("revoked still online")
	}
	if _, err := api.hub.request(ctx, id, bridgewire.Message{Method: "GET", Path: "/api/desktops"}); err == nil {
		t.Fatal("revoked RPC accepted")
	}
	if c, res, err := dial.Dial(endpoint, nil); err == nil {
		c.Close()
		t.Fatal("revoked certificate accepted")
	} else if res == nil || res.StatusCode != 403 {
		t.Fatal("expected explicit revocation rejection")
	}
}
