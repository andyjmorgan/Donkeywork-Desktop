package main

import (
	"bytes"
	"encoding/binary"
	"io"
	"net"
	"os"
	"syscall"
	"testing"

	"github.com/pion/webrtc/v4"
)

func unixPair(t *testing.T) (*net.UnixConn, *net.UnixConn) {
	t.Helper()
	fds, err := syscall.Socketpair(syscall.AF_UNIX, syscall.SOCK_STREAM|syscall.SOCK_CLOEXEC, 0)
	if err != nil {
		t.Fatal(err)
	}
	convert := func(fd int) *net.UnixConn {
		file := os.NewFile(uintptr(fd), "test")
		conn, err := net.FileConn(file)
		file.Close()
		if err != nil {
			t.Fatal(err)
		}
		return conn.(*net.UnixConn)
	}
	return convert(fds[0]), convert(fds[1])
}
func TestBrokerReceivesExactlyOneCaptureDescriptor(t *testing.T) {
	for _, count := range []int{0, 1, 2} {
		t.Run(string(rune('0'+count)), func(t *testing.T) {
			client, server := unixPair(t)
			defer client.Close()
			defer server.Close()
			capture, producer := unixPair(t)
			defer capture.Close()
			defer producer.Close()
			file, err := capture.File()
			if err != nil {
				t.Fatal(err)
			}
			defer file.Close()
			rightsFD := int(file.Fd())
			go func() {
				command := make([]byte, 1)
				if _, err := io.ReadFull(server, command); err != nil {
					return
				}
				if command[0] != 'C' {
					t.Error("wrong broker command")
					return
				}
				var rights []byte
				if count == 1 {
					rights = syscall.UnixRights(rightsFD)
				}
				if count == 2 {
					rights = syscall.UnixRights(rightsFD, rightsFD)
				}
				_, _, _ = server.WriteMsgUnix([]byte{'F'}, rights, nil)
			}()
			conn, err := brokerCapture(client)
			if count != 1 {
				if err == nil {
					conn.Close()
					t.Fatal("accepted malformed descriptor count")
				}
				return
			}
			if err != nil {
				t.Fatal(err)
			}
			defer conn.Close()
			go producer.Write([]byte("ok"))
			data := make([]byte, 2)
			if _, err = io.ReadFull(conn, data); err != nil || string(data) != "ok" {
				t.Fatal("received descriptor not connected", err)
			}
		})
	}
}
func TestCaptureRecoveryRetainsViewerAndFlushesOldQueue(t *testing.T) {
	pc, err := webrtc.NewPeerConnection(webrtc.Configuration{})
	if err != nil {
		t.Fatal(err)
	}
	defer pc.Close()
	v := &viewer{pc: pc, frames: make(chan accessUnit, 3), done: make(chan struct{})}
	v.frames <- accessUnit{data: []byte("old"), key: true}
	b := &bridge{viewers: map[*viewer]bool{v: true}}
	b.beginRecovery()
	if !b.recovering || b.generation != 1 || len(v.frames) != 0 || b.viewers[v] {
		t.Fatal("recovery did not clear old media")
	}
	select {
	case <-v.done:
		t.Fatal("recovery closed viewer")
	default:
	}
	if err = b.broadcast(accessUnit{data: []byte("delta")}); err != nil || !b.recovering {
		t.Fatal("delta resumed stream")
	}
	if err = b.broadcast(accessUnit{data: []byte("key"), key: true}); err != nil || b.recovering {
		t.Fatal("keyframe did not resume")
	}
	if b.viewers[v] {
		t.Fatal("unconnected viewer marked ready")
	}
}
func TestCaptureHeaderCompatibility(t *testing.T) {
	a := streamHeader{Protocol: "dwconsole.stream", Version: "0.1.0", Codec: "h264", Bitstream: "annex-b", Encoder: "libx264", Width: 1920, Height: 1080, FPS: 30, PixelFormat: "yuv420p"}
	b := a
	if !compatibleCapture(a, b) {
		t.Fatal("stable codec rejected")
	}
	b.DisplayID = "changed"
	if compatibleCapture(a, b) {
		t.Fatal("different display accepted")
	}
	b = a
	b.Device = "changed"
	if compatibleCapture(a, b) {
		t.Fatal("different device accepted")
	}
	b = a
	b.Width = 1024
	if compatibleCapture(a, b) {
		t.Fatal("wrong source dimensions accepted")
	}
	b = a
	b.FPS = 60
	if compatibleCapture(a, b) {
		t.Fatal("wrong cadence accepted")
	}
	b = a
	b.PixelFormat = "yuv444p"
	if compatibleCapture(a, b) {
		t.Fatal("wrong format accepted")
	}
}
func framedCapture(data []byte) *bytes.Reader {
	var b bytes.Buffer
	_ = binary.Write(&b, binary.BigEndian, uint32(len(data)))
	b.Write(data)
	return bytes.NewReader(b.Bytes())
}
func TestCaptureParserDoesNotCarryStateAcrossFeeds(t *testing.T) {
	b := &bridge{viewers: map[*viewer]bool{}}
	// First source ends partway through its next start code.
	_ = b.readFeed(framedCapture([]byte{0, 0, 1, 0x67, 0x42, 0, 0, 1, 0x68, 0xee, 0, 0}))
	b.beginRecovery()
	// IDR without this source's parameter sets must not inherit old SPS/PPS.
	err := b.readFeed(framedCapture([]byte{0, 0, 1, 0x65, 0x80, 0, 0, 1, 0x41, 0x80, 0, 0, 1, 9, 0xf0}))
	if err == nil || err == io.EOF {
		t.Fatal("old source parameter sets leaked")
	}
	if !b.recovering {
		t.Fatal("invalid source resumed")
	}
	good := []byte{0, 0, 1, 0x67, 0x42, 0xe0, 0x28, 0, 0, 1, 0x68, 0xee, 0, 0, 1, 0x65, 0x80, 0, 0, 1, 0x41, 0x80, 0, 0, 1, 9, 0xf0}
	if err = b.readFeed(framedCapture(good)); err != io.EOF {
		t.Fatal(err)
	}
	if b.recovering {
		t.Fatal("fresh complete keyframe failed to resume")
	}
}
