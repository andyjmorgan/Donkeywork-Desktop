package main

import (
	"errors"
	"net"
	"os"
	"syscall"
	"time"
)

// Rights received on the private inherited socket are the only way this
// unprivileged process obtains another capture connection.
func brokerCapture(broker *net.UnixConn) (net.Conn, error) {
	if err := broker.SetDeadline(time.Now().Add(2 * time.Second)); err != nil {
		return nil, err
	}
	if n, err := broker.Write([]byte{'C'}); err != nil || n != 1 {
		return nil, errors.New("capture request failed")
	}
	payload := make([]byte, 1)
	oob := make([]byte, syscall.CmsgSpace(8*4))
	n, on, flags, _, readErr := broker.ReadMsgUnix(payload, oob)
	var fds []int
	messages, parseErr := syscall.ParseSocketControlMessage(oob[:on])
	for _, message := range messages {
		rights, err := syscall.ParseUnixRights(&message)
		if err != nil {
			parseErr = err
		} else {
			fds = append(fds, rights...)
		}
	}
	defer func() {
		for _, fd := range fds {
			_ = syscall.Close(fd)
		}
	}()
	if readErr != nil || parseErr != nil || flags&(syscall.MSG_CTRUNC|syscall.MSG_TRUNC) != 0 || n != 1 {
		return nil, errors.New("invalid capture broker response")
	}
	if payload[0] == 'E' && len(fds) == 0 {
		return nil, errors.New("capture not ready")
	}
	if payload[0] != 'F' || len(fds) != 1 {
		return nil, errors.New("invalid capture descriptor count")
	}
	syscall.CloseOnExec(fds[0])
	file := os.NewFile(uintptr(fds[0]), "replacement-capture")
	fds = nil // File owns the received descriptor; net.FileConn duplicates it.
	defer file.Close()
	conn, err := net.FileConn(file)
	if err != nil {
		return nil, err
	}
	if _, ok := conn.(*net.UnixConn); !ok {
		conn.Close()
		return nil, errors.New("capture descriptor is not Unix socket")
	}
	return conn, nil
}

func compatibleCapture(a, b streamHeader) bool {
	return a.Device == b.Device && a.DisplayID == b.DisplayID && a.Protocol == b.Protocol && a.Version == b.Version && a.Codec == b.Codec && a.Bitstream == b.Bitstream &&
		a.Width == b.Width && a.Height == b.Height && a.FPS == b.FPS && a.PixelFormat == b.PixelFormat && a.Encoder == b.Encoder
}
func (b *bridge) beginRecovery() {
	if b.input != nil {
		b.input.pause()
	}
	b.mu.Lock()
	defer b.mu.Unlock()
	b.recovering = true
	b.generation++
	for v := range b.viewers {
		b.viewers[v] = false
		for {
			select {
			case <-v.frames:
			default:
				goto drained
			}
		}
	drained:
	}
}
func (b *bridge) stopRecovery() {
	b.mu.Lock()
	defer b.mu.Unlock()
	if b.recoveryDone != nil {
		select {
		case <-b.recoveryDone:
		default:
			close(b.recoveryDone)
		}
	}
	if b.captureConn != nil {
		_ = b.captureConn.Close()
	}
	if b.captureBroker != nil {
		_ = b.captureBroker.Close()
	}
}
func (b *bridge) readRecoverable(initial net.Conn) {
	conn := initial
	for {
		_ = b.readFeed(deadlineReader{conn})
		_ = conn.Close()
		b.beginRecovery()
		for {
			select {
			case <-b.recoveryDone:
				return
			default:
			}
			next, err := brokerCapture(b.captureBroker)
			if err == nil {
				_ = next.SetReadDeadline(time.Now().Add(2 * time.Second))
				header, headerErr := readHeader(next)
				if headerErr == nil && compatibleCapture(b.header, header) {
					b.mu.Lock()
					b.captureConn = next
					b.mu.Unlock()
					conn = next
					break
				}
				_ = next.Close()
			}
			timer := time.NewTimer(250 * time.Millisecond)
			select {
			case <-b.recoveryDone:
				timer.Stop()
				return
			case <-timer.C:
			}
		}
	}
}
