package main

import (
	"bytes"
	"encoding/binary"
	"encoding/json"
	"errors"
	"fmt"
	"io"
)

const maxChunk = 1024 * 1024
const maxAU = 16 * 1024 * 1024

type streamHeader struct {
	Protocol    string `json:"protocol"`
	Version     string `json:"version"`
	Codec       string `json:"codec"`
	Bitstream   string `json:"bitstream"`
	Encoder     string `json:"encoder"`
	DisplayID   string `json:"displayId"`
	Device      string `json:"device"`
	Width       uint32 `json:"width"`
	Height      uint32 `json:"height"`
	FPS         uint32 `json:"framesPerSecond"`
	PixelFormat string `json:"pixelFormat"`
}

func readRecord(r io.Reader, maximum uint32) ([]byte, error) {
	var size uint32
	if err := binary.Read(r, binary.BigEndian, &size); err != nil {
		return nil, err
	}
	if size == 0 || size > maximum {
		return nil, errors.New("invalid record length")
	}
	b := make([]byte, size)
	_, err := io.ReadFull(r, b)
	return b, err
}

func readHeader(r io.Reader) (streamHeader, error) {
	var h streamHeader
	b, err := readRecord(r, 64*1024)
	if err != nil {
		return h, err
	}
	d := json.NewDecoder(bytes.NewReader(b))
	d.DisallowUnknownFields()
	if err = d.Decode(&h); err != nil {
		return h, err
	}
	if d.Decode(new(any)) != io.EOF {
		return h, errors.New("trailing header data")
	}
	if h.Protocol != "dwconsole.stream" || h.Version != "0.1.0" || h.Codec != "h264" || h.Bitstream != "annex-b" || h.Width == 0 || h.Height == 0 || h.Width > 4096 || h.Height > 4096 || h.FPS < 1 || h.FPS > 60 || h.DisplayID == "" || h.Device == "" || h.PixelFormat != "yuv420p" {
		return h, errors.New("unsupported stream header")
	}
	return h, nil
}

// The daemon's records are arbitrary byte chunks. Neither record boundaries nor
// NAL boundaries are frame boundaries. Retain incomplete start codes across reads.
type annexParser struct {
	pending []byte
	started bool
}

func startCode(b []byte) (int, int) {
	for i := 0; i+2 < len(b); i++ {
		if b[i] == 0 && b[i+1] == 0 {
			if b[i+2] == 1 {
				return i, 3
			}
			if i+3 < len(b) && b[i+2] == 0 && b[i+3] == 1 {
				return i, 4
			}
		}
	}
	return -1, 0
}
func (p *annexParser) push(chunk []byte, emit func([]byte) error) error {
	if len(p.pending)+len(chunk) > maxAU+maxChunk {
		return errors.New("NAL exceeds limit")
	}
	p.pending = append(p.pending, chunk...)
	for {
		if !p.started {
			i, n := startCode(p.pending)
			if i < 0 {
				if len(p.pending) > 4 {
					return errors.New("missing Annex B start code")
				}
				return nil
			}
			for _, v := range p.pending[:i] {
				if v != 0 {
					return errors.New("invalid Annex B prefix")
				}
			}
			p.pending = p.pending[i+n:]
			p.started = true
		}
		i, n := startCode(p.pending)
		if i < 0 {
			return nil
		}
		if i == 0 {
			return errors.New("empty NAL")
		}
		if err := emit(p.pending[:i]); err != nil {
			return err
		}
		p.pending = p.pending[i+n:]
	}
}

type accessUnit struct {
	generation uint64
	data       []byte
	key        bool
}
type auParser struct {
	data     []byte
	vcl      bool
	key      bool
	sps, pps []byte
}

// first_mb_in_slice is the first unsigned Exp-Golomb field. We only accept
// progressive, no-B-frame AVC from the current encoder (no data partitions).
func firstMacroblock(nal []byte) (uint32, error) {
	if len(nal) < 2 {
		return 0, errors.New("short slice")
	}
	var rbsp []byte
	zeros := 0
	for _, v := range nal[1:] {
		if zeros == 2 && v == 3 {
			zeros = 0
			continue
		}
		rbsp = append(rbsp, v)
		if v == 0 {
			zeros++
		} else {
			zeros = 0
		}
		if len(rbsp) >= 9 {
			break
		}
	}
	bit := func(i int) uint32 { return uint32((rbsp[i/8] >> uint(7-i%8)) & 1) }
	z := 0
	for z < len(rbsp)*8 && bit(z) == 0 {
		z++
	}
	if z > 31 || 2*z+1 > len(rbsp)*8 {
		return 0, errors.New("invalid first_mb_in_slice")
	}
	v := uint32(1)
	for i := z + 1; i <= 2*z; i++ {
		v = v<<1 | bit(i)
	}
	return v - 1, nil
}
func (p *auParser) flush(emit func(accessUnit) error) error {
	if !p.vcl {
		return nil
	}
	data := p.data
	if p.key {
		if len(p.sps) == 0 || len(p.pps) == 0 {
			return errors.New("IDR missing parameter sets")
		}
		data = append(append(append([]byte{}, p.sps...), p.pps...), data...)
	}
	err := emit(accessUnit{data: data, key: p.key})
	p.data = nil
	p.vcl = false
	p.key = false
	return err
}
func (p *auParser) nal(nal []byte, emit func(accessUnit) error) error {
	if len(nal) == 0 || nal[0]&0x80 != 0 {
		return errors.New("invalid NAL")
	}
	kind := nal[0] & 31
	if (kind == 7 || kind == 8) && len(nal) > 64*1024 {
		return errors.New("parameter set exceeds limit")
	}
	if kind == 2 || kind == 3 || kind == 4 || kind == 19 || kind == 20 {
		return errors.New("unsupported AVC partition/extension")
	}
	if kind == 1 || kind == 5 {
		first, err := firstMacroblock(nal)
		if err != nil {
			return err
		}
		if first == 0 && p.vcl {
			if err = p.flush(emit); err != nil {
				return err
			}
		}
		p.vcl = true
		p.key = p.key || kind == 5
	} else if kind == 6 || kind == 7 || kind == 8 || kind == 9 {
		if err := p.flush(emit); err != nil {
			return err
		}
	}
	framed := append([]byte{0, 0, 0, 1}, nal...)
	if kind == 7 {
		p.sps = framed
		return nil
	}
	if kind == 8 {
		p.pps = framed
		return nil
	}
	if kind == 9 {
		return nil
	}
	if len(p.data)+len(framed) > maxAU {
		return fmt.Errorf("access unit exceeds %d bytes", maxAU)
	}
	p.data = append(p.data, framed...)
	return nil
}
