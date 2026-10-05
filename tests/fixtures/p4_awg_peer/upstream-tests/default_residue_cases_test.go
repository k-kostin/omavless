//go:build p4_cookie_overlay && p4_default_residue_overlay

// SPDX-License-Identifier: MIT
// Source-only proposal until ROOT separately reviews/builds/selects an artifact.
package device

import (
	"crypto/rand"
	"net/netip"
	"testing"
	"time"
)

type p4ResiduePair struct {
	client, server *p4ResidueDevice
	cp, sp         *Peer
	ck, sk         *Keypair
	nonce          [16]byte
	started, end   time.Time
}

func p4ResidueBaseline(t *testing.T) *p4ResiduePair {
	t.Helper()
	f := &p4ResiduePair{started: time.Now()}
	f.end = f.started.Add(570 * time.Second)
	var header [32]byte
	if _, err := rand.Read(header[:]); err != nil {
		t.Fatal("synthetic header generation refused")
	}
	if _, err := rand.Read(f.nonce[:]); err != nil {
		t.Fatal("synthetic nonce generation refused")
	}
	f.client, f.server = p4ResidueNewDevice(t, header[:]), p4ResidueNewDevice(t, header[:])
	f.client.b.other, f.server.b.other = f.server.b, f.client.b
	var err error
	f.cp, err = f.client.d.NewPeer(f.server.sk.publicKey())
	if err != nil {
		t.Fatal("client peer refused")
	}
	f.sp, err = f.server.d.NewPeer(f.client.sk.publicKey())
	if err != nil {
		t.Fatal("server peer refused")
	}
	f.cp.endpoint.val, f.sp.endpoint.val = p4RekeyEndpoint{}, p4RekeyEndpoint{}
	f.client.d.allowedips.Insert(netip.MustParsePrefix("198.18.0.2/32"), f.cp)
	f.server.d.allowedips.Insert(netip.MustParsePrefix("198.18.0.1/32"), f.sp)
	if f.server.d.Up() != nil || f.client.d.Up() != nil {
		t.Fatal("channel peers not active")
	}
	for _, row := range []struct {
		d *Device
		p *Peer
	}{{f.client.d, f.cp}, {f.server.d, f.sp}} {
		if row.d.keychainExpireTime() != 180*time.Second || row.d.keyRefreshTimeoutSending() != 120*time.Second || row.d.keyRefreshTimeoutReceiving() != 165*time.Second || row.p.retransmitHandshakeTimeout() != 5*time.Second || row.p.newHandshakeTimeout() != 15*time.Second || row.p.sendKeepaliveTimeout() != 10*time.Second || row.d.maxHandshakeAttemps() != 18 {
			t.Fatal("unchanged product defaults differ")
		}
	}
	end := f.started.Add(5 * time.Second)
	request := p4RekeyPacket(f.nonce, 1, false)
	p4ResidueInput(t, f.client.tun, request)
	forward := p4RekeyReceive(t, f.server.tun, request, end)
	p4RekeyWait(t, end, f.cp.timers.newHandshake.IsPending, "actual baseline data timer arm missing")
	reply := p4RekeyPacket(f.nonce, 1, true)
	p4ResidueInput(t, f.server.tun, reply)
	reverse := p4RekeyReceive(t, f.client.tun, reply, end)
	f.ck, f.sk = f.cp.keypairs.Current(), f.sp.keypairs.Current()
	if f.ck == nil || f.sk == nil || !f.ck.isInitiator || f.sk.isInitiator || f.ck.remoteIndex != f.sk.localIndex || f.sk.remoteIndex != f.ck.localIndex || forward.index != f.sk.localIndex || reverse.index != f.ck.localIndex || f.cp.timers.newHandshake.IsPending() || p4RekeyHandshakeCount(&f.client.b.p4RekeyBind, MessageInitiationType) != 1 || p4RekeyHandshakeCount(&f.server.b.p4RekeyBind, MessageResponseType) != 1 {
		t.Fatal("genuine indexed bidirectional baseline differs")
	}
	p4RekeyMatch(t, &f.client.b.p4RekeyBind, forward)
	p4RekeyMatch(t, &f.server.b.p4RekeyBind, reverse)
	p4ResidueRetained(t, f.cp, f.ck)
	p4ResidueRetained(t, f.sp, f.sk)
	if ageGap := f.ck.created.Sub(f.sk.created); ageGap < 0 || ageGap > 250*time.Millisecond {
		t.Fatal("baseline key epochs too far apart for539-second joint window")
	}
	p4ResidueScheduled(t, f.cp.timers.zeroKeyMaterial, 540*time.Second, 540*time.Second)
	p4ResidueScheduled(t, f.sp.timers.zeroKeyMaterial, 540*time.Second, 540*time.Second)
	return f
}

func p4ResidueInput(t *testing.T, tun *p4RekeyTun, packet []byte) {
	t.Helper()
	select {
	case tun.input <- packet:
	default:
		t.Fatal("bounded genuine TUN packet admission refused")
	}
}

func p4ResidueScheduled(t *testing.T, timer *Timer, minimum, maximum time.Duration) time.Duration {
	t.Helper()
	// Never sampled around the zero-timer firing edge: its upstream callback
	// resets duration outside modifyingLock. Callback/locked key removal is
	// observed independently at540, without this near-edge timer read.
	timer.modifyingLock.RLock()
	duration := timer.duration
	timer.modifyingLock.RUnlock()
	if duration < minimum || duration > maximum {
		t.Fatal("actual default scheduling target differs")
	}
	return duration
}

func p4ResidueAt(t *testing.T, epoch time.Time, lower, upper time.Duration) time.Time {
	t.Helper()
	target := epoch.Add(lower)
	if !time.Now().Before(epoch.Add(upper)) {
		t.Fatal("finite elapsed observation window missed")
	}
	if duration := time.Until(target); duration > 0 {
		timer := time.NewTimer(duration)
		<-timer.C
	}
	now := time.Now()
	if now.Before(target) || !now.Before(epoch.Add(upper)) {
		t.Fatal("actual monotonic observation outside bounds")
	}
	return now
}

func p4ResidueRetained(t *testing.T, peer *Peer, key *Keypair) {
	t.Helper()
	peer.keypairs.RLock()
	current, previous, next := peer.keypairs.current, peer.keypairs.previous, peer.keypairs.next.Load()
	peer.keypairs.RUnlock()
	if current != key || previous != nil || next != nil || peer.device.indexTable.Lookup(key.localIndex).keypair != key {
		t.Fatal("original current key/index retention differs")
	}
}

func p4ResidueCleared(peer *Peer, key *Keypair, partial uint32) bool {
	peer.keypairs.RLock()
	empty := peer.keypairs.current == nil && peer.keypairs.previous == nil && peer.keypairs.next.Load() == nil
	peer.keypairs.RUnlock()
	peer.handshake.mutex.RLock()
	h := &peer.handshake
	cleared := h.localIndex == 0 && h.state == handshakeZeroed && h.localEphemeral == (NoisePrivateKey{}) && h.remoteEphemeral == (NoisePublicKey{}) && h.chainKey == ([32]byte{}) && h.hash == ([32]byte{})
	peer.handshake.mutex.RUnlock()
	entry := peer.device.indexTable.Lookup(key.localIndex)
	partialEntry := peer.device.indexTable.Lookup(partial)
	return empty && cleared && entry.keypair == nil && entry.peer == nil && partialEntry.keypair == nil && partialEntry.peer == nil && len(peer.queue.staged) == 0
}

func p4ResidueFinish(t *testing.T, f *p4ResiduePair, partial uint32) (time.Duration, time.Duration, time.Duration, time.Duration, time.Duration) {
	t.Helper()
	//539 window observes retained keys, NOT the unsafe duration reset field.
	p4ResidueAt(t, f.ck.created, 539*time.Second, 539500*time.Millisecond)
	p4ResidueRetained(t, f.cp, f.ck)
	p4ResidueRetained(t, f.sp, f.sk)
	if len(f.client.log.events("zero")) != 0 || len(f.server.log.events("zero")) != 0 {
		t.Fatal("zero-key callback before original residue expiry")
	}
	f.cp.handshake.mutex.RLock()
	currentPartial := f.cp.handshake.localIndex
	f.cp.handshake.mutex.RUnlock()
	if partial != 0 && currentPartial != partial {
		t.Fatal("retained genuine partial handshake changed before residue expiry")
	}
	partial = currentPartial
	f.sp.handshake.mutex.RLock()
	serverPartial := f.sp.handshake.localIndex
	f.sp.handshake.mutex.RUnlock()
	end := f.ck.created.Add(550 * time.Second)
	if reserve := f.end.Add(-5 * time.Second); reserve.Before(end) {
		end = reserve
	}
	p4RekeyWait(t, end, func() bool {
		return len(f.client.log.events("zero")) == 1 && len(f.server.log.events("zero")) == 1 && p4ResidueCleared(f.cp, f.ck, partial) && p4ResidueCleared(f.sp, f.sk, serverPartial)
	}, "actual default zero callback/removal incomplete")
	removed := time.Now()
	clientRemoved, serverRemoved := removed.Sub(f.ck.created), removed.Sub(f.sk.created)
	clientZero, serverZero := f.client.log.events("zero")[0], f.server.log.events("zero")[0]
	clientAge, serverAge := clientZero.Sub(f.ck.created), serverZero.Sub(f.sk.created)
	if clientAge < 540*time.Second || clientAge > 550*time.Second || serverAge < 540*time.Second || serverAge > 550*time.Second || clientRemoved < clientAge || serverRemoved < serverAge || clientRemoved > 550*time.Second || serverRemoved > 550*time.Second {
		t.Fatal("residue callback not at original540-second age")
	}
	teardown := time.Now()
	closeEnd := teardown.Add(5 * time.Second)
	if f.end.Before(closeEnd) {
		closeEnd = f.end
	}
	f.client.close(t, closeEnd)
	f.server.close(t, closeEnd)
	if !time.Now().Before(f.end) {
		t.Fatal("case including actual teardown exceeded570 seconds")
	}
	return clientAge, serverAge, clientRemoved, serverRemoved, time.Since(teardown)
}

func TestP4DefaultElapsedRejectAndResidue(t *testing.T) {
	f := p4ResidueBaseline(t)
	f.client.b.holdData.Store(true)
	for _, phase := range []byte{2, 3} {
		p4ResidueInput(t, f.client.tun, p4RekeyPacket(f.nonce, phase, false))
	}
	p4RekeyWait(t, f.started.Add(8*time.Second), func() bool { return len(f.client.b.heldPair()) == 2 }, "two genuine protected sender packets missing")
	held := f.client.b.heldPair()
	if held[0].wire.index != f.sk.localIndex || held[1].wire.index != f.sk.localIndex || held[0].wire.counter == held[1].wire.counter || held[1].wire.at.Sub(f.ck.created) >= 120*time.Second {
		t.Fatal("unused original protected counters/epochs differ")
	}
	p4RekeyWait(t, f.started.Add(8*time.Second), f.cp.timers.newHandshake.IsPending, "held sender's actual idle arm missing")
	p4ResidueInput(t, f.server.tun, p4RekeyPacket(f.nonce, 4, true))
	p4RekeyReceive(t, f.client.tun, p4RekeyPacket(f.nonce, 4, true), f.started.Add(9*time.Second))
	if f.cp.timers.newHandshake.IsPending() {
		t.Fatal("authenticated reverse did not cancel held sender idle")
	}
	f.client.b.holdData.Store(false)
	f.client.b.dropH1.Store(true)
	f.server.b.dropH1.Store(true) // All subsequent H1: idle-loss behavior stays real.
	p4ResidueAt(t, f.sk.created, 177*time.Second, 179*time.Second)
	if f.server.b.deliver(held[0].packet) != nil {
		t.Fatal("early genuine protected delivery refused")
	}
	plain := p4RekeyReceive(t, f.server.tun, p4RekeyPacket(f.nonce, 2, false), f.sk.created.Add(179*time.Second))
	if plain.index != held[0].wire.index || plain.counter != held[0].wire.counter {
		t.Fatal("early exact protected sender/AEAD correlation differs")
	}
	p4RekeyWait(t, f.sk.created.Add(179*time.Second), func() bool { _, ok := f.server.b.consumed(held[0].wire); return ok }, "early actual ReceiveFunc return missing")
	early, _ := f.server.b.consumed(held[0].wire)
	if early.at.Sub(f.sk.created) < 177*time.Second || early.at.Sub(f.sk.created) >= 179*time.Second {
		t.Fatal("early actual receive outside valid-key window")
	}
	p4RekeyWait(t, f.sk.created.Add(179*time.Second), f.sp.timers.sendKeepalive.IsPending, "responder receive keepalive arm missing")
	p4ResidueInput(t, f.server.tun, p4RekeyPacket(f.nonce, 5, true))
	p4RekeyReceive(t, f.client.tun, p4RekeyPacket(f.nonce, 5, true), f.sk.created.Add(179*time.Second))
	p4ResidueRetained(t, f.sp, f.sk)
	p4ResidueAt(t, f.sk.created, 181*time.Second, 183*time.Second)
	if _, used := f.server.b.consumed(held[1].wire); used {
		t.Fatal("held late counter was already received")
	}
	if f.server.b.deliver(held[1].packet) != nil {
		t.Fatal("late original protected delivery refused")
	}
	p4RekeyWait(t, f.sk.created.Add(184*time.Second), func() bool { _, ok := f.server.b.consumed(held[1].wire); return ok }, "actual ReceiveFunc consumption/next-entry missing")
	late, _ := f.server.b.consumed(held[1].wire)
	if late.at.Sub(f.sk.created) < 181*time.Second || late.at.Sub(f.sk.created) >= 183*time.Second {
		t.Fatal("late actual receive outside expired-key window")
	}
	quietStart := time.Now()
	timer := time.NewTimer(1500 * time.Millisecond)
	select {
	case <-f.server.tun.output:
		timer.Stop()
		t.Fatal("expired original packet reached fake TUN")
	case <-timer.C:
	}
	quiet := time.Since(quietStart)
	p4ResidueRetained(t, f.sp, f.sk)
	cz, sz, cr, sr, teardown := p4ResidueFinish(t, f, 0)
	t.Logf("p4_residue_receipt case=reject early_age_ns=%d late_age_ns=%d quiet_ns=%d client_zero_age_ns=%d server_zero_age_ns=%d client_removed_age_ns=%d server_removed_age_ns=%d teardown_ns=%d body_ns=%d baseline=true original_packet=true next_receive_entry=true expired_no_tun=true keys_retained_539=true previous_next_empty=true removal=true teardown_complete=true defaults_unchanged=true network_fds=false", early.at.Sub(f.sk.created), late.at.Sub(f.sk.created), quiet, cz, sz, cr, sr, teardown, time.Since(f.started))
}

func TestP4DefaultElapsedExhaustionKeepsResidue(t *testing.T) {
	f := p4ResidueBaseline(t)
	f.client.b.dropH1.Store(true)
	f.server.b.dropH1.Store(true)
	p4ResidueAt(t, f.ck.created, 181*time.Second, 183*time.Second)
	// ONE genuine packet: another would reset attempts before the min-time gate.
	p4ResidueInput(t, f.client.tun, p4RekeyPacket(f.nonce, 6, false))
	chainEnd := f.ck.created.Add(315 * time.Second)
	p4RekeyWait(t, f.ck.created.Add(184*time.Second), func() bool {
		return p4RekeyHandshakeCount(&f.client.b.p4RekeyBind, MessageInitiationType) == 2 && len(f.cp.queue.staged) == 1
	}, "actual expired-key outbound staging/H1 missing")
	var chain []p4RekeyWire
	var targets []time.Duration
	var first time.Time
	var retryMin, retryMax time.Duration
	for n := 1; n <= 20; n++ {
		end := chainEnd
		if n > 1 {
			end = chain[n-2].at.Add(8 * time.Second)
		}
		p4RekeyWait(t, end, func() bool { return p4RekeyHandshakeCount(&f.client.b.p4RekeyBind, MessageInitiationType) == n+1 }, "default retry emission outside finite window")
		chain = nil
		for _, wire := range f.client.b.snapshot() {
			if wire.kind == MessageInitiationType && wire.at.After(f.ck.created) {
				chain = append(chain, wire)
			}
		}
		if len(chain) != n || len(f.cp.queue.staged) != 1 {
			t.Fatal("single chain/staged packet preservation differs")
		}
		current := chain[n-1]
		if n == 1 {
			first = current.at
		} else if current.index == chain[n-2].index || current.at.Sub(chain[n-2].at) < 5*time.Second || current.at.Sub(chain[n-2].at) > 8*time.Second {
			t.Fatal("genuine default fresh retry index/elapsed differs")
		}
		if n > 1 {
			elapsed := current.at.Sub(chain[n-2].at)
			if retryMin == 0 || elapsed < retryMin {
				retryMin = elapsed
			}
			if elapsed > retryMax {
				retryMax = elapsed
			}
		}
		p4RekeyWait(t, current.at.Add(time.Second), f.cp.timers.retransmitHandshake.IsPending, "actual retry arm missing")
		targets = append(targets, p4ResidueScheduled(t, f.cp.timers.retransmitHandshake, 5*time.Second, 5333*time.Millisecond))
	}
	triggerAge := first.Sub(f.ck.created)
	if triggerAge < 181*time.Second || triggerAge >= 184*time.Second {
		t.Fatal("actual expired-key H1 trigger window differs")
	}
	targetMin, targetMax := targets[0], targets[0]
	for _, duration := range targets {
		if duration < targetMin {
			targetMin = duration
		}
		if duration > targetMax {
			targetMax = duration
		}
	}
	p4RekeyWait(t, first.Add(130*time.Second), func() bool { return len(f.client.log.events("exhausted")) == 1 && len(f.cp.queue.staged) == 0 }, "actual exhaustion and staged drain missing")
	exhaustion := f.client.log.events("exhausted")[0].Sub(first)
	if exhaustion < 100*time.Second || exhaustion > 130*time.Second || len(f.client.log.events("retry")) != 19 || f.cp.timers.handshakeAttempts.Load() != 19 || f.cp.timers.retransmitHandshake.IsPending() || f.cp.timers.sendKeepalive.IsPending() || len(targets) != 20 {
		t.Fatal("default20-expiration/19-retry exhaustion differs")
	}
	p4ResidueScheduled(t, f.cp.timers.zeroKeyMaterial, 540*time.Second, 540*time.Second)
	p4ResidueRetained(t, f.cp, f.ck)
	f.cp.handshake.mutex.RLock()
	partial := f.cp.handshake.localIndex
	f.cp.handshake.mutex.RUnlock()
	if partial != chain[19].index || f.client.d.indexTable.Lookup(partial).peer != f.cp {
		t.Fatal("latest genuine partial handshake index missing")
	}
	quietStart := time.Now()
	p4ResidueAt(t, quietStart, 5500*time.Millisecond, 6*time.Second)
	quiet := time.Since(quietStart)
	if p4RekeyHandshakeCount(&f.client.b.p4RekeyBind, MessageInitiationType) != 21 || p4RekeyHandshakeCount(&f.server.b.p4RekeyBind, MessageInitiationType) != 0 || p4RekeyHandshakeCount(&f.server.b.p4RekeyBind, MessageResponseType) != 1 || len(f.cp.queue.staged) != 0 || len(f.client.log.events("exhausted")) != 1 {
		t.Fatal("exhausted chain continued or staged drain changed")
	}
	cz, sz, cr, sr, teardown := p4ResidueFinish(t, f, partial)
	t.Logf("p4_residue_receipt case=exhaust trigger_age_ns=%d exhaustion_ns=%d retry_min_ns=%d retry_max_ns=%d target_min_ns=%d target_max_ns=%d quiet_ns=%d client_zero_age_ns=%d server_zero_age_ns=%d client_removed_age_ns=%d server_removed_age_ns=%d teardown_ns=%d body_ns=%d chain_h1=20 retries=19 expirations=20 attempts=19 baseline=true genuine_staged=true real_timer_chain=true partial_retained=true prearmed_preserved=true keys_retained_539=true previous_next_empty=true removal=true teardown_complete=true defaults_unchanged=true network_fds=false", triggerAge, exhaustion, retryMin, retryMax, targetMin, targetMax, quiet, cz, sz, cr, sr, teardown, time.Since(f.started))
}
