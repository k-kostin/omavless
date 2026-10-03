//go:build p4_cookie_transport

// SPDX-License-Identifier: MIT
// Tagged developer peer only. Not available to normal peer/application builds.
package main

import (
	"io"
	"os"
	"path/filepath"
	"sync"
	"syscall"

	"github.com/amnezia-vpn/amneziawg-go/v3/conn"
	"github.com/amnezia-vpn/amneziawg-go/v3/device"
	"golang.org/x/sys/unix"
)

type cookieObserver struct {
	sync.Mutex
	dev                                                                 *device.Device
	corrupt                                                             bool
	cookies, corrupted, mac1NoMAC2, validMAC2, invalidMAC1, frameErrors int
	mac2AfterCookie, mac2WithoutCookie                                  int
}

func (o *cookieObserver) snapshot(facts map[string]int) {
	o.Lock()
	defer o.Unlock()
	facts["cookie"], facts["corrupted_cookie"] = o.cookies, o.corrupted
	facts["mac1_no_mac2"], facts["valid_mac2"] = o.mac1NoMAC2, o.validMAC2
	facts["invalid_mac1"], facts["cookie_frame_error"] = o.invalidMAC1, o.frameErrors
	facts["mac2_after_cookie"], facts["mac2_without_cookie"] = o.mac2AfterCookie, o.mac2WithoutCookie
	facts["forced_load"], facts["cookie_corrupt_mode"] = 0, 0
	if o.dev != nil && o.dev.IsUnderLoad() {
		facts["forced_load"] = 1
	}
	if o.corrupt {
		facts["cookie_corrupt_mode"] = 1
	}
}

func (o *cookieObserver) relay(wire []byte, fromServer bool) bool {
	if !fromServer {
		return true
	}
	o.Lock()
	defer o.Unlock()
	if o.dev == nil {
		o.frameErrors++
		return false
	}
	begin, width, classified := o.dev.P4FixtureCookieAuthField(wire)
	if !classified {
		return true
	} // Other actual message types are untouched.
	if width <= 0 || begin < 0 || begin+width > len(wire) {
		o.frameErrors++
		return false
	}
	if o.mac1NoMAC2 == 0 {
		o.frameErrors++
		return false
	}
	o.cookies++
	if o.corrupt {
		wire[begin] ^= 1 // Only encrypted cookie authentication bytes; no magic/index/nonce/trailer change.
		o.corrupted++
	}
	return true
}

type cookieBind struct {
	conn.Bind
	observer *cookieObserver
}

func (b *cookieBind) Open(port uint16) ([]conn.ReceiveFunc, uint16, error) {
	functions, actual, err := b.Bind.Open(port)
	for i, receive := range functions {
		functions[i] = func(packets [][]byte, sizes []int, endpoints []conn.Endpoint) (int, error) {
			n, err := receive(packets, sizes, endpoints)
			if n < 0 || n > len(packets) || n > len(sizes) || n > len(endpoints) {
				return 0, syscall.EINVAL
			}
			for j := 0; j < n; j++ {
				if sizes[j] <= 0 || sizes[j] > len(packets[j]) || endpoints[j] == nil {
					continue
				}
				b.observer.Lock()
				if b.observer.dev != nil {
					init, mac1, mac2 := b.observer.dev.P4FixtureMACFacts(packets[j][:sizes[j]], endpoints[j].DstToBytes())
					if init && !mac1 {
						b.observer.invalidMAC1++
					}
					if init && mac1 && !mac2 {
						b.observer.mac1NoMAC2++
					}
					if init && mac2 {
						b.observer.validMAC2++
						if b.observer.cookies > 0 {
							b.observer.mac2AfterCookie++
						} else {
							b.observer.mac2WithoutCookie++
						}
					}
				}
				b.observer.Unlock()
			}
			return n, err // Original buffers, endpoints, count and error enter the real worker unchanged.
		}
	}
	return functions, actual, err
}

func fixtureBind(o *observations) conn.Bind {
	observer := &cookieObserver{}
	o.Lock()
	o.fixture = observer
	o.Unlock()
	return &cookieBind{Bind: conn.NewDefaultBind(), observer: observer}
}

func fixtureMode(root string) (bool, error) {
	fd, err := unix.Open(filepath.Join(root, "cookie.mode"), unix.O_RDONLY|unix.O_NOFOLLOW|unix.O_NONBLOCK|unix.O_CLOEXEC, 0)
	if err != nil {
		return false, err
	}
	file := os.NewFile(uintptr(fd), "cookie.mode")
	defer file.Close()
	var before, after unix.Stat_t
	if unix.Fstat(fd, &before) != nil || before.Mode&unix.S_IFMT != unix.S_IFREG || before.Mode&0077 != 0 || before.Uid != uint32(os.Getuid()) || before.Nlink != 1 || before.Size < 5 || before.Size > 8 {
		return false, syscall.EINVAL
	}
	mode, err := io.ReadAll(io.LimitReader(file, 9))
	if err != nil || unix.Fstat(fd, &after) != nil || before.Dev != after.Dev || before.Ino != after.Ino || before.Mode != after.Mode || before.Uid != after.Uid || before.Nlink != after.Nlink || before.Size != after.Size || before.Mtim != after.Mtim || before.Ctim != after.Ctim {
		return false, syscall.EINVAL
	}
	if string(mode) != "pass\n" && string(mode) != "corrupt\n" {
		return false, syscall.EINVAL
	}
	return string(mode) == "corrupt\n", nil
}

func fixturePrepare(d *device.Device, root string) error {
	corrupt, err := fixtureMode(root)
	if err != nil {
		return err
	}
	b, ok := d.Bind().(*cookieBind)
	if !ok || !d.P4FixtureForceUnderload() {
		return syscall.EINVAL
	}
	b.observer.Lock()
	defer b.observer.Unlock()
	b.observer.dev, b.observer.corrupt = d, corrupt
	return nil
}
