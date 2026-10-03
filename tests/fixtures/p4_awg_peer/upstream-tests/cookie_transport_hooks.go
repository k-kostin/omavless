//go:build p4_cookie_transport

// SPDX-License-Identifier: MIT
// Developer-only ADD-ONLY virtual overlay at the immutable official peer pin.
// No existing upstream implementation is replaced; normal builds omit this tag.
package device

import (
	"bytes"
	"encoding/binary"
	"time"
)

// P4FixtureForceUnderload exercises the existing real worker without traffic
// flooding. One phase has a fixed 90-second deadline; callers cannot extend or
// choose it through IPC. It is never exposed by a normal application or peer.
func (d *Device) P4FixtureForceUnderload() bool {
	if d.isClosed() {
		return false
	}
	d.rate.underLoadUntil.Store(time.Now().Add(90 * time.Second).UnixNano())
	return d.IsUnderLoad()
}

func (d *Device) p4FixtureDecode(wire []byte) (uint32, int, []byte) {
	if len(wire) < HeaderCipherNonceSize || len(wire) > MaxMessageSize {
		return MessageUnknownType, 0, nil
	}
	cipher, err := d.HeaderProtectionCipher(wire[:HeaderCipherNonceSize])
	if err != nil || cipher == nil { // This fixture requires active header protection.
		return MessageUnknownType, 0, nil
	}
	var hash [4]byte
	cipher.XORKeyStream(hash[:], hash[:])
	size, kind, padding := d.DeterminePacketTypeAndPadding(wire, hash[:])
	if kind != MessageInitiationType && kind != MessageCookieReplyType || size <= 0 || int(padding)+size > len(wire) {
		return MessageUnknownType, 0, nil
	}
	plain := bytes.Clone(wire[padding : int(padding)+size])
	cipher, _ = d.HeaderProtectionCipher(wire[:HeaderCipherNonceSize])
	cipher.XORKeyStream(plain, plain)
	return kind, int(padding), plain
}

// Only booleans leave this helper. MAC2 is verified using the actual server
// checker/secret and actual Bind endpoint bytes, not inferred from nonzero data.
func (d *Device) P4FixtureMACFacts(wire, source []byte) (initiation, mac1, mac2 bool) {
	kind, _, plain := d.p4FixtureDecode(wire)
	if kind != MessageInitiationType || len(plain) != MessageInitiationSize {
		return false, false, false
	}
	mac1 = d.cookieChecker.CheckMAC1(plain)
	return true, mac1, mac1 && d.cookieChecker.CheckMAC2(plain, source)
}

// Locate the authentication field from the actually classified/decoded cookie
// structure. Never guess a frame from datagram length or a raw header offset.
func (d *Device) P4FixtureCookieAuthField(wire []byte) (begin, width int, classified bool) {
	kind, padding, plain := d.p4FixtureDecode(wire)
	if kind != MessageCookieReplyType || len(plain) != MessageCookieReplySize {
		return 0, 0, false
	}
	var reply MessageCookieReply
	if binary.Read(bytes.NewReader(plain), binary.LittleEndian, &reply) != nil || !d.headers.cookie.Load().Contains(reply.Type) {
		return 0, 0, false
	}
	begin = padding + binary.Size(reply.Type) + binary.Size(reply.Receiver) + binary.Size(reply.Nonce)
	width = len(reply.Cookie)
	if width == 0 || begin < padding || begin+width != padding+MessageCookieReplySize || begin+width > len(wire) {
		return 0, 0, false
	}
	return begin, width, true
}
