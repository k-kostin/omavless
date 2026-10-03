//go:build p4_cookie_overlay

// SPDX-License-Identifier: MIT
// Source-engine-only tests; the shared in-memory Bind/TUN open no sockets/FDs.
package device

import (
	"crypto/rand"
	"testing"
	"time"
)

func p4TimerPeer(t *testing.T) (*Device, *Peer, *p4Bind) {
	t.Helper()
	var key [32]byte
	if _, err := rand.Read(key[:]); err != nil {
		t.Fatal("synthetic header key failed")
	}
	d, b, _ := p4Device(t, key[:], false, false)
	remote, err := newPrivateKey()
	if err != nil {
		t.Fatal("synthetic remote key failed")
	}
	p, err := d.NewPeer(remote.publicKey())
	if err != nil {
		t.Fatal("synthetic peer failed")
	}
	p.endpoint.val = p4Endpoint{}
	if d.Up() != nil || !p.timersActive() {
		t.Fatal("in-memory engine not active")
	}
	return d, p, b
}

func p4Stage(t *testing.T, p *Peer) {
	t.Helper()
	e := p.device.NewOutboundElement()
	c := p.device.GetOutboundElementsContainer()
	c.elems = append(c.elems, e)
	p.queue.staged <- c
}

func TestP4TimerBoundaries(t *testing.T) {
	t.Run("defaults", func(t *testing.T) {
		d, p, _ := p4TimerPeer(t)
		if p.retransmitHandshakeTimeout() != 5*time.Second || p.sendKeepaliveTimeout() != 10*time.Second || p.newHandshakeTimeout() != 15*time.Second || d.keyRefreshTimeoutSending() != 120*time.Second || d.keyRefreshTimeoutReceiving() != 165*time.Second || d.keychainExpireTime() != 180*time.Second || d.rekeyMinTimeout() != 5*time.Second || d.maxHandshakeAttemps() != 18 {
			t.Fatal("official default timer contract differs")
		}
	})
	t.Run("ranges", func(t *testing.T) {
		d, p, _ := p4TimerPeer(t)
		if d.IpcSet("rekey_after_time=7-9\nrekey_timeout=2-4\nreject_after_time=20-22\nkeepalive_timeout=3-5\nmax_handshake_attempts=1-3\n") != nil {
			t.Fatal("timer range configuration refused")
		}
		for i := 0; i < 128; i++ {
			within := func(v, lo, hi time.Duration) bool {
				return v >= lo*time.Second && v <= hi*time.Second && v%time.Second == 0
			}
			if !within(p.retransmitHandshakeTimeout(), 2, 4) || !within(p.sendKeepaliveTimeout(), 3, 5) || !within(p.newHandshakeTimeout(), 7, 9) || !within(d.keyRefreshTimeoutSending(), 7, 9) || !within(d.keyRefreshTimeoutReceiving(), 15, 17) || d.keychainExpireTime() != 22*time.Second || d.rekeyMinTimeout() != 2*time.Second {
				t.Fatal("actual timer selection escaped configured bounds")
			}
			if n := d.maxHandshakeAttemps(); n < 1 || n > 3 {
				t.Fatal("actual attempt selection escaped range")
			}
		}
		if d.IpcSet("reject_after_time=2\n") != nil || d.keyRefreshTimeoutReceiving() != 0 {
			t.Fatal("negative receiving refresh interval not clamped")
		}
	})
	t.Run("retry_exhaustion", func(t *testing.T) {
		d, p, b := p4TimerPeer(t)
		if d.IpcSet("reject_after_time=20-22\nmax_handshake_attempts=2\n") != nil {
			t.Fatal("bounded timer configuration refused")
		}
		p.timers.maxHandshakeAttempts.Store(2)
		p.timers.handshakeAttempts.Store(2)
		// Invoke the real callback at its precise >max boundary. This is not
		// elapsed RekeyTimeout or Mihomo-path retry evidence.
		expiredRetransmitHandshake(p, time.Second)
		p4Capture(t, b)
		p.timers.retransmitHandshake.DelSync()
		if p.timers.handshakeAttempts.Load() != 3 {
			t.Fatal("max boundary did not permit the final retry")
		}
		p4Stage(t, p)
		p.timers.sendKeepalive.Mod(time.Hour)
		expiredRetransmitHandshake(p, time.Second)
		if len(p.queue.staged) != 0 || p.timers.sendKeepalive.IsPending() || !p.timers.zeroKeyMaterial.IsPending() || p.timers.handshakeAttempts.Load() != 3 || len(b.sent) != 0 {
			t.Fatal("exhaustion failed to flush/suppress retry")
		}
		p.timers.zeroKeyMaterial.modifyingLock.RLock()
		duration := p.timers.zeroKeyMaterial.duration
		p.timers.zeroKeyMaterial.modifyingLock.RUnlock()
		if duration != 66*time.Second {
			t.Fatal("key residue expiry did not use three times reject upper bound")
		}
		// A second exhaustion must preserve an already scheduled residue timer.
		p.timers.zeroKeyMaterial.Mod(time.Hour)
		expiredRetransmitHandshake(p, time.Second)
		p.timers.zeroKeyMaterial.modifyingLock.RLock()
		duration = p.timers.zeroKeyMaterial.duration
		p.timers.zeroKeyMaterial.modifyingLock.RUnlock()
		if duration != time.Hour {
			t.Fatal("exhaustion renewed pending residue timer")
		}
	})
	t.Run("elapsed_key_expiry", func(t *testing.T) {
		var key [32]byte
		if _, err := rand.Read(key[:]); err != nil {
			t.Fatal("synthetic key failed")
		}
		client, _, clientSK := p4Device(t, key[:], false, false)
		server, _, serverSK := p4Device(t, key[:], false, false)
		cp, err := client.NewPeer(serverSK.publicKey())
		if err != nil {
			t.Fatal("client peer failed")
		}
		sp, err := server.NewPeer(clientSK.publicKey())
		if err != nil {
			t.Fatal("server peer failed")
		}
		if client.Up() != nil || server.Up() != nil {
			t.Fatal("in-memory Noise peers not active")
		}
		init, err := client.CreateMessageInitiation(cp)
		if err != nil {
			t.Fatal("actual Noise initiation creation failed")
		}
		init.Type = MessageInitiationType // the real receive worker normalizes H1
		if server.ConsumeMessageInitiation(init) != sp {
			t.Fatal("actual Noise initiation failed")
		}
		response, err := server.CreateMessageResponse(sp)
		if err != nil {
			t.Fatal("actual Noise response creation failed")
		}
		response.Type = MessageResponseType // the real receive worker normalizes H2
		if client.ConsumeMessageResponse(response) != cp || cp.BeginSymmetricSession() != nil {
			t.Fatal("actual Noise session derivation failed")
		}
		cp.keypairs.RLock()
		kp := cp.keypairs.current
		cp.keypairs.RUnlock()
		if kp == nil || client.indexTable.Lookup(kp.localIndex).keypair != kp {
			t.Fatal("derived keypair missing from actual index")
		}
		// Retain a genuine second partial Noise exchange as well as the session.
		if _, err := client.CreateMessageInitiation(cp); err != nil {
			t.Fatal("partial exchange failed")
		}
		partial := cp.handshake.localIndex
		p4Stage(t, cp)
		completed := make(chan struct{})
		cp.timers.zeroKeyMaterial.DelSync()
		cp.timers.zeroKeyMaterial = cp.NewTimer(func(p *Peer, d time.Duration) { expiredZeroKeyMaterial(p, d); close(completed) })
		// Real Timer elapsed firing, deliberately shortened by the test. It does
		// not prove wall-clock RejectAfterTime, data rejection or transport expiry.
		cp.timers.zeroKeyMaterial.Mod(time.Millisecond)
		select {
		case <-completed:
		case <-time.After(2 * time.Second):
			t.Fatal("real expiry callback deadline")
		}
		cp.timers.zeroKeyMaterial.DelSync()
		cp.keypairs.RLock()
		empty := cp.keypairs.current == nil && cp.keypairs.previous == nil && cp.keypairs.next.Load() == nil
		cp.keypairs.RUnlock()
		if !empty || len(cp.queue.staged) != 0 || cp.handshake.state != handshakeZeroed || cp.handshake.localIndex != 0 || cp.handshake.localEphemeral != (NoisePrivateKey{}) || client.indexTable.Lookup(kp.localIndex).keypair != nil || client.indexTable.Lookup(partial).handshake != nil {
			t.Fatal("elapsed expiry retained session/partial/staged state")
		}
	})
}
