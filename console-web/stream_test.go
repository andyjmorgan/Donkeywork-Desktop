package main

import (
	"bytes"
	"encoding/binary"
	"io"
	"testing"
)

func TestAnnexBAtEveryChunkBoundary(t *testing.T) {
	input := []byte{0, 0, 0, 1, 0x67, 0x42, 0xe0, 0x28, 0, 0, 1, 0x68, 0xee, 0, 0, 0, 1, 0x65, 0x80, 0, 0, 1, 0x41, 0x80, 0, 0, 1, 0x41, 0x80, 0, 0, 1, 9, 0xf0}
	for chunk := 1; chunk <= len(input); chunk++ {
		p := annexParser{}
		au := auParser{}
		var samples []accessUnit
		emit := func(n []byte) error {
			return au.nal(n, func(a accessUnit) error { samples = append(samples, a); return nil })
		}
		for offset := 0; offset < len(input); offset += chunk {
			end := offset + chunk
			if end > len(input) {
				end = len(input)
			}
			if err := p.push(input[offset:end], emit); err != nil {
				t.Fatalf("chunk %d: %v", chunk, err)
			}
		}
		if len(samples) != 2 || !samples[0].key || samples[1].key {
			t.Fatalf("chunk %d: wrong AU boundaries %#v", chunk, samples)
		}
		if !bytes.Contains(samples[0].data, []byte{0, 0, 0, 1, 0x67}) || !bytes.Contains(samples[0].data, []byte{0, 0, 0, 1, 0x68}) {
			t.Fatal("IDR missing cached parameter sets")
		}
	}
}
func TestSlicesShareAnAccessUnit(t *testing.T) {
	p := auParser{sps: []byte{0, 0, 1, 0x67}, pps: []byte{0, 0, 1, 0x68}}
	var samples []accessUnit
	emit := func(a accessUnit) error { samples = append(samples, a); return nil }
	for _, n := range [][]byte{{0x65, 0x80}, {0x65, 0x40}, {0x41, 0x80}} {
		if err := p.nal(n, emit); err != nil {
			t.Fatal(err)
		}
	}
	if len(samples) != 1 || bytes.Count(samples[0].data, []byte{0, 0, 0, 1, 0x65}) != 2 {
		t.Fatal("multi-slice frame split incorrectly")
	}
}
func TestMalformedAndOversized(t *testing.T) {
	for _, size := range []uint32{0, maxChunk + 1} {
		var b bytes.Buffer
		_ = binary.Write(&b, binary.BigEndian, size)
		if _, err := readRecord(&b, maxChunk); err == nil {
			t.Fatal("accepted invalid length")
		}
	}
	if _, err := readRecord(bytes.NewReader([]byte{0, 0, 0, 2, 1}), maxChunk); err != io.ErrUnexpectedEOF {
		t.Fatalf("truncation %v", err)
	}
	if _, err := firstMacroblock([]byte{0x65, 0}); err == nil {
		t.Fatal("accepted invalid slice")
	}
	p := annexParser{}
	if err := p.push([]byte{1, 2, 3, 4, 5}, func([]byte) error { return nil }); err == nil {
		t.Fatal("accepted invalid prefix")
	}
	if err := p.push(make([]byte, maxAU+maxChunk+1), func([]byte) error { return nil }); err == nil {
		t.Fatal("accepted unbounded NAL")
	}
}
func TestInvalidHeader(t *testing.T) {
	for _, data := range []string{`{}`, `{"protocol":"wrong"}`, `{"x":1}`, `{} {}`} {
		var b bytes.Buffer
		_ = binary.Write(&b, binary.BigEndian, uint32(len(data)))
		b.WriteString(data)
		if _, err := readHeader(&b); err == nil {
			t.Fatal("accepted invalid header")
		}
	}
}
