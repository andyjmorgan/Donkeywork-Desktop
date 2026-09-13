package main

import (
	"context"
	"donkeywork-desktop/manager/bridgewire"
	"os"
	"testing"
	"time"
)

func TestPilotLocalMedia(t *testing.T) {
	path := os.Getenv("DW_TEST_DESKTOP_OFFER")
	if path == "" {
		t.Skip("explicit existing pilot offer path required")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	video := make(chan struct{}, 1)
	m := &localMedia{streams: make(map[string]*localViewer), send: func(b []byte) error {
		if len(b) > 33 && b[32] == bridgewire.Video {
			select {
			case video <- struct{}{}:
			default:
			}
		}
		return nil
	}}
	defer m.closeAll()
	reply := m.open(ctx, "/run/dwdesktop-session/broker.sock", bridgewire.Message{ID: "00112233445566778899aabbccddeeff", Method: "POST", Path: path})
	if reply.Status != 200 {
		t.Fatal("local media rejected")
	}
	select {
	case <-video:
	case <-ctx.Done():
		t.Fatal("no local video")
	}
}
