//go:build p4_cookie_overlay

// SPDX-License-Identifier: MIT
// Test-only overlay into the exact official engine's device package. This is
// not linked into the peer executable or any application component.
package device

import (
	"bytes"
	"crypto/rand"
	"encoding/binary"
	"encoding/hex"
	"fmt"
	"net/netip"
	"os"
	"sync"
	"testing"
	"time"

	"github.com/amnezia-vpn/amneziawg-go/v3/conn"
	"github.com/amnezia-vpn/amneziawg-go/v3/tun"
)

// No socket, interface, OS network action, TUN FD or incoming receive function.
type p4Bind struct{ sent chan []byte }

func (*p4Bind) Open(uint16) ([]conn.ReceiveFunc, uint16, error) { return nil, 1, nil }
func (*p4Bind) Close() error                                    { return nil }
func (*p4Bind) SetMark(uint32) error                            { return nil }
func (*p4Bind) BatchSize() int                                  { return 1 }
func (*p4Bind) ParseEndpoint(string) (conn.Endpoint, error)     { return p4Endpoint{}, nil }
func (b *p4Bind) Send(bufs [][]byte, _ conn.Endpoint) error {
	for _, packet := range bufs {
		select {
		case b.sent <- bytes.Clone(packet):
		default:
			return os.ErrInvalid
		}
	}
	return nil
}

type p4Endpoint struct{}

func (p4Endpoint) ClearSrc()           {}
func (p4Endpoint) SrcToString() string { return "192.0.2.2:1" }
func (p4Endpoint) DstToString() string { return "192.0.2.1:1" }
func (p4Endpoint) DstToBytes() []byte  { return []byte{192, 0, 2, 1, 0, 1} }
func (p4Endpoint) DstIP() netip.Addr   { return netip.MustParseAddr("192.0.2.1") }
func (p4Endpoint) SrcIP() netip.Addr   { return netip.MustParseAddr("192.0.2.2") }

type p4Tun struct {
	closed chan struct{}
	events chan tun.Event
	once   sync.Once
}

func (*p4Tun) File() *os.File                   { return nil }
func (*p4Tun) MTU() (int, error)                { return 1420, nil }
func (*p4Tun) Name() (string, error)            { return "p4-memory", nil }
func (*p4Tun) BatchSize() int                   { return 1 }
func (t *p4Tun) Events() <-chan tun.Event       { return t.events }
func (*p4Tun) Write([][]byte, int) (int, error) { return 0, os.ErrInvalid }
func (t *p4Tun) Read([][]byte, []int, int) (int, error) {
	<-t.closed
	return 0, os.ErrClosed
}
func (t *p4Tun) Close() error {
	t.once.Do(func() { close(t.closed); close(t.events) })
	return nil
}

func p4Device(t *testing.T, headerKey []byte, trailers, disable bool) (*Device, *p4Bind, NoisePrivateKey) {
	t.Helper()
	b := &p4Bind{sent: make(chan []byte, 8)}
	d := NewDevice(&p4Tun{closed: make(chan struct{}), events: make(chan tun.Event)}, b, NewLogger(LogLevelSilent, ""))
	t.Cleanup(func() { d.Close(); <-d.Wait() })
	sk, err := newPrivateKey()
	if err != nil || d.SetPrivateKey(sk) != nil {
		t.Fatal("synthetic identity setup failed")
	}
	config := fmt.Sprintf("s1=24\ns2=32\ns3=40\ns4=48\nh1=101\nh2=202\nh3=303\nh4=404\nheader_protection_key=%s\nrandom_trailers=%t\ndisable_cookies=%t\n", hex.EncodeToString(headerKey), trailers, disable)
	if d.IpcSet(config) != nil {
		t.Fatal("synthetic obfuscation setup failed")
	}
	return d, b, sk
}

func p4Inject(t *testing.T, d *Device, packet []byte) {
	t.Helper()
	buffer := d.GetMessageBuffer()
	copy(buffer[:], packet)
	// Exactly the real handshake worker input after receive-side decryption;
	// one real pool buffer, not an artificial queue flood or direct cookie call.
	elem := QueueHandshakeElement{msgType: MessageInitiationType, packet: buffer[:len(packet)], endpoint: p4Endpoint{}, buffer: buffer}
	select {
	case d.queue.handshake.c <- elem:
	case <-time.After(time.Second):
		d.PutMessageBuffer(buffer)
		t.Fatal("bounded handshake admission timed out")
	}
}

func p4Capture(t *testing.T, b *p4Bind) []byte {
	t.Helper()
	select {
	case packet := <-b.sent:
		return packet
	case <-time.After(2 * time.Second):
		t.Fatal("bounded engine response timed out")
		return nil
	}
}

func p4Decode(t *testing.T, d *Device, wire []byte, wantType uint32, wantPadding uint32, wantHeader uint32) []byte {
	t.Helper()
	if len(wire) < HeaderCipherNonceSize {
		t.Fatal("missing protected header nonce")
	}
	cipher, err := d.HeaderProtectionCipher(wire[:HeaderCipherNonceSize])
	if err != nil || cipher == nil {
		t.Fatal("header protection inactive")
	}
	var hash [4]byte
	cipher.XORKeyStream(hash[:], hash[:])
	size, kind, padding := d.DeterminePacketTypeAndPadding(wire, hash[:])
	if kind != wantType || padding != wantPadding {
		t.Fatal("actual engine packet classifier disagrees with nondefault cookie/response shape")
	}
	if wantType == MessageCookieReplyType {
		for cut := 0; cut < int(padding)+size; cut++ {
			_, truncatedKind, _ := d.DeterminePacketTypeAndPadding(wire[:cut], hash[:])
			if truncatedKind == MessageCookieReplyType {
				t.Fatal("truncated cookie classified as complete")
			}
		}
	}
	plain := bytes.Clone(wire[padding : int(padding)+size])
	cipher, _ = d.HeaderProtectionCipher(wire[:HeaderCipherNonceSize])
	cipher.XORKeyStream(plain, plain)
	if binary.LittleEndian.Uint32(plain[:4]) != wantHeader || bytes.Equal(plain[:4], wire[padding:padding+4]) {
		t.Fatal("nondefault H2/H3 header not actively protected")
	}
	return plain
}

func TestP4CookieUnderload(t *testing.T) {
	for _, trailers := range []bool{false, true} {
		for _, disable := range []bool{false, true} {
			t.Run(fmt.Sprintf("trailers=%t/disable=%t", trailers, disable), func(t *testing.T) {
				var key [32]byte
				if _, err := rand.Read(key[:]); err != nil {
					t.Fatal("synthetic header key generation failed")
				}
				client, _, clientSK := p4Device(t, key[:], trailers, disable)
				server, bind, serverSK := p4Device(t, key[:], trailers, disable)
				clientPeer, err := client.NewPeer(serverSK.publicKey())
				if err != nil {
					t.Fatal("synthetic client peer failed")
				}
				if _, err := server.NewPeer(clientSK.publicKey()); err != nil || server.Up() != nil {
					t.Fatal("synthetic server peer failed")
				}
				if len(server.queue.handshake.c) != 0 {
					t.Fatal("fixture queue must start empty")
				}
				server.rate.underLoadUntil.Store(time.Now().Add(time.Minute).UnixNano())
				if !server.IsUnderLoad() || server.disableCookies.Load() != disable {
					t.Fatal("deterministic load/cookie policy not active")
				}
				init, err := client.CreateMessageInitiation(clientPeer)
				if err != nil {
					t.Fatal("actual Noise initiation failed")
				}
				var encoded bytes.Buffer
				if binary.Write(&encoded, binary.LittleEndian, init) != nil {
					t.Fatal("initiation encoding failed")
				}
				packet := encoded.Bytes()
				clientPeer.cookieGenerator.AddMacs(packet)
				if !server.cookieChecker.CheckMAC1(packet) || server.cookieChecker.CheckMAC2(packet, p4Endpoint{}.DstToBytes()) {
					t.Fatal("fixture must present valid MAC1 and absent MAC2")
				}
				p4Inject(t, server, packet)
				wire := p4Capture(t, bind)
				if !disable {
					plain := p4Decode(t, client, wire, MessageCookieReplyType, 40, 303)
					if !trailers && len(wire) != 40+MessageCookieReplySize {
						t.Fatal("non-trailer cookie length differs from S3 plus cookie")
					}
					var reply MessageCookieReply
					if binary.Read(bytes.NewReader(plain), binary.LittleEndian, &reply) != nil || reply.Receiver != init.Sender {
						t.Fatal("cookie reply does not target actual initiating Noise index")
					}
					bad := reply
					bad.Cookie[0] ^= 1
					if clientPeer.cookieGenerator.ConsumeReply(&bad) {
						t.Fatal("corrupt cookie authenticated")
					}
					var wrongContext CookieGenerator
					wrongContext.Init(serverSK.publicKey())
					if wrongContext.ConsumeReply(&reply) || !clientPeer.cookieGenerator.ConsumeReply(&reply) {
						t.Fatal("cookie authentication failed to distinguish MAC1 context")
					}
					clientPeer.cookieGenerator.AddMacs(packet)
					if !server.cookieChecker.CheckMAC2(packet, p4Endpoint{}.DstToBytes()) {
						t.Fatal("cookie challenge did not yield valid source-bound MAC2")
					}
					if server.cookieChecker.CheckMAC2(packet, []byte{192, 0, 2, 9, 0, 1}) {
						t.Fatal("cookie MAC2 accepted a different synthetic source")
					}
					p4Inject(t, server, packet)
					wire = p4Capture(t, bind)
				}
				plain := p4Decode(t, client, wire, MessageResponseType, 32, 202)
				if !trailers && len(wire) != 32+MessageResponseSize {
					t.Fatal("non-trailer response length differs from S2 plus response")
				}
				if !client.cookieChecker.CheckMAC1(plain) {
					t.Fatal("actual response MAC1 invalid")
				}
				var response MessageResponse
				if binary.Read(bytes.NewReader(plain), binary.LittleEndian, &response) != nil {
					t.Fatal("response decoding failed")
				}
				response.Type = MessageResponseType // same normalization as real receive worker
				if client.ConsumeMessageResponse(&response) != clientPeer {
					t.Fatal("actual Noise handshake response failed")
				}
			})
		}
	}
}
