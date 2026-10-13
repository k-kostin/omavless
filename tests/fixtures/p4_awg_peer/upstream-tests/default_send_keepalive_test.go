//go:build p4_cookie_overlay && p4_default_residue_overlay && p4_default_keepalive_overlay

// SPDX-License-Identifier: MIT
// Add-only ordinary channel case. No timer/callback/key-age/worker substitution.
package device

import (
	"bytes"
	"crypto/rand"
	"encoding/binary"
	"encoding/hex"
	"fmt"
	"net/netip"
	"os"
	"runtime"
	"sync"
	"testing"
	"time"

	"github.com/amnezia-vpn/amneziawg-go/v3/conn"
)

type p4KeepaliveLog struct {
	base     *p4ResidueLog
	mu       sync.Mutex
	received []time.Time
}

func (l *p4KeepaliveLog) observe(format string, args ...any) {
	l.base.observe(format, args...) // Fixed literals only; arguments stay uninspected.
	if format == "%v - Receiving keepalive packet" {
		l.mu.Lock()
		defer l.mu.Unlock()
		if len(l.received) >= p4ResidueEventCap {
			l.base.failed.Store(true)
			return
		}
		l.received = append(l.received, time.Now())
	}
}

func (l *p4KeepaliveLog) snapshot() []time.Time {
	l.mu.Lock()
	defer l.mu.Unlock()
	return append([]time.Time(nil), l.received...)
}

type p4KeepaliveBind struct {
	*p4ResidueBind
	mu    sync.Mutex
	empty []p4ResidueHeld
}

func p4KeepaliveZero(b *p4ResidueBind, packet []byte) (bool, p4RekeyWire, error) {
	plain, kind, err := p4ResidueHeader(b.device, packet)
	if err != nil || kind != MessageTransportType || len(plain) < MessageTransportHeaderSize+16 || b.other == nil || binary.LittleEndian.Uint32(plain[:4]) != 404 {
		return false, p4RekeyWire{}, os.ErrInvalid
	}
	wire := p4RekeyWire{kind: kind, index: binary.LittleEndian.Uint32(plain[4:8]), counter: binary.LittleEndian.Uint64(plain[8:16]), protected: true}
	entry := b.other.device.indexTable.Lookup(wire.index)
	if entry.peer == nil || entry.keypair == nil {
		return false, wire, os.ErrInvalid
	}
	// Retain the actual alias under the receiver's real keypair lock. Pure AEAD
	// Open observes original ciphertext; it never calls ValidateCounter or
	// changes the original replay window, peer, key epoch or packet bytes.
	entry.peer.keypairs.RLock()
	defer entry.peer.keypairs.RUnlock()
	key := entry.keypair
	if key.receive == nil || key != entry.peer.keypairs.current && key != entry.peer.keypairs.previous && key != entry.peer.keypairs.next.Load() {
		return false, wire, os.ErrInvalid
	}
	var nonce [12]byte
	binary.LittleEndian.PutUint64(nonce[4:], wire.counter)
	decoded, err := key.receive.Open(nil, nonce[:], plain[MessageTransportHeaderSize:], nil)
	if err != nil {
		return false, wire, err
	}
	return len(decoded) == 0, wire, nil // Plaintext/key bytes are never retained/logged.
}

func (b *p4KeepaliveBind) Send(packets [][]byte, endpoint conn.Endpoint) error {
	for _, packet := range packets {
		at := time.Now() // Actual Bind Send entry, not an observer wake-up.
		_, kind, err := p4ResidueHeader(b.device, packet)
		if err != nil {
			b.failed.Store(true)
			return err
		}
		if kind == MessageTransportType {
			zero, wire, err := p4KeepaliveZero(b.p4ResidueBind, packet)
			if err != nil {
				b.failed.Store(true)
				return err
			}
			if zero {
				wire.at = at
				b.mu.Lock()
				if len(b.empty) >= p4ResidueEventCap {
					b.mu.Unlock()
					b.failed.Store(true)
					return os.ErrInvalid
				}
				b.empty = append(b.empty, p4ResidueHeld{bytes.Clone(packet), wire})
				b.mu.Unlock()
			}
		}
	}
	return b.p4ResidueBind.Send(packets, endpoint)
}

func (b *p4KeepaliveBind) snapshot() []p4ResidueHeld {
	b.mu.Lock()
	defer b.mu.Unlock()
	return append([]p4ResidueHeld(nil), b.empty...)
}

func p4KeepaliveNewDevice(t *testing.T, header []byte) (*p4ResidueDevice, *p4KeepaliveBind, *p4KeepaliveLog) {
	t.Helper()
	if runtime.NumCPU() < 1 || runtime.NumCPU() > 32 {
		t.Fatal("finite keepalive worker capacity refused")
	}
	f := &p4ResidueDevice{b: &p4ResidueBind{p4RekeyBind: p4RekeyBind{incoming: make(chan []byte, 128)}}, tun: p4RekeyNewTun(), log: &p4ResidueLog{}, workers: uint32(runtime.NumCPU())}
	b := &p4KeepaliveBind{p4ResidueBind: f.b}
	l := &p4KeepaliveLog{base: f.log}
	logger := NewLogger(LogLevelSilent, "")
	logger.Verbosef = l.observe // Before NewDevice starts any worker.
	f.d = NewDevice(f.tun, b, logger)
	f.b.device = f.d
	t.Cleanup(func() { f.close(t, time.Now().Add(5*time.Second)) })
	var err error
	f.sk, err = newPrivateKey()
	if err != nil || f.d.SetPrivateKey(f.sk) != nil {
		t.Fatal("synthetic keepalive identity refused")
	}
	config := fmt.Sprintf("s1=24\ns2=32\ns3=40\ns4=48\nh1=101\nh2=202\nh3=303\nh4=404\nheader_protection_key=%s\nrandom_trailers=false\ndisable_cookies=false\n", hex.EncodeToString(header))
	if f.d.IpcSet(config) != nil {
		t.Fatal("synthetic keepalive protected shape refused")
	}
	p4RekeyConfiguredRead(t, f.tun, time.Now().Add(time.Second))
	return f, b, l
}

func p4KeepaliveConsumption(b *p4ResidueBind, wire p4RekeyWire, wanted int) (p4ResidueDelivery, bool) {
	b.Lock()
	defer b.Unlock()
	var last p4ResidueDelivery
	count := 0
	for _, event := range b.deliveries {
		if event.index == wire.index && event.count == wire.counter {
			last = event
			count++
		}
	}
	return last, count == wanted && b.entries.Load() > last.entry
}

func TestP4DefaultSendKeepalive(t *testing.T) {
	started := time.Now()
	end := started.Add(30 * time.Second)
	var header [32]byte
	var nonce [16]byte
	if _, err := rand.Read(header[:]); err != nil {
		t.Fatal("synthetic keepalive header refused")
	}
	if _, err := rand.Read(nonce[:]); err != nil {
		t.Fatal("synthetic keepalive nonce refused")
	}
	client, cb, _ := p4KeepaliveNewDevice(t, header[:])
	server, _, sl := p4KeepaliveNewDevice(t, header[:])
	client.b.other, server.b.other = server.b, client.b
	cp, err := client.d.NewPeer(server.sk.publicKey())
	if err != nil {
		t.Fatal("keepalive client peer refused")
	}
	sp, err := server.d.NewPeer(client.sk.publicKey())
	if err != nil {
		t.Fatal("keepalive server peer refused")
	}
	cp.endpoint.val, sp.endpoint.val = p4RekeyEndpoint{}, p4RekeyEndpoint{}
	client.d.allowedips.Insert(netip.MustParsePrefix("198.18.0.2/32"), cp)
	server.d.allowedips.Insert(netip.MustParsePrefix("198.18.0.1/32"), sp)
	if server.d.Up() != nil || client.d.Up() != nil {
		t.Fatal("keepalive channel activation refused")
	}
	for _, row := range []struct {
		d *Device
		p *Peer
	}{{client.d, cp}, {server.d, sp}} {
		if row.d.keychainExpireTime() != 180*time.Second || row.d.keyRefreshTimeoutSending() != 120*time.Second || row.d.keyRefreshTimeoutReceiving() != 165*time.Second || row.p.retransmitHandshakeTimeout() != 5*time.Second || row.p.newHandshakeTimeout() != 15*time.Second || row.p.sendKeepaliveTimeout() != 10*time.Second || row.d.maxHandshakeAttemps() != 18 {
			t.Fatal("keepalive product defaults differ")
		}
	}
	setupEnd := started.Add(5 * time.Second)
	request := p4RekeyPacket(nonce, 1, false)
	p4ResidueInput(t, client.tun, request)
	forward := p4RekeyReceive(t, server.tun, request, setupEnd)
	p4RekeyWait(t, setupEnd, cp.timers.newHandshake.IsPending, "baseline keepalive idle arm missing")
	reply := p4RekeyPacket(nonce, 1, true)
	p4ResidueInput(t, server.tun, reply)
	reverse := p4RekeyReceive(t, client.tun, reply, setupEnd)
	ck, sk := cp.keypairs.Current(), sp.keypairs.Current()
	if ck == nil || sk == nil || !ck.isInitiator || sk.isInitiator || forward.index != sk.localIndex || reverse.index != ck.localIndex || cp.timers.newHandshake.IsPending() {
		t.Fatal("keepalive genuine indexed baseline differs")
	}
	p4RekeyMatch(t, &client.b.p4RekeyBind, forward)
	p4RekeyMatch(t, &server.b.p4RekeyBind, reverse)
	p4RekeyWait(t, setupEnd, sp.timers.newHandshake.IsPending, "genuine responder data idle arm missing")
	if sp.timers.sendKeepalive.IsPending() {
		t.Fatal("responder authenticated send did not cancel keepalive")
	}
	p4RekeyWait(t, setupEnd, cp.timers.sendKeepalive.IsPending, "default keepalive real arm missing")
	p4RekeyWait(t, setupEnd, func() bool {
		_, ok := client.b.consumed(p4RekeyWire{index: reverse.index, counter: reverse.counter})
		return ok
	}, "genuine trigger ReceiveFunc consumption missing")
	trigger, _ := client.b.consumed(p4RekeyWire{index: reverse.index, counter: reverse.counter})
	scheduled := p4ResidueScheduled(t, cp.timers.sendKeepalive, 10*time.Second, 10*time.Second)
	before, receivedBefore := len(cb.snapshot()), len(sl.snapshot())
	keepaliveEnd := trigger.at.Add(13 * time.Second)
	p4RekeyWait(t, keepaliveEnd, func() bool { return len(cb.snapshot()) == before+1 }, "default keepalive emission missing or duplicated")
	empty := cb.snapshot()[before]
	if empty.wire.index != sk.localIndex || empty.wire.at.Sub(trigger.at) < 10*time.Second || empty.wire.at.Sub(trigger.at) > 13*time.Second {
		t.Fatal("actual zero-payload keepalive identity/age differs")
	}
	consumeEnd := empty.wire.at.Add(2 * time.Second)
	p4RekeyWait(t, consumeEnd, func() bool {
		_, consumed := p4KeepaliveConsumption(server.b, empty.wire, 1)
		return consumed && len(sl.snapshot()) == receivedBefore+1 && !sp.timers.newHandshake.IsPending()
	}, "actual authenticated keepalive receiver/idle cancellation missing")
	consumed, _ := p4KeepaliveConsumption(server.b, empty.wire, 1)
	received := sl.snapshot()[receivedBefore]
	if consumed.at.Before(empty.wire.at) || received.Before(consumed.at) || received.Sub(empty.wire.at) > 2*time.Second {
		t.Fatal("keepalive causal receiver timestamps differ")
	}
	p4ResidueRetained(t, cp, ck)
	p4ResidueRetained(t, sp, sk)
	corrupt := bytes.Clone(empty.packet)
	corrupt[len(corrupt)-1] ^= 1 // One original transport authentication-tag bit.
	if _, _, err := p4KeepaliveZero(client.b, corrupt); err == nil {
		t.Fatal("corrupt keepalive tag unexpectedly authenticated")
	}
	if server.b.deliver(corrupt) != nil {
		t.Fatal("corrupt keepalive channel delivery refused")
	}
	p4RekeyWait(t, time.Now().Add(time.Second), func() bool { _, consumed := p4KeepaliveConsumption(server.b, empty.wire, 2); return consumed }, "corrupt packet actual ReceiveFunc return missing")
	quietStart := time.Now()
	quiet := time.NewTimer(1500 * time.Millisecond)
	select {
	case <-server.tun.output:
		quiet.Stop()
		t.Fatal("keepalive/corrupt packet produced fake-TUN data")
	case <-client.tun.output:
		quiet.Stop()
		t.Fatal("unexpected client fake-TUN data")
	case <-quiet.C:
	}
	quietElapsed := time.Since(quietStart)
	if len(sl.snapshot()) != receivedBefore+1 || len(cb.snapshot()) != before+1 || cp.timers.sendKeepalive.IsPending() || sp.timers.sendKeepalive.IsPending() || cp.timers.newHandshake.IsPending() || sp.timers.newHandshake.IsPending() || p4RekeyHandshakeCount(&client.b.p4RekeyBind, MessageInitiationType) != 1 || p4RekeyHandshakeCount(&server.b.p4RekeyBind, MessageInitiationType) != 0 || p4RekeyHandshakeCount(&server.b.p4RekeyBind, MessageResponseType) != 1 {
		t.Fatal("keepalive/corrupt quiet timer/count state differs")
	}
	p4ResidueRetained(t, cp, ck)
	p4ResidueRetained(t, sp, sk)
	closed := time.Now()
	closeEnd := closed.Add(5 * time.Second)
	if end.Before(closeEnd) {
		closeEnd = end
	}
	client.close(t, closeEnd)
	server.close(t, closeEnd)
	if len(sl.snapshot()) != receivedBefore+1 || len(cb.snapshot()) != before+1 || p4RekeyHandshakeCount(&client.b.p4RekeyBind, MessageInitiationType) != 1 || p4RekeyHandshakeCount(&server.b.p4RekeyBind, MessageInitiationType) != 0 || p4RekeyHandshakeCount(&server.b.p4RekeyBind, MessageResponseType) != 1 {
		t.Fatal("keepalive closure changed final transport/handshake counts")
	}
	if !time.Now().Before(end) {
		t.Fatal("keepalive body/closure exceeded30 seconds")
	}
	t.Logf("p4_keepalive_receipt case=keepalive scheduled_ns=%d keepalive_ns=%d consume_ns=%d receive_ns=%d quiet_ns=%d teardown_ns=%d body_ns=%d baseline=true default_keepalive=true actual_zero_payload=true actual_receive=true idle_cancelled=true corrupt_refused=true stable_keys=true no_extra_h1=true quiet_no_tun=true teardown_complete=true defaults_unchanged=true network_fds=false", scheduled, empty.wire.at.Sub(trigger.at), consumed.at.Sub(empty.wire.at), received.Sub(empty.wire.at), quietElapsed, time.Since(closed), time.Since(started))
}
