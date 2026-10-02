// SPDX-License-Identifier: MIT
// No network, TUN, namespaces, privileges or installed state in ordinary tests.
package main

import (
	"errors"
	"github.com/amnezia-vpn/amneziawg-go/v3/tun"
	"testing"
)

type fakeTun struct {
	tun.Device
	fail bool
}

func (t fakeTun) Write(bufs [][]byte, offset int) (int, error) {
	if t.fail {
		return 0, errors.New("synthetic failure")
	}
	bytes := 0
	for _, packet := range bufs {
		bytes += len(packet) - offset
	}
	return bytes, nil // Match actual upstream Linux NativeTun.Write semantics.
}

func TestByteCountIsNotPacketSliceBound(t *testing.T) {
	o := &observations{Wire: map[int]int{181: 1}, Plain: map[int]int{}}
	dev := observedTun{fakeTun{}, o}
	n, err := dev.Write([][]byte{make([]byte, 80)}, 16)
	if n != 64 || err != nil || o.Plain[64] != 1 || o.snapshot()["padding_size_matches"] != 1 {
		t.Fatal("byte-count observation mismatch")
	}
}

func TestFailedWriteDoesNotClaimPaddingEvidence(t *testing.T) {
	o := &observations{Wire: map[int]int{181: 1}, Plain: map[int]int{}}
	dev := observedTun{fakeTun{fail: true}, o}
	if _, err := dev.Write([][]byte{make([]byte, 80)}, 16); err == nil {
		t.Fatal("failure missing")
	}
	if len(o.Plain) != 0 || o.snapshot()["padding_size_matches"] != 0 {
		t.Fatal("invented evidence")
	}
}

func TestPaddingMatchesAreBoundedByBothObservations(t *testing.T) {
	o := &observations{Wire: map[int]int{181: 2, 182: 100}, Plain: map[int]int{64: 10}}
	if o.snapshot()["padding_size_matches"] != 2 {
		t.Fatal("padding overcount")
	}
}
