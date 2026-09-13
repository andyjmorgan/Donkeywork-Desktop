package desktopportal

import (
	"context"
	"fmt"
	"github.com/pion/rtp"
	"github.com/pion/rtp/codecs"
	"github.com/pion/webrtc/v4/pkg/media/h264reader"
	"io"
	"net"
	"os"
	"os/exec"
	"strings"
)

func HasEncoder() bool {
	if exec.Command("gst-inspect-1.0", "x264enc").Run() == nil {
		return true
	}
	out, err := exec.Command(ffmpegPath(), "-hide_banner", "-encoders").Output()
	return err == nil && strings.Contains(string(out), "libx264")
}
func ffmpegPath() string {
	if _, err := os.Stat("/opt/donkeywork-desktop/bin/ffmpeg"); err == nil {
		return "/opt/donkeywork-desktop/bin/ffmpeg"
	}
	return "/usr/bin/ffmpeg"
}

// Keep using the fleet's existing libx264 build where the distro does not ship
// GStreamer's encoder plugin. PipeWire still owns capture and permission scoping.
func startEncoder(ctx context.Context, cmd *exec.Cmd, width, height uint32, udp *net.UDPConn, writePacket func(*rtp.Packet) error) (*exec.Cmd, error) {
	if os.Getenv("XDG_SESSION_TYPE") == "x11" {
		replacement := exec.Command("/usr/bin/ffmpeg", "-hide_banner", "-loglevel", "error", "-f", "x11grab", "-draw_mouse", "1", "-video_size", fmt.Sprintf("%dx%d", width, height), "-framerate", "30", "-i", os.Getenv("DISPLAY"), "-an", "-c:v", "libx264", "-preset", "ultrafast", "-tune", "zerolatency", "-pix_fmt", "yuv420p", "-profile:v", "baseline", "-g", "30", "-bf", "0", "-crf", "23", "-x264-params", "aud=1", "-flush_packets", "1", "-f", "h264", "pipe:1")
		// Preserve the existing CommandContext cancellation closure: copying a
		// different Cmd would make cancellation dereference its unstarted Process.
		cmd.Path, cmd.Args = replacement.Path, replacement.Args
		cmd.Stderr = os.Stderr
		encoded, err := cmd.StdoutPipe()
		if err != nil {
			return nil, err
		}
		if err = cmd.Start(); err != nil {
			return nil, err
		}
		go relayAnnexB(encoded, cmd, writePacket)
		return nil, nil
	}
	if exec.Command("gst-inspect-1.0", "x264enc").Run() == nil {
		return nil, cmd.Start()
	}
	index := -1
	for i, arg := range cmd.Args {
		if arg == "x264enc" {
			index = i
			break
		}
	}
	if index < 0 {
		return nil, fmt.Errorf("encoder pipeline missing")
	}
	cmd.Args = append(cmd.Args[:index], "fdsink", "fd=1")
	reader, writer, err := os.Pipe()
	if err != nil {
		return nil, err
	}
	defer reader.Close()
	defer writer.Close()
	encoder := exec.CommandContext(ctx, ffmpegPath(), "-hide_banner", "-loglevel", "error", "-f", "rawvideo", "-pixel_format", "yuv420p", "-video_size", fmt.Sprintf("%dx%d", width, height), "-framerate", "30", "-i", "pipe:0", "-an", "-c:v", "libx264", "-preset", "ultrafast", "-tune", "zerolatency", "-profile:v", "baseline", "-g", "30", "-bf", "0", "-crf", "23", "-f", "rtp", fmt.Sprintf("rtp://127.0.0.1:%d?pkt_size=1200", udp.LocalAddr().(*net.UDPAddr).Port))
	encoder.Stdin = reader
	// The lab's minimal Rocky FFmpeg has the H.264 muxer, not the RTP muxer.
	encoder.Args = append(encoder.Args[:len(encoder.Args)-3], "-x264-params", "aud=1", "-f", "h264", "pipe:1")
	encoded, err := encoder.StdoutPipe()
	if err != nil {
		return nil, err
	}
	encoder.Stderr = os.Stderr
	cmd.Stdout = writer
	if err = encoder.Start(); err != nil {
		return nil, err
	}
	if err = cmd.Start(); err != nil {
		_ = encoder.Process.Kill()
		_ = encoder.Wait()
		return nil, err
	}
	go relayAnnexB(encoded, encoder, writePacket)
	return encoder, nil
}
func relayAnnexB(encoded io.ReadCloser, encoder *exec.Cmd, writePacket func(*rtp.Packet) error) {
	defer encoded.Close()
	nals, err := h264reader.NewReader(encoded)
	if err != nil {
		_ = encoder.Process.Kill()
		return
	}
	packetizer := rtp.NewPacketizer(1200, 96, 87654321, &codecs.H264Payloader{}, rtp.NewRandomSequencer(), 90000)
	var frame []byte
	flush := func() error {
		if len(frame) == 0 {
			return nil
		}
		for _, packet := range packetizer.Packetize(frame, 3000) {
			if err := writePacket(packet); err != nil {
				return err
			}
		}
		frame = nil
		return nil
	}
	for {
		nal, err := nals.NextNAL()
		if err != nil {
			_ = flush()
			return
		}
		if nal.UnitType == 9 {
			if flush() != nil {
				_ = encoder.Process.Kill()
				return
			}
		}
		frame = append(frame, 0, 0, 0, 1)
		frame = append(frame, nal.Data...)
		if len(frame) > 16*1024*1024 {
			_ = encoder.Process.Kill()
			return
		}
	}
}
