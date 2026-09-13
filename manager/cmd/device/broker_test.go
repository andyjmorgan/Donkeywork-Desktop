package main

import (
	"context"
	"donkeywork-desktop/manager/bridgewire"
	"net"
	"net/http"
	"path/filepath"
	"testing"
)

func TestLocalBroker(t *testing.T) {
	socket := filepath.Join(t.TempDir(), "broker.sock")
	listener, err := net.Listen("unix", socket)
	if err != nil {
		t.Fatal(err)
	}
	server := &http.Server{Handler: http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/api/desktops" || r.Host != "localhost" || r.Header.Get("Origin") != "http://localhost" {
			t.Error("unexpected request")
		}
		w.Header().Set("Content-Type", "application/json")
		w.Write([]byte(`{"desktops":[]}`))
	})}
	go server.Serve(listener)
	defer server.Close()
	msg := bridgewire.Message{ID: "test", Method: "GET", Path: "/api/desktops"}
	got := brokerRequest(context.Background(), socket, msg)
	if got.Status != 200 || got.ID != "test" || string(got.Body) != `{"desktops":[]}` {
		t.Fatalf("unexpected reply: %+v", got)
	}
	for _, path := range []string{"http://evil/", "/api/exec", "/api/desktops/../environments", "/api/desktops?url=evil"} {
		msg.Path = path
		if brokerRequest(context.Background(), socket, msg).Status != 503 {
			t.Fatal("unsafe route accepted")
		}
	}
	if brokerRequest(context.Background(), "", msg).Status != 503 {
		t.Fatal("missing socket accepted")
	}
}
