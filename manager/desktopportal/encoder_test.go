package desktopportal

import (
	"bytes"
	"io"
	"os/exec"
	"testing"

	"github.com/pion/rtp"
)

func TestAnnexBLargeAccessUnitDirectDelivery(t *testing.T) {
	// A keyframe larger than the default UDP socket buffer must not lose
	// packets. Delivery is synchronous and bounded by one access unit.
	stream := []byte{0, 0, 0, 1, 9, 0xf0, 0, 0, 0, 1, 0x65}
	stream = append(stream, bytes.Repeat([]byte{0x55}, 512*1024)...)
	stream = append(stream, 0, 0, 0, 1, 9, 0xf0, 0, 0, 0, 1, 0x41, 0x55)
	var count, markers int
	var lastSequence uint16
	var firstTimestamp uint32
	relayAnnexB(io.NopCloser(bytes.NewReader(stream)), &exec.Cmd{}, func(p *rtp.Packet) error {
		if count == 0 {
			firstTimestamp = p.Timestamp
		} else if p.SequenceNumber != lastSequence+1 {
			t.Fatal("packet sequence gap")
		}
		if p.MarshalSize() > 1200 {
			t.Fatal("MTU exceeded")
		}
		lastSequence = p.SequenceNumber
		count++
		if p.Marker {
			if p.Timestamp != firstTimestamp+uint32(markers)*3000 {
				t.Fatal("incorrect frame timestamp")
			}
			markers++
		}
		return nil
	})
	if count < 400 || markers != 2 {
		t.Fatalf("packets=%d frames=%d", count, markers)
	}
}
