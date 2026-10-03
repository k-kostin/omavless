//go:build p4_cookie_overlay

// SPDX-License-Identifier: MIT
// CPU source-engine observation only: no sockets, TUN FD or timer shortening.
package device

import (
	"bytes"
	"crypto/rand"
	"encoding/binary"
	"encoding/hex"
	"fmt"
	"os"
	"testing"
	"time"

	"github.com/amnezia-vpn/amneziawg-go/v3/conn"
	"github.com/amnezia-vpn/amneziawg-go/v3/tun"
)

type p4ElapsedEmission struct {
	packet []byte
	at     time.Time
}

type p4ElapsedBind struct {
	p4Bind
	emitted chan p4ElapsedEmission
}

func (b *p4ElapsedBind) Send(packets [][]byte, _ conn.Endpoint) error {
	for _, packet := range packets {
		// Timestamp at the actual engine's Send boundary, not observer wakeup.
		emission := p4ElapsedEmission{bytes.Clone(packet), time.Now()}
		select {
		case b.emitted <- emission:
		default:
			return os.ErrInvalid
		}
	}
	return nil
}

func p4ElapsedDevice(t *testing.T, headerKey []byte, refused chan struct{}) (*Device, *p4ElapsedBind, NoisePrivateKey) {
	t.Helper()
	b := &p4ElapsedBind{emitted: make(chan p4ElapsedEmission, 8)}
	logger := NewLogger(LogLevelSilent, "")
	logger.Verbosef = func(format string, _ ...any) {
		if format == "Received invalid response message from %s" && refused != nil {
			// Only a fixed existing worker-stage event; never record its arguments.
			select {
			case refused <- struct{}{}:
			default:
			}
		}
	}
	d := NewDevice(&p4Tun{closed: make(chan struct{}), events: make(chan tun.Event)}, b, logger)
	t.Cleanup(func() { d.Close(); <-d.Wait() })
	sk, err := newPrivateKey()
	if err != nil || d.SetPrivateKey(sk) != nil {
		t.Fatal("synthetic identity setup refused")
	}
	config := fmt.Sprintf("s1=24\ns2=32\ns3=40\ns4=48\nh1=101\nh2=202\nh3=303\nh4=404\nheader_protection_key=%s\nrandom_trailers=false\ndisable_cookies=false\n", hex.EncodeToString(headerKey))
	if d.IpcSet(config) != nil {
		t.Fatal("synthetic shape setup refused")
	}
	return d, b, sk
}

func p4ElapsedCapture(t *testing.T, b *p4ElapsedBind, deadline time.Time) p4ElapsedEmission {
	t.Helper()
	remaining := time.Until(deadline)
	if remaining <= 0 {
		t.Fatal("elapsed case deadline")
	}
	timer := time.NewTimer(remaining)
	defer timer.Stop()
	select {
	case emission := <-b.emitted:
		return emission
	case <-timer.C:
		t.Fatal("elapsed engine emission deadline")
		return p4ElapsedEmission{}
	}
}

func p4ElapsedInject(t *testing.T, d *Device, packet []byte, kind uint32, deadline time.Time) {
	t.Helper()
	buffer := d.GetMessageBuffer()
	copy(buffer[:], packet)
	element := QueueHandshakeElement{msgType: kind, packet: buffer[:len(packet)], endpoint: p4Endpoint{}, buffer: buffer}
	timer := time.NewTimer(max(0, time.Until(deadline)))
	defer timer.Stop()
	select {
	case d.queue.handshake.c <- element:
	case <-timer.C:
		d.PutMessageBuffer(buffer)
		t.Fatal("actual worker admission deadline")
	}
}

func p4ElapsedSchedule(t *testing.T, p *Peer) time.Duration {
	t.Helper()
	p.timers.retransmitHandshake.modifyingLock.RLock()
	duration := p.timers.retransmitHandshake.duration
	p.timers.retransmitHandshake.modifyingLock.RUnlock()
	if duration < 5*time.Second || duration >= 5*time.Second+334*time.Millisecond {
		t.Fatal("unchanged default retry scheduling target differs")
	}
	return duration
}

func TestP4DefaultElapsedRetryAndCancellation(t *testing.T) {
	started := time.Now()
	deadline := started.Add(18 * time.Second)
	var headerKey [32]byte
	if _, err := rand.Read(headerKey[:]); err != nil {
		t.Fatal("synthetic header generation refused")
	}
	refused := make(chan struct{}, 1)
	client, cb, clientSK := p4ElapsedDevice(t, headerKey[:], refused)
	server, sb, serverSK := p4ElapsedDevice(t, headerKey[:], nil)
	cp, err := client.NewPeer(serverSK.publicKey())
	if err != nil {
		t.Fatal("client peer refused")
	}
	sp, err := server.NewPeer(clientSK.publicKey())
	if err != nil {
		t.Fatal("server peer refused")
	}
	cp.endpoint.val = p4Endpoint{}
	if client.Up() != nil || server.Up() != nil || !cp.timersActive() || !sp.timersActive() {
		t.Fatal("in-memory peers not active")
	}
	if cp.retransmitHandshakeTimeout() != 5*time.Second || client.rekeyMinTimeout() != 5*time.Second || client.keyRefreshTimeoutSending() != 120*time.Second {
		t.Fatal("product defaults overridden")
	}
	if cp.SendHandshakeInitiation(false) != nil {
		t.Fatal("initial real initiation refused")
	}
	first := p4ElapsedCapture(t, cb, started.Add(time.Second))
	firstPlain := p4Decode(t, server, first.packet, MessageInitiationType, 24, 101)
	if !server.cookieChecker.CheckMAC1(firstPlain) {
		t.Fatal("initial real MAC1 refused")
	}
	firstIndex := binary.LittleEndian.Uint32(firstPlain[4:8])
	target := p4ElapsedSchedule(t, cp)
	// Deliberately withhold the first request. The real unmodified Timer—not a
	// direct callback, aged key or shortened delay—must cause the next send.
	retry := p4ElapsedCapture(t, cb, first.at.Add(8*time.Second))
	observed := retry.at.Sub(first.at)
	if observed < 5*time.Second || observed > 8*time.Second {
		t.Fatal("default elapsed emission outside observation bounds")
	}
	retryPlain := p4Decode(t, server, retry.packet, MessageInitiationType, 24, 101)
	if !server.cookieChecker.CheckMAC1(retryPlain) || binary.LittleEndian.Uint32(retryPlain[4:8]) == firstIndex || cp.timers.handshakeAttempts.Load() != 1 {
		t.Fatal("fresh authenticated retry/attempt receipt missing")
	}
	// Send returns before its caller arms the next retry. Observe that actual
	// arm with a bounded wait; never modify it to help the test.
	for !cp.timers.retransmitHandshake.IsPending() && time.Now().Before(retry.at.Add(time.Second)) {
		time.Sleep(time.Millisecond)
	}
	p4ElapsedSchedule(t, cp)
	p4ElapsedInject(t, server, retryPlain, MessageInitiationType, retry.at.Add(time.Second))
	response := p4ElapsedCapture(t, sb, retry.at.Add(2*time.Second))
	responsePlain := p4Decode(t, client, response.packet, MessageResponseType, 32, 202)
	var malformed MessageResponse
	if binary.Read(bytes.NewReader(responsePlain), binary.LittleEndian, &malformed) != nil {
		t.Fatal("real response decode refused")
	}
	malformed.Empty[0] ^= 1
	var encoded bytes.Buffer
	if binary.Write(&encoded, binary.LittleEndian, malformed) != nil {
		t.Fatal("full-size negative encoding refused")
	}
	bad := encoded.Bytes()
	sp.cookieGenerator.AddMacs(bad)
	if len(bad) != MessageResponseSize || !client.cookieChecker.CheckMAC1(bad) {
		t.Fatal("negative must have actual full size and valid outer MAC1")
	}
	p4ElapsedInject(t, client, bad, MessageResponseType, retry.at.Add(2*time.Second))
	select {
	case <-refused:
	case <-time.After(max(0, time.Until(retry.at.Add(2*time.Second)))):
		t.Fatal("actual worker Noise-refusal event missing")
	}
	if cp.keypairs.Current() != nil || !cp.timers.retransmitHandshake.IsPending() || cp.timers.handshakeAttempts.Load() != 1 || cp.lastHandshakeNano.Load() != 0 {
		t.Fatal("malformed authenticated response derived/cancelled session")
	}
	p4ElapsedInject(t, client, responsePlain, MessageResponseType, retry.at.Add(2*time.Second))
	// The actual response worker derives keys and calls handshake-complete.
	for time.Now().Before(retry.at.Add(3 * time.Second)) {
		if cp.keypairs.Current() != nil && cp.lastHandshakeNano.Load() > 0 && cp.timers.handshakeAttempts.Load() == 0 && !cp.timers.retransmitHandshake.IsPending() {
			break
		}
		time.Sleep(time.Millisecond)
	}
	kp := cp.keypairs.Current()
	if kp == nil || client.indexTable.Lookup(kp.localIndex).keypair != kp || cp.lastHandshakeNano.Load() <= 0 || cp.timers.handshakeAttempts.Load() != 0 || cp.timers.retransmitHandshake.IsPending() {
		t.Fatal("actual worker session/cancellation receipt missing")
	}
	completed := time.Now()
	quietUntil := completed.Add(5500 * time.Millisecond)
	if quietUntil.After(deadline) {
		t.Fatal("remaining default cancellation window exceeds case deadline")
	}
	keepalives := 0
	timer := time.NewTimer(time.Until(quietUntil))
	defer timer.Stop()
quiet:
	for {
		select {
		case emission := <-cb.emitted:
			// The response worker's H4 keepalive is not a retry H1.
			p4Decode(t, server, emission.packet, MessageTransportType, 48, 404)
			keepalives++
		case <-timer.C:
			break quiet
		}
	}
	if keepalives != 1 || cp.timers.retransmitHandshake.IsPending() || cp.timers.handshakeAttempts.Load() != 0 || time.Now().After(deadline) {
		t.Fatal("default cancellation window receipt differs")
	}
	// Fixed numeric observations only: scheduled target is distinct from actual
	// elapsed send observations, which include scheduler/engine work delays.
	t.Logf("p4_elapsed_receipt retry_target_ns=%d observed_retry_ns=%d quiet_window_ns=%d malformed_worker_refused=true actual_session=true h1_initial=1 h1_retry=1 h1_after=0 h4_after=%d defaults_unchanged=true", target, observed, time.Since(completed), keepalives)
}
