// SPDX-License-Identifier: MIT
package main

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"errors"
	"github.com/amnezia-vpn/amneziawg-go/v3/tun"
	"testing"
)

func TestUDPFlowIdentity(t *testing.T) {
	p := ip4(1280)
	p[9] = 17
	copy(p[12:16], []byte{10, 203, 0, 2})
	copy(p[16:20], []byte{10, 203, 0, 1})
	binary.BigEndian.PutUint16(p[20:22], 12345)
	binary.BigEndian.PutUint16(p[22:24], 8090)
	binary.BigEndian.PutUint16(p[24:26], 1260)
	h := sha256.Sum256(p[28:])
	f, ok := udpFlow("rx", p)
	if !ok || f.Size != 1280 || f.PayloadSHA != hex.EncodeToString(h[:]) || f.DestPort != 8090 {
		t.Fatal("UDP identity missing")
	}
	p[9] = 6
	if _, ok := udpFlow("rx", p); ok {
		t.Fatal("TCP admitted as UDP")
	}
	p[9] = 17
	p[19] = 9
	if _, ok := udpFlow("rx", p); ok {
		t.Fatal("wrong target admitted")
	}
	p[19] = 1
	p[6] = 0x20
	f, ok = udpFlow("rx", p)
	if !ok || !f.Fragment || !f.More || f.PayloadSHA != "" {
		t.Fatal("partial fragment claimed full hash")
	}
	p = ip6(56)
	p[6] = 44
	copy(p[8:24], []byte{0xfd, 0x20, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2})
	copy(p[24:40], []byte{0xfd, 0x20, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1})
	p[40] = 17
	binary.BigEndian.PutUint16(p[48:50], 12345)
	binary.BigEndian.PutUint16(p[50:52], 8090)
	f, ok = udpFlow("rx", p)
	if !ok || !f.Fragment || f.More || f.Offset != 0 {
		t.Fatal("atomic fragment metadata lost")
	}
}

func ip4(size int) []byte {
	p := make([]byte, size)
	p[0] = 0x45
	binary.BigEndian.PutUint16(p[2:4], uint16(size))
	return p
}
func ip6(size int) []byte {
	p := make([]byte, size)
	p[0] = 0x60
	p[6] = 17
	binary.BigEndian.PutUint16(p[4:6], uint16(size-40))
	return p
}

func TestIPShapes(t *testing.T) {
	for _, mtu := range []int{1280, 1420} {
		for _, size := range []int{mtu - 1, mtu, mtu + 1} {
			for family, p := range map[int][]byte{4: ip4(size), 6: ip6(size)} {
				f, n, frag, ok := shape(p)
				if !ok || f != family || n != size || frag != 0 {
					t.Fatal("shape mismatch")
				}
			}
		}
	}
}
func TestFragments(t *testing.T) {
	p := ip4(1280)
	p[6] = 0x20
	if _, _, frag, ok := shape(p); !ok || frag != 1 {
		t.Fatal("missing v4 fragment")
	}
	p = ip6(1280)
	p[6] = 44
	if _, _, frag, ok := shape(p); !ok || frag != 1 {
		t.Fatal("missing v6 fragment")
	}
	p[6] = 0
	p[40] = 44
	p[41] = 0
	if _, _, frag, ok := shape(p); !ok || frag != 1 {
		t.Fatal("missing extension fragment")
	}
}
func TestMalformedShapes(t *testing.T) {
	bad := [][]byte{nil, make([]byte, 19), ip4(20), ip6(40), ip4(30), ip6(48)}
	bad[2][0] = 0x44
	bad[3][6] = 44
	bad[4][3] = 31
	bad[5][6] = 0
	bad[5][41] = 2
	for _, p := range bad {
		if _, _, _, ok := shape(p); ok {
			t.Fatal("malformed packet accepted")
		}
	}
}

type fakeTun struct {
	tun.Device
	err error
}

func (f fakeTun) Write(p [][]byte, offset int) (int, error) { return len(p[0]) - offset, f.err }
func TestByteCountAndFailure(t *testing.T) {
	o := &observations{Packets: map[string]int{}, Outer: map[string]int{}}
	packet := ip4(1280)
	observed := &observedTun{fakeTun{}, o}
	if n, e := observed.Write([][]byte{packet}, 0); n != 1280 || e != nil || o.Packets["rx:4:1280:0"] != 1 {
		t.Fatal("byte contract changed")
	}
	observed.Device = fakeTun{err: errors.New("synthetic")}
	observed.Write([][]byte{packet}, 0)
	if o.Packets["rx:4:1280:0"] != 1 || o.WriteErrors != 1 {
		t.Fatal("failure claimed receipt")
	}
}
