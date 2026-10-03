//go:build p4_cookie_transport && p4_cookie_overlay

// SPDX-License-Identifier: MIT
package device

import (
	"bytes"
	"crypto/rand"
	"encoding/binary"
	"fmt"
	"testing"
)

func p4Protect(t *testing.T, d *Device, plain []byte) []byte {
	t.Helper()
	wire := make([]byte, 24+len(plain))
	if _, err := rand.Read(wire[:24]); err != nil {
		t.Fatal("synthetic prefix failed")
	}
	copy(wire[24:], plain)
	cipher, err := d.HeaderProtectionCipher(wire[:HeaderCipherNonceSize])
	if err != nil || cipher == nil {
		t.Fatal("synthetic protection inactive")
	}
	cipher.XORKeyStream(wire[24:], wire[24:])
	return wire
}

func TestP4CookieTransportHooks(t *testing.T) {
	for _, trailers := range []bool{false, true} {
		t.Run(fmt.Sprintf("trailers=%t", trailers), func(t *testing.T) {
			var key [32]byte
			if _, err := rand.Read(key[:]); err != nil {
				t.Fatal("synthetic key failed")
			}
			client, _, clientSK := p4Device(t, key[:], trailers, false)
			server, bind, serverSK := p4Device(t, key[:], trailers, false)
			peer, err := client.NewPeer(serverSK.publicKey())
			if err != nil {
				t.Fatal("synthetic peer failed")
			}
			if _, err := server.NewPeer(clientSK.publicKey()); err != nil || server.Up() != nil || !server.P4FixtureForceUnderload() {
				t.Fatal("synthetic server/load failed")
			}
			init, err := client.CreateMessageInitiation(peer)
			if err != nil {
				t.Fatal("actual initiation failed")
			}
			var encoded bytes.Buffer
			if binary.Write(&encoded, binary.LittleEndian, init) != nil {
				t.Fatal("initiation encode failed")
			}
			packet := encoded.Bytes()
			peer.cookieGenerator.AddMacs(packet)
			wire := p4Protect(t, client, packet)
			before := bytes.Clone(wire)
			isInit, mac1, mac2 := server.P4FixtureMACFacts(wire, p4Endpoint{}.DstToBytes())
			if !isInit || !mac1 || mac2 || !bytes.Equal(wire, before) {
				t.Fatal("initial MAC1/no-MAC2 observation failed or altered wire")
			}
			p4Inject(t, server, packet)
			cookie := p4Capture(t, bind)
			begin, width, classified := server.P4FixtureCookieAuthField(cookie)
			if !classified || width != 32 || begin != 72 || begin+width > len(cookie) {
				t.Fatal("actual cookie authentication-field classification failed")
			}
			for cut := 0; cut < begin+width; cut++ {
				if _, _, ok := server.P4FixtureCookieAuthField(cookie[:cut]); ok {
					t.Fatal("truncated cookie field accepted")
				}
			}
			corrupt := bytes.Clone(cookie)
			corrupt[begin] ^= 1
			badPlain := p4Decode(t, client, corrupt, MessageCookieReplyType, 40, 303)
			goodPlain := p4Decode(t, client, cookie, MessageCookieReplyType, 40, 303)
			if !bytes.Equal(badPlain[:32], goodPlain[:32]) {
				t.Fatal("cookie corruption changed type/index/nonce")
			}
			var bad, good MessageCookieReply
			if binary.Read(bytes.NewReader(badPlain), binary.LittleEndian, &bad) != nil || binary.Read(bytes.NewReader(goodPlain), binary.LittleEndian, &good) != nil {
				t.Fatal("cookie decode failed")
			}
			if peer.cookieGenerator.ConsumeReply(&bad) || !peer.cookieGenerator.ConsumeReply(&good) {
				t.Fatal("cookie auth did not distinguish corruption")
			}
			peer.cookieGenerator.AddMacs(packet)
			wire = p4Protect(t, client, packet)
			isInit, mac1, mac2 = server.P4FixtureMACFacts(wire, p4Endpoint{}.DstToBytes())
			if !isInit || !mac1 || !mac2 {
				t.Fatal("actual source-bound retry MAC2 observation failed")
			}
			if _, _, wrong := server.P4FixtureMACFacts(wire, []byte{192, 0, 2, 9, 0, 1}); wrong {
				t.Fatal("MAC2 source substitution accepted")
			}
			if _, _, ok := server.P4FixtureCookieAuthField(wire); ok {
				t.Fatal("initiation misclassified as cookie")
			}
		})
	}
}
