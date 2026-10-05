//go:build p4_cookie_overlay && p4_default_residue_overlay

// SPDX-License-Identifier: MIT
// Add-only channel fixture. Reuses the exact pinned #589 packet/TUN support,
// never its unbounded cleanup helper. No timer/key-age/callback substitution.
package device

import (
	"bytes"
	"encoding/binary"
	"encoding/hex"
	"fmt"
	"net"
	"os"
	"runtime"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/amnezia-vpn/amneziawg-go/v3/conn"
)

const p4ResiduePacketCap = 4096
const p4ResidueEventCap = 128

type p4ResidueHeld struct {
	packet []byte // Silent memory only, never receipt/log material.
	wire   p4RekeyWire
}

type p4ResidueDelivery struct {
	entry uint32
	at    time.Time
	index uint32
	count uint64
}

type p4ResidueBind struct {
	p4RekeyBind
	other      *p4ResidueBind
	dropH1     atomic.Bool
	holdData   atomic.Bool
	entries    atomic.Uint32
	failed     atomic.Bool
	held       []p4ResidueHeld
	deliveries []p4ResidueDelivery
}

func (b *p4ResidueBind) Open(port uint16) ([]conn.ReceiveFunc, uint16, error) {
	functions, bound, err := b.p4RekeyBind.Open(port)
	if err != nil || len(functions) != 1 {
		return functions, bound, err
	}
	receive := functions[0]
	return []conn.ReceiveFunc{func(packets [][]byte, sizes []int, endpoints []conn.Endpoint) (int, error) {
		entry := b.entries.Add(1)
		if entry > p4ResidueEventCap {
			b.failed.Store(true)
			return 0, os.ErrInvalid
		}
		n, err := receive(packets, sizes, endpoints)
		if err == nil && n == 1 {
			plain, kind, err := p4ResidueHeader(b.device, packets[0][:sizes[0]])
			if err != nil {
				b.failed.Store(true)
				return 0, err
			}
			if kind == MessageTransportType {
				b.Lock()
				if len(b.deliveries) >= p4ResidueEventCap {
					b.Unlock()
					b.failed.Store(true)
					return 0, os.ErrInvalid
				}
				b.deliveries = append(b.deliveries, p4ResidueDelivery{entry, time.Now(), binary.LittleEndian.Uint32(plain[4:8]), binary.LittleEndian.Uint64(plain[8:16])})
				b.Unlock()
			}
		}
		return n, err
	}}, bound, nil
}

func p4ResidueHeader(d *Device, packet []byte) ([]byte, uint32, error) {
	if len(packet) < HeaderCipherNonceSize || len(packet) > p4ResiduePacketCap {
		return nil, 0, os.ErrInvalid
	}
	cipher, err := d.HeaderProtectionCipher(packet[:HeaderCipherNonceSize])
	if err != nil || cipher == nil {
		return nil, 0, os.ErrInvalid
	}
	var hash [4]byte
	cipher.XORKeyStream(hash[:], hash[:])
	size, kind, padding := d.DeterminePacketTypeAndPadding(packet, hash[:])
	// The classifier returns the MINIMUM transport size, not its variable
	// payload length. Preserve the complete original body for length checks.
	if kind == MessageTransportType {
		size = len(packet) - int(padding)
	}
	if size < MessageTransportHeaderSize || int(padding)+size > len(packet) ||
		(kind != MessageInitiationType && kind != MessageResponseType && kind != MessageTransportType) {
		return nil, 0, os.ErrInvalid
	}
	plain := bytes.Clone(packet[padding : int(padding)+size])
	cipher, err = d.HeaderProtectionCipher(packet[:HeaderCipherNonceSize])
	if err != nil || cipher == nil {
		return nil, 0, os.ErrInvalid
	}
	if kind == MessageTransportType {
		cipher.XORKeyStream(plain[:MessageTransportHeaderSize], plain[:MessageTransportHeaderSize])
	} else {
		cipher.XORKeyStream(plain, plain)
	}
	if bytes.Equal(plain[:4], packet[padding:padding+4]) {
		return nil, 0, os.ErrInvalid
	}
	return plain, kind, nil
}

func (b *p4ResidueBind) Send(packets [][]byte, _ conn.Endpoint) error {
	for _, packet := range packets {
		plain, kind, err := p4ResidueHeader(b.device, packet)
		if err != nil || b.other == nil {
			b.failed.Store(true)
			return os.ErrInvalid
		}
		wire := p4RekeyWire{at: time.Now(), kind: kind, index: binary.LittleEndian.Uint32(plain[4:8]), protected: true}
		if kind == MessageInitiationType && (!b.other.device.cookieChecker.CheckMAC1(plain) || len(plain) != MessageInitiationSize || binary.LittleEndian.Uint32(plain[:4]) != 101) {
			b.failed.Store(true)
			return os.ErrInvalid
		}
		if kind == MessageTransportType {
			wire.counter = binary.LittleEndian.Uint64(plain[8:16])
		}
		b.Lock()
		if len(b.wires) >= p4ResidueEventCap {
			b.Unlock()
			b.failed.Store(true)
			return os.ErrInvalid
		}
		b.wires = append(b.wires, wire)
		hold := kind == MessageTransportType && len(plain) == MessageTransportHeaderSize+64+16 && b.holdData.Load()
		if hold {
			if len(b.held) >= 2 {
				b.Unlock()
				b.failed.Store(true)
				return os.ErrInvalid
			}
			b.held = append(b.held, p4ResidueHeld{bytes.Clone(packet), wire})
		}
		b.Unlock()
		if hold || (kind == MessageInitiationType && b.dropH1.Load()) {
			continue
		}
		if err := b.other.deliver(packet); err != nil {
			b.failed.Store(true)
			return err
		}
	}
	return nil
}

func (b *p4ResidueBind) deliver(packet []byte) error {
	if len(packet) > p4ResiduePacketCap {
		return os.ErrInvalid
	}
	b.Lock()
	open, closed := b.open, b.closed
	b.Unlock()
	if !open {
		return net.ErrClosed
	}
	select {
	case b.incoming <- bytes.Clone(packet):
		return nil
	case <-closed:
		return net.ErrClosed
	default:
		return os.ErrInvalid
	}
}

func (b *p4ResidueBind) heldPair() []p4ResidueHeld {
	b.Lock()
	defer b.Unlock()
	return append([]p4ResidueHeld(nil), b.held...)
}

func (b *p4ResidueBind) consumed(wire p4RekeyWire) (p4ResidueDelivery, bool) {
	b.Lock()
	defer b.Unlock()
	var found p4ResidueDelivery
	count := 0
	for _, event := range b.deliveries {
		if event.index == wire.index && event.count == wire.counter {
			found = event
			count++
		}
	}
	return found, count == 1 && b.entries.Load() > found.entry
}

type p4ResidueLog struct {
	sync.Mutex
	zero, retries, exhausted []time.Time
	starts, stops            [8]uint32
	failed                   atomic.Bool
}

var p4ResidueWorkers = [8][2]string{
	{"Routine: encryption worker %d - started", "Routine: encryption worker %d - stopped"},
	{"Routine: decryption worker %d - started", "Routine: decryption worker %d - stopped"},
	{"Routine: handshake worker %d - started", "Routine: handshake worker %d - stopped"},
	{"Routine: TUN reader - started", "Routine: TUN reader - stopped"},
	{"Routine: event worker - started", "Routine: event worker - stopped"},
	{"Routine: receive incoming %s - started", "Routine: receive incoming %s - stopped"},
	{"%v - Routine: sequential sender - started", "%v - Routine: sequential sender - stopped"},
	{"%v - Routine: sequential receiver - started", "%v - Routine: sequential receiver - stopped"},
}

func (l *p4ResidueLog) observe(format string, _ ...any) {
	l.Lock()
	defer l.Unlock()
	// Never inspect/format arguments, notably peer identities and keys.
	var events *[]time.Time
	switch format {
	case "%s - Removing all keys, since we haven't received a new one in %d seconds":
		events = &l.zero
	case "%s - Handshake did not complete after %d seconds, retrying (try %d)":
		events = &l.retries
	case "%s - Handshake did not complete after %d attempts, giving up":
		events = &l.exhausted
	}
	if events != nil {
		if len(*events) >= p4ResidueEventCap {
			l.failed.Store(true)
			return
		}
		*events = append(*events, time.Now()) // Callback entry, NOT completion.
	}
	for family, literals := range p4ResidueWorkers {
		if format == literals[0] {
			l.starts[family]++
		}
		if format == literals[1] {
			l.stops[family]++
		}
		if l.starts[family] > 32 || l.stops[family] > 32 {
			l.failed.Store(true)
		}
	}
}

func (l *p4ResidueLog) events(kind string) []time.Time {
	l.Lock()
	defer l.Unlock()
	switch kind {
	case "zero":
		return append([]time.Time(nil), l.zero...)
	case "retry":
		return append([]time.Time(nil), l.retries...)
	case "exhausted":
		return append([]time.Time(nil), l.exhausted...)
	}
	return nil
}

type p4ResidueDevice struct {
	d       *Device
	b       *p4ResidueBind
	tun     *p4RekeyTun
	log     *p4ResidueLog
	sk      NoisePrivateKey
	closeMu sync.Mutex
	closed  bool
}

func p4ResidueNewDevice(t *testing.T, key []byte) *p4ResidueDevice {
	t.Helper()
	// NewDevice uses NumCPU, not GOMAXPROCS. Bound before spawning anything.
	if runtime.NumCPU() < 1 || runtime.NumCPU() > 32 {
		t.Fatal("finite engine worker capacity refused")
	}
	f := &p4ResidueDevice{b: &p4ResidueBind{p4RekeyBind: p4RekeyBind{incoming: make(chan []byte, 128)}}, tun: p4RekeyNewTun(), log: &p4ResidueLog{}}
	logger := NewLogger(LogLevelSilent, "")
	logger.Verbosef = f.log.observe
	f.d = NewDevice(f.tun, f.b, logger)
	f.b.device = f.d
	t.Cleanup(func() { f.close(t, time.Now().Add(5*time.Second)) })
	var err error
	f.sk, err = newPrivateKey()
	if err != nil || f.d.SetPrivateKey(f.sk) != nil {
		t.Fatal("synthetic identity refused")
	}
	config := fmt.Sprintf("s1=24\ns2=32\ns3=40\ns4=48\nh1=101\nh2=202\nh3=303\nh4=404\nheader_protection_key=%s\nrandom_trailers=false\ndisable_cookies=false\n", hex.EncodeToString(key))
	if f.d.IpcSet(config) != nil {
		t.Fatal("synthetic protected shape refused")
	}
	p4RekeyConfiguredRead(t, f.tun, time.Now().Add(time.Second))
	return f
}

func (f *p4ResidueDevice) close(t *testing.T, end time.Time) {
	t.Helper()
	f.closeMu.Lock()
	defer f.closeMu.Unlock()
	if f.closed {
		return
	}
	f.closed = true // Exactly one close attempt, including uncertain failure.
	done := make(chan struct{})
	go func() { f.d.Close(); close(done) }()
	timer := time.NewTimer(max(0, time.Until(end)))
	defer timer.Stop()
	select {
	case <-done:
	case <-timer.C:
		t.Fatal("bounded device Close did not return")
	}
	select {
	case <-f.d.Wait():
	default:
		t.Fatal("actual device Wait not closed after Close")
	}
	p4RekeyWait(t, end, func() bool {
		f.log.Lock()
		defer f.log.Unlock()
		return f.log.starts == f.log.stops
	}, "fixed worker stop boundaries incomplete")
	// Logger boundaries alone are not joins. Actual Close joins receive,
	// sequential and its registered stopping group, synchronizes timers and
	// drops queue references. Encryption/decryption stop literals are NOT
	// separately exposed joins; the outer supervisor owns process completion.
	if f.log.failed.Load() || f.b.failed.Load() {
		t.Fatal("bounded fixture event capacity or packet refusal")
	}
}
