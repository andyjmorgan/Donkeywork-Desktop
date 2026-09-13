package main

import (
	"bytes"
	"context"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"donkeywork-desktop/manager/bridgewire"
	"encoding/json"
	"encoding/pem"
	"errors"
	"flag"
	"fmt"
	"io"
	"log"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"time"

	"github.com/gorilla/websocket"
)

type config struct {
	DeviceID string `json:"deviceId"`
	Endpoint string `json:"deviceEndpoint"`
}

func writeNew(path string, data []byte) error {
	f, e := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0600)
	if e != nil {
		return e
	}
	defer f.Close()
	if _, e = f.Write(data); e != nil {
		return e
	}
	return f.Sync()
}
func roots(path string) (*x509.CertPool, error) {
	p, e := x509.SystemCertPool()
	if e != nil {
		p = x509.NewCertPool()
	}
	if path != "" {
		b, e := os.ReadFile(path)
		if e != nil {
			return nil, e
		}
		if !p.AppendCertsFromPEM(b) {
			return nil, errors.New("invalid manager trust file")
		}
	}
	return p, nil
}
func enroll(state, origin, code, codeFile, caFile string) error {
	u, e := url.Parse(origin)
	if e != nil || u.Scheme != "https" || u.Host == "" || u.User != nil || u.RawQuery != "" || u.Fragment != "" || (u.Path != "" && u.Path != "/") {
		return errors.New("manager must be an HTTPS origin")
	}
	if _, e = os.Stat(filepath.Join(state, "device.json")); e == nil {
		return errors.New("already enrolled; no code will be submitted")
	}
	if code != "" && codeFile != "" {
		return errors.New("choose code or code-file")
	}
	if codeFile != "" {
		f, e := os.Open(codeFile)
		if e != nil {
			return e
		}
		b, e := io.ReadAll(io.LimitReader(f, 64))
		f.Close()
		if e != nil {
			return e
		}
		code = string(b)
	}
	if len(code) == 0 || len(code) > 32 {
		return errors.New("enrollment code required")
	}
	trust, e := roots(caFile)
	if e != nil {
		return errors.New("cannot load HTTPS trust")
	}
	keyPath := filepath.Join(state, "device.key")
	var key *ecdsa.PrivateKey
	if b, e := os.ReadFile(keyPath); e == nil {
		block, _ := pem.Decode(b)
		if block == nil {
			return errors.New("invalid retained key")
		}
		k, e := x509.ParsePKCS8PrivateKey(block.Bytes)
		if e != nil {
			return e
		}
		var ok bool
		key, ok = k.(*ecdsa.PrivateKey)
		if !ok {
			return errors.New("invalid retained key type")
		}
	} else if os.IsNotExist(e) {
		key, e = ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
		if e != nil {
			return e
		}
		der, e := x509.MarshalPKCS8PrivateKey(key)
		if e != nil {
			return e
		}
		if e = writeNew(keyPath, pem.EncodeToMemory(&pem.Block{Type: "PRIVATE KEY", Bytes: der})); e != nil {
			return e
		}
	} else {
		return e
	}
	csr, e := x509.CreateCertificateRequest(rand.Reader, &x509.CertificateRequest{Subject: pkix.Name{CommonName: "pending-device"}}, key)
	if e != nil {
		return e
	}
	body, _ := json.Marshal(map[string]string{"code": code, "csrPEM": string(pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE REQUEST", Bytes: csr}))})
	client := &http.Client{Timeout: 20 * time.Second, CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }, Transport: &http.Transport{TLSClientConfig: &tls.Config{MinVersion: tls.VersionTLS13, RootCAs: trust}}}
	u.Path = "/api/v1/enroll"
	response, e := client.Post(u.String(), "application/json", bytes.NewReader(body))
	if e != nil {
		return errors.New("enrollment request failed; verify TLS trust/network; retained key was not replaced")
	}
	defer response.Body.Close()
	if response.StatusCode != 201 {
		return fmt.Errorf("enrollment rejected (HTTP %d); code not logged", response.StatusCode)
	}
	var reply struct {
		DeviceID    string `json:"deviceId"`
		Endpoint    string `json:"deviceEndpoint"`
		Certificate string `json:"certificatePEM"`
		CA          string `json:"deviceCAPEM"`
	}
	if json.NewDecoder(io.LimitReader(response.Body, 32768)).Decode(&reply) != nil {
		return errors.New("invalid enrollment response; code may already be consumed")
	}
	endpoint, e := url.Parse(reply.Endpoint)
	if e != nil || endpoint.Scheme != "wss" || endpoint.Hostname() != u.Hostname() || endpoint.Path != "/connect" || endpoint.User != nil {
		return errors.New("invalid device endpoint; code consumed")
	}
	block, _ := pem.Decode([]byte(reply.Certificate))
	if block == nil {
		return errors.New("invalid certificate response")
	}
	cert, e := x509.ParseCertificate(block.Bytes)
	if e != nil {
		return e
	}
	a, _ := x509.MarshalPKIXPublicKey(key.Public())
	b, _ := x509.MarshalPKIXPublicKey(cert.PublicKey)
	caPool := x509.NewCertPool()
	if !caPool.AppendCertsFromPEM([]byte(reply.CA)) {
		return errors.New("invalid device CA")
	}
	if _, e = cert.Verify(x509.VerifyOptions{Roots: caPool, KeyUsages: []x509.ExtKeyUsage{x509.ExtKeyUsageClientAuth}}); e != nil {
		return errors.New("invalid issued certificate")
	}
	if !bytes.Equal(a, b) || cert.Subject.CommonName != reply.DeviceID || len(cert.URIs) != 1 || cert.URIs[0].String() != "spiffe://desktops.donkeywork.dev/device/"+reply.DeviceID {
		return errors.New("certificate binding mismatch")
	}
	if e = writeNew(filepath.Join(state, "device.crt"), []byte(reply.Certificate)); e != nil {
		return e
	}
	if caFile != "" {
		data, e := os.ReadFile(caFile)
		if e != nil {
			return e
		}
		if e = writeNew(filepath.Join(state, "manager-ca.crt"), data); e != nil {
			return e
		}
	}
	data, _ := json.Marshal(config{reply.DeviceID, reply.Endpoint})
	if e = writeNew(filepath.Join(state, "device.json"), data); e != nil {
		return e
	}
	fmt.Println("Enrollment complete. Code consumed; private key retained on this device.")
	return nil
}
func connect(state, brokerSocket string) error {
	data, e := os.ReadFile(filepath.Join(state, "device.json"))
	if e != nil {
		return e
	}
	var c config
	if json.Unmarshal(data, &c) != nil {
		return errors.New("invalid device config")
	}
	pair, e := tls.LoadX509KeyPair(filepath.Join(state, "device.crt"), filepath.Join(state, "device.key"))
	if e != nil {
		return e
	}
	caPath := filepath.Join(state, "manager-ca.crt")
	if _, e = os.Stat(caPath); os.IsNotExist(e) {
		caPath = ""
	}
	trust, e := roots(caPath)
	if e != nil {
		return e
	}
	dial := websocket.Dialer{HandshakeTimeout: 15 * time.Second, TLSClientConfig: &tls.Config{MinVersion: tls.VersionTLS13, RootCAs: trust, Certificates: []tls.Certificate{pair}}}
	conn, response, e := dial.Dial(c.Endpoint, nil)
	if response != nil && response.Body != nil {
		defer response.Body.Close()
	}
	if e != nil {
		return errors.New("manager connection refused or unavailable")
	}
	defer conn.Close()
	conn.SetReadLimit(bridgewire.Limit)
	hostname, _ := os.Hostname()
	log.Print("manager connection established")
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	var writer sync.Mutex
	write := func(msg bridgewire.Message) error {
		writer.Lock()
		defer writer.Unlock()
		conn.SetWriteDeadline(time.Now().Add(5 * time.Second))
		return conn.WriteJSON(msg)
	}
	beat := func() error {
		available := false
		if info, err := os.Stat(brokerSocket); err == nil {
			available = info.Mode()&os.ModeSocket != 0
		}
		return write(bridgewire.Message{Version: 1, Type: "heartbeat", Hostname: hostname, Platform: runtime.GOOS + "/" + runtime.GOARCH, Broker: available})
	}
	if beat() != nil {
		return errors.New("heartbeat send failed")
	}
	go func() {
		ticker := time.NewTicker(15 * time.Second)
		defer ticker.Stop()
		for {
			select {
			case <-ctx.Done():
				return
			case <-ticker.C:
				if beat() != nil {
					conn.Close()
					return
				}
			}
		}
	}()
	workers := make(chan struct{}, 8)
	media := &localMedia{streams: make(map[string]*localViewer), send: func(b []byte) error {
		writer.Lock()
		defer writer.Unlock()
		conn.SetWriteDeadline(time.Now().Add(2 * time.Second))
		return conn.WriteMessage(websocket.BinaryMessage, b)
	}}
	defer media.closeAll()
	lastAck := time.Now()
	for {
		conn.SetReadDeadline(lastAck.Add(45 * time.Second))
		kind, data, err := conn.ReadMessage()
		if err != nil {
			return errors.New("manager connection ended")
		}
		if kind == websocket.BinaryMessage {
			media.receive(data)
			continue
		}
		var reply bridgewire.Message
		if kind != websocket.TextMessage || json.Unmarshal(data, &reply) != nil || reply.Version != 1 {
			return errors.New("heartbeat rejected")
		}
		if reply.Type == "ack" && reply.DeviceID == c.DeviceID {
			lastAck = time.Now()
			continue
		}
		if (reply.Type != "request" && reply.Type != "media.open") || len(reply.ID) != 32 || !bridgewire.Allowed(reply.Method, reply.Path) || (reply.Type == "media.open" && (!strings.HasSuffix(reply.Path, "/offer") || reply.Method != "POST")) {
			return errors.New("invalid broker request")
		}
		select {
		case workers <- struct{}{}:
			go func(msg bridgewire.Message) {
				defer func() { <-workers }()
				var result bridgewire.Message
				if msg.Type == "media.open" {
					result = media.open(ctx, brokerSocket, msg)
				} else {
					result = brokerRequest(ctx, brokerSocket, msg)
				}
				if write(result) != nil {
					conn.Close()
				}
			}(reply)
		default:
			return errors.New("broker capacity exceeded")
		}
	}
}
func main() {
	if len(os.Args) < 2 {
		log.Fatal("usage: dwdesktop-agent enroll|run [options]")
	}
	action := os.Args[1]
	f := flag.NewFlagSet(action, flag.ExitOnError)
	state := f.String("state-dir", "/var/lib/dwdesktop-agent", "private state directory")
	origin := f.String("manager", "", "HTTPS manager URL")
	code := f.String("code", "", "single-use code (visible in process arguments; prefer code-file)")
	codeFile := f.String("code-file", "", "read code from file or /dev/stdin")
	ca := f.String("ca-file", "", "trusted manager CA PEM")
	brokerSocket := f.String("broker-socket", "/run/dwdesktop-session/broker.sock", "local session broker Unix socket")
	f.Parse(os.Args[2:])
	if e := os.MkdirAll(*state, 0700); e != nil {
		log.Fatal("cannot create state directory")
	}
	info, e := os.Lstat(*state)
	if e != nil || !info.IsDir() || info.Mode().Perm()&0077 != 0 {
		log.Fatal("state must be a private non-symlink directory (0700)")
	}
	switch action {
	case "enroll":
		if e := enroll(*state, *origin, *code, *codeFile, *ca); e != nil {
			log.Fatal(e)
		}
	case "run":
		delay := 2 * time.Second
		for {
			started := time.Now()
			if e := connect(*state, *brokerSocket); e != nil {
				log.Print(e)
			}
			var jitter [1]byte
			rand.Read(jitter[:])
			if time.Since(started) > 30*time.Second {
				delay = 2 * time.Second
			}
			time.Sleep(delay + time.Duration(jitter[0])*delay/512)
			if delay < 32*time.Second {
				delay *= 2
			}
		}
	default:
		log.Fatal("unknown action")
	}
}
