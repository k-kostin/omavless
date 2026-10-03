//go:build p4_cookie_overlay

// SPDX-License-Identifier: MIT
// Full pinned-engine pipeline over channels only: no network/TUN FD, timer
// callback replacement, artificial key age or timing-option override.
package device

import (
	"bytes"
	"crypto/rand"
	"encoding/binary"
	"encoding/hex"
	"fmt"
	"net"
	"net/netip"
	"os"
	"sync"
	"testing"
	"time"

	"github.com/amnezia-vpn/amneziawg-go/v3/conn"
	"github.com/amnezia-vpn/amneziawg-go/v3/tun"
)

type p4RekeyEndpoint struct{}

func (p4RekeyEndpoint) ClearSrc()           {}
func (p4RekeyEndpoint) SrcToString() string { return "192.0.2.2:1" }
func (p4RekeyEndpoint) DstToString() string { return "192.0.2.1:1" }
func (p4RekeyEndpoint) DstToBytes() []byte  { return []byte{192, 0, 2, 1, 0, 1} }
func (p4RekeyEndpoint) DstIP() netip.Addr   { return netip.MustParseAddr("192.0.2.1") }
func (p4RekeyEndpoint) SrcIP() netip.Addr   { return netip.MustParseAddr("192.0.2.2") }

type p4RekeyWire struct {
	at        time.Time
	kind      uint32
	index     uint32
	counter   uint64
	protected bool
}

// Not *conn.StdNetBind: the pinned Linux route listener returns before opening
// a netlink socket. ReceiveFuncs deliver unchanged encrypted bytes to the
// actual RoutineReceiveIncoming, rather than manually populating worker queues.
type p4RekeyBind struct {
	sync.Mutex
	device   *Device
	remote   *p4RekeyBind
	incoming chan []byte
	closed   chan struct{}
	open     bool
	wires    []p4RekeyWire
}

func (b *p4RekeyBind) Open(uint16) ([]conn.ReceiveFunc, uint16, error) {
	b.Lock()
	defer b.Unlock()
	if b.open {
		return nil, 0, conn.ErrBindAlreadyOpen
	}
	b.open = true
	b.closed = make(chan struct{})
	closed := b.closed
	return []conn.ReceiveFunc{func(packets [][]byte, sizes []int, endpoints []conn.Endpoint) (int, error) {
		select {
		case <-closed:
			return 0, net.ErrClosed
		case packet := <-b.incoming:
			if len(packets) != 1 || len(sizes) != 1 || len(endpoints) != 1 || len(packet) > len(packets[0]) {
				return 0, os.ErrInvalid
			}
			copy(packets[0], packet)
			sizes[0] = len(packet)
			endpoints[0] = p4RekeyEndpoint{}
			return 1, nil
		}
	}}, 1, nil
}
func (b *p4RekeyBind) Close() error {
	b.Lock()
	defer b.Unlock()
	if b.open {
		b.open = false
		close(b.closed)
	}
	return nil
}
func (*p4RekeyBind) SetMark(uint32) error                        { return nil }
func (*p4RekeyBind) BatchSize() int                              { return 1 }
func (*p4RekeyBind) ParseEndpoint(string) (conn.Endpoint, error) { return p4RekeyEndpoint{}, nil }
func (b *p4RekeyBind) Send(packets [][]byte, _ conn.Endpoint) error {
	for _, packet := range packets {
		if len(packet) < HeaderCipherNonceSize {
			return os.ErrInvalid
		}
		cipher, err := b.device.HeaderProtectionCipher(packet[:HeaderCipherNonceSize])
		if err != nil || cipher == nil {
			return os.ErrInvalid
		}
		var typeHash [4]byte
		cipher.XORKeyStream(typeHash[:], typeHash[:])
		_, kind, padding := b.device.DeterminePacketTypeAndPadding(packet, typeHash[:])
		if kind != MessageInitiationType && kind != MessageResponseType && kind != MessageTransportType {
			return os.ErrInvalid
		}
		header := bytes.Clone(packet[padding : int(padding)+MessageTransportHeaderSize])
		cipher, _ = b.device.HeaderProtectionCipher(packet[:HeaderCipherNonceSize])
		cipher.XORKeyStream(header, header)
		wire := p4RekeyWire{at: time.Now(), kind: kind, index: binary.LittleEndian.Uint32(header[4:8]), protected: !bytes.Equal(header[:4], packet[padding:padding+4])}
		if kind == MessageTransportType {
			wire.counter = binary.LittleEndian.Uint64(header[8:16])
		}
		b.Lock()
		if len(b.wires) >= 128 {
			b.Unlock()
			return os.ErrInvalid
		}
		b.wires = append(b.wires, wire)
		b.Unlock()
		b.remote.Lock()
		open, closed := b.remote.open, b.remote.closed
		b.remote.Unlock()
		if !open {
			return net.ErrClosed
		}
		select {
		case b.remote.incoming <- bytes.Clone(packet):
		case <-closed:
			return net.ErrClosed
		default:
			return os.ErrInvalid
		}
	}
	return nil
}
func (b *p4RekeyBind) snapshot() []p4RekeyWire {
	b.Lock()
	defer b.Unlock()
	return append([]p4RekeyWire(nil), b.wires...)
}

type p4RekeyPlain struct {
	packet  []byte
	index   uint32
	counter uint64
}
type p4RekeyTun struct {
	input  chan []byte
	output chan p4RekeyPlain
	closed chan struct{}
	events chan tun.Event
	once   sync.Once
}

func (*p4RekeyTun) File() *os.File             { return nil }
func (*p4RekeyTun) MTU() (int, error)          { return 1420, nil }
func (*p4RekeyTun) Name() (string, error)      { return "p4-channel", nil }
func (*p4RekeyTun) BatchSize() int             { return 1 }
func (m *p4RekeyTun) Events() <-chan tun.Event { return m.events }
func (m *p4RekeyTun) Read(packets [][]byte, sizes []int, offset int) (int, error) {
	select {
	case <-m.closed:
		return 0, os.ErrClosed
	case packet := <-m.input:
		if len(packets) != 1 || len(sizes) != 1 || offset < 0 || offset+len(packet) > len(packets[0]) {
			return 0, os.ErrInvalid
		}
		copy(packets[0][offset:], packet)
		sizes[0] = len(packet)
		return 1, nil
	}
}
func (m *p4RekeyTun) Write(packets [][]byte, offset int) (int, error) {
	for _, packet := range packets {
		if offset != MessageTransportOffsetContent || len(packet) < offset+20 {
			return 0, os.ErrInvalid
		}
		// These are the actual decrypted transport header+IP buffers supplied
		// by the sequential receiver after successful AEAD and replay checks.
		plain := p4RekeyPlain{bytes.Clone(packet[offset:]), binary.LittleEndian.Uint32(packet[4:8]), binary.LittleEndian.Uint64(packet[8:16])}
		select {
		case m.output <- plain:
		default:
			return 0, os.ErrInvalid
		}
	}
	return len(packets), nil
}
func (m *p4RekeyTun) Close() error {
	m.once.Do(func() { close(m.closed); close(m.events) })
	return nil
}

func p4RekeyDevice(t *testing.T, key []byte) (*Device, *p4RekeyBind, *p4RekeyTun, NoisePrivateKey) {
	t.Helper()
	b := &p4RekeyBind{incoming: make(chan []byte, 64)}
	m := &p4RekeyTun{input: make(chan []byte, 8), output: make(chan p4RekeyPlain, 8), closed: make(chan struct{}), events: make(chan tun.Event)}
	d := NewDevice(m, b, NewLogger(LogLevelSilent, ""))
	b.device = d
	t.Cleanup(func() { d.Close(); <-d.Wait() })
	sk, err := newPrivateKey()
	if err != nil || d.SetPrivateKey(sk) != nil {
		t.Fatal("synthetic private identity refused")
	}
	config := fmt.Sprintf("s1=24\ns2=32\ns3=40\ns4=48\nh1=101\nh2=202\nh3=303\nh4=404\nheader_protection_key=%s\nrandom_trailers=false\ndisable_cookies=false\n", hex.EncodeToString(key))
	if d.IpcSet(config) != nil {
		t.Fatal("synthetic protected shape refused")
	}
	return d, b, m, sk
}

func p4RekeyWait(t *testing.T, deadline time.Time, predicate func() bool, label string) {
	t.Helper()
	for !predicate() {
		if !time.Now().Before(deadline) {
			t.Fatal(label)
		}
		time.Sleep(time.Millisecond)
	}
}
func p4RekeyReceive(t *testing.T, m *p4RekeyTun, packet []byte, deadline time.Time) p4RekeyPlain {
	t.Helper()
	timer := time.NewTimer(max(0, time.Until(deadline)))
	defer timer.Stop()
	select {
	case receipt := <-m.output:
		if !bytes.Equal(receipt.packet, packet) {
			t.Fatal("exact nonce-bearing decrypted IP bytes differ")
		}
		return receipt
	case <-timer.C:
		t.Fatal("decrypted fake TUN receipt deadline")
		return p4RekeyPlain{}
	}
}
func p4RekeyPacket(nonce [16]byte, phase byte, reverse bool) []byte {
	packet := make([]byte, 64)
	packet[0], packet[8], packet[9] = 0x45, 64, 17
	binary.BigEndian.PutUint16(packet[2:4], uint16(len(packet)))
	binary.BigEndian.PutUint16(packet[4:6], uint16(phase))
	copy(packet[12:16], []byte{198, 18, 0, 1})
	copy(packet[16:20], []byte{198, 18, 0, 2})
	if reverse {
		packet[15], packet[19] = 2, 1
	}
	binary.BigEndian.PutUint16(packet[20:22], 39001)
	binary.BigEndian.PutUint16(packet[22:24], 39002)
	binary.BigEndian.PutUint16(packet[24:26], uint16(len(packet)-20))
	copy(packet[28:44], nonce[:])
	packet[44] = phase
	if reverse {
		packet[45] = 1
	}
	var sum uint32
	for i := 0; i < 20; i += 2 {
		sum += uint32(binary.BigEndian.Uint16(packet[i : i+2]))
	}
	for sum>>16 != 0 {
		sum = sum&0xffff + sum>>16
	}
	binary.BigEndian.PutUint16(packet[10:12], ^uint16(sum))
	return packet
}
func p4RekeyMatch(t *testing.T, b *p4RekeyBind, plain p4RekeyPlain) p4RekeyWire {
	t.Helper()
	var matches []p4RekeyWire
	for _, wire := range b.snapshot() {
		if wire.kind == MessageTransportType && wire.index == plain.index && wire.counter == plain.counter {
			matches = append(matches, wire)
		}
	}
	if len(matches) != 1 || !matches[0].protected {
		t.Fatal("exact authenticated receive lacks unique protected send index/counter")
	}
	return matches[0]
}
func p4RekeyHandshakeCount(b *p4RekeyBind, kind uint32) int {
	count := 0
	for _, wire := range b.snapshot() {
		if wire.kind == kind {
			count++
		}
	}
	return count
}

func TestP4DefaultElapsedRekey(t *testing.T) {
	started := time.Now()
	deadline := started.Add(150 * time.Second)
	var headerKey, nonce [16]byte
	if _, err := rand.Read(headerKey[:]); err != nil {
		t.Fatal("synthetic header key failed")
	}
	if _, err := rand.Read(nonce[:]); err != nil {
		t.Fatal("synthetic nonce failed")
	}
	// HeaderProtectionCipher requires its native 32-byte key; no key material
	// is ever persisted or emitted by the silent fixture.
	var fullKey [32]byte
	copy(fullKey[:16], headerKey[:])
	if _, err := rand.Read(fullKey[16:]); err != nil {
		t.Fatal("synthetic header key failed")
	}
	client, cb, ct, clientSK := p4RekeyDevice(t, fullKey[:])
	server, sb, st, serverSK := p4RekeyDevice(t, fullKey[:])
	cb.remote, sb.remote = sb, cb
	cp, err := client.NewPeer(serverSK.publicKey())
	if err != nil {
		t.Fatal("client peer refused")
	}
	sp, err := server.NewPeer(clientSK.publicKey())
	if err != nil {
		t.Fatal("server peer refused")
	}
	cp.endpoint.val, sp.endpoint.val = p4RekeyEndpoint{}, p4RekeyEndpoint{}
	client.allowedips.Insert(netip.MustParsePrefix("198.18.0.2/32"), cp)
	server.allowedips.Insert(netip.MustParsePrefix("198.18.0.1/32"), sp)
	if server.Up() != nil || client.Up() != nil || client.keyRefreshTimeoutSending() != 120*time.Second || client.keyRefreshTimeoutReceiving() != 165*time.Second || client.keychainExpireTime() != 180*time.Second || cp.newHandshakeTimeout() != 15*time.Second || cp.sendKeepaliveTimeout() != 10*time.Second || cp.retransmitHandshakeTimeout() != 5*time.Second {
		t.Fatal("actual channel engine/defaults refused")
	}
	roundTrip := func(phase byte, end time.Time, waitSendArm bool) (p4RekeyWire, p4RekeyWire) {
		request, reply := p4RekeyPacket(nonce, phase, false), p4RekeyPacket(nonce, phase, true)
		select {
		case ct.input <- request:
		default:
			t.Fatal("bounded TUN admission refused")
		}
		forward := p4RekeyReceive(t, st, request, end)
		if waitSendArm {
			// Observe the actual send worker's data timer arm before returning
			// an authenticated echo; otherwise ultra-fast channel delivery could
			// race the arm and leave an idle-loss timer behind.
			p4RekeyWait(t, end, cp.timers.newHandshake.IsPending, "actual data-sent timer arm missing")
		}
		select {
		case st.input <- reply:
		default:
			t.Fatal("bounded reverse TUN admission refused")
		}
		reverse := p4RekeyReceive(t, ct, reply, end)
		if waitSendArm && cp.timers.newHandshake.IsPending() {
			t.Fatal("actual authenticated echo did not cancel idle-loss timer")
		}
		return p4RekeyMatch(t, cb, forward), p4RekeyMatch(t, sb, reverse)
	}
	baselineForward, baselineReverse := roundTrip(1, started.Add(5*time.Second), true)
	old, oldServer := cp.keypairs.Current(), sp.keypairs.Current()
	if old == nil || oldServer == nil || !old.isInitiator || oldServer.isInitiator || old.remoteIndex != oldServer.localIndex || old.localIndex != oldServer.remoteIndex || client.indexTable.Lookup(old.localIndex).keypair != old || server.indexTable.Lookup(oldServer.localIndex).keypair != oldServer || baselineForward.index != oldServer.localIndex || baselineReverse.index != old.localIndex {
		t.Fatal("actual baseline indexed sessions missing")
	}
	oldHandshake := cp.lastHandshakeNano.Load()
	if oldHandshake <= 0 || p4RekeyHandshakeCount(cb, MessageInitiationType) != 1 || p4RekeyHandshakeCount(sb, MessageResponseType) != 1 {
		t.Fatal("baseline real Noise chain differs")
	}
	preAt := old.created.Add(117 * time.Second)
	if time.Until(preAt) <= 0 {
		t.Fatal("baseline unexpectedly exceeded pre-threshold target")
	}
	<-time.After(time.Until(preAt))
	pre, reversePre := roundTrip(2, old.created.Add(119*time.Second), true)
	preAge := pre.at.Sub(old.created)
	if preAge < 117*time.Second || preAge >= 120*time.Second || pre.index != oldServer.localIndex || reversePre.index != old.localIndex || cp.keypairs.Current() != old || cp.lastHandshakeNano.Load() != oldHandshake || p4RekeyHandshakeCount(cb, MessageInitiationType) != 1 || p4RekeyHandshakeCount(sb, MessageInitiationType) != 0 || cp.timers.newHandshake.IsPending() {
		t.Fatal("pre-threshold causal negative/echo differs")
	}
	<-time.After(max(0, time.Until(old.created.Add(121*time.Second))))
	if cp.keypairs.Current() != old || cp.timers.newHandshake.IsPending() || p4RekeyHandshakeCount(cb, MessageInitiationType) != 1 || old.sendNonce.Load() >= RekeyAfterMessages {
		t.Fatal("pre-trigger key/idle/nonce confound")
	}
	late, _ := roundTrip(3, old.created.Add(125*time.Second), false)
	lateAge := late.at.Sub(old.created)
	if lateAge < 120*time.Second || late.index != oldServer.localIndex {
		t.Fatal("late actual data did not use old session after default age")
	}
	p4RekeyWait(t, old.created.Add(128*time.Second), func() bool {
		return cp.keypairs.Current() != nil && cp.keypairs.Current() != old && sp.keypairs.Current() != nil && sp.keypairs.Current() != oldServer
	}, "genuine worker rekey/confirmation missing")
	current, serverCurrent := cp.keypairs.Current(), sp.keypairs.Current()
	var rekeys []p4RekeyWire
	for _, wire := range cb.snapshot() {
		if wire.kind == MessageInitiationType && wire.at.After(baselineForward.at) {
			rekeys = append(rekeys, wire)
		}
	}
	if len(rekeys) != 1 || !rekeys[0].protected || rekeys[0].at.Before(late.at) || rekeys[0].at.Sub(old.created) < 120*time.Second || current.localIndex == old.localIndex || current.created.Before(rekeys[0].at) || cp.lastHandshakeNano.Load() <= oldHandshake || current.remoteIndex != serverCurrent.localIndex || current.localIndex != serverCurrent.remoteIndex || client.indexTable.Lookup(current.localIndex).keypair != current || server.indexTable.Lookup(serverCurrent.localIndex).keypair != serverCurrent || p4RekeyHandshakeCount(cb, MessageInitiationType) != 2 || p4RekeyHandshakeCount(sb, MessageResponseType) != 2 {
		t.Fatal("actual elapsed new Noise epoch/index receipt differs")
	}
	post, reversePost := roundTrip(4, old.created.Add(131*time.Second), true)
	if post.index != serverCurrent.localIndex || reversePost.index != current.localIndex || cp.keypairs.Current() != current || sp.keypairs.Current() != serverCurrent || !current.isInitiator || serverCurrent.isInitiator || serverCurrent.localIndex == oldServer.localIndex || p4RekeyHandshakeCount(cb, MessageInitiationType) != 2 || p4RekeyHandshakeCount(cb, MessageResponseType) != 0 || p4RekeyHandshakeCount(sb, MessageInitiationType) != 0 || p4RekeyHandshakeCount(sb, MessageResponseType) != 2 || time.Now().After(deadline) {
		t.Fatal("post-rekey exact protected payload/epoch deadline differs")
	}
	t.Logf("p4_rekey_receipt pre_age_ns=%d trigger_data_age_ns=%d rekey_h1_age_ns=%d observed_body_ns=%d baseline_bidir=true pre_no_rekey=true echo_cancels_idle=true real_receive_aead=true new_indexed_sessions=true post_bidir=true client_h1=2 server_h1=0 server_h2=2 defaults_unchanged=true network_fds=false", preAge, lateAge, rekeys[0].at.Sub(old.created), time.Since(started))
}
