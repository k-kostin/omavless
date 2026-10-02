// SPDX-License-Identifier: MIT
// Developer-only synthetic peer; never installed or called by application IPC.
package main

import (
	"encoding/base64"
	"encoding/binary"
	"encoding/json"
	"net"
	"os"
	"os/signal"
	"path/filepath"
	"strconv"
	"sync"
	"syscall"

	"github.com/amnezia-vpn/amneziawg-go/v3/conn"
	"github.com/amnezia-vpn/amneziawg-go/v3/device"
	"github.com/amnezia-vpn/amneziawg-go/v3/tun"
	"golang.org/x/crypto/chacha20"
	"golang.org/x/sys/unix"
)

type observations struct {
	sync.Mutex
	Junk, Special, Init, Response, Transport, Protected, Trailers, Padding int
	Wire, Plain                                                            map[int]int
}

func (o *observations) snapshot() map[string]int {
	o.Lock()
	defer o.Unlock()
	padding := 0
	for size, n := range o.Plain {
		padding += min(n, o.Wire[size+48+32+37])
	}
	return map[string]int{"junk": o.Junk, "special": o.Special, "init": o.Init,
		"response": o.Response, "transport": o.Transport, "protected": o.Protected,
		"trailers": o.Trailers, "padding_size_matches": padding}
}

type observedTun struct {
	tun.Device
	observations *observations
}

func (t *observedTun) Write(bufs [][]byte, offset int) (int, error) {
	n, err := t.Device.Write(bufs, offset)
	t.observations.Lock()
	defer t.observations.Unlock()
	for _, packet := range bufs[:n] {
		t.observations.Plain[len(packet)-offset]++
	}
	return n, err
}

// The relay observes only our own encrypted datagrams. No packet bytes, keys,
// addresses or peer identities are returned. Header decoding independently
// checks the configured padding/magic values rather than trusting a YAML flag.
func relay(root string, o *observations) (*net.UDPConn, error) {
	encoded, err := os.ReadFile(filepath.Join(root, "header.key"))
	if err != nil {
		return nil, err
	}
	key, err := base64.StdEncoding.DecodeString(string(encoded))
	if err != nil || len(key) != 32 {
		return nil, syscall.EINVAL
	}
	sock, err := net.ListenUDP("udp4", &net.UDPAddr{IP: net.IPv4(127, 0, 0, 1), Port: 51888})
	if err != nil {
		return nil, err
	}
	go func() {
		var client *net.UDPAddr
		buf := make([]byte, 2048)
		for {
			n, from, err := sock.ReadFromUDP(buf)
			if err != nil {
				return
			}
			if n == len(buf) || !from.IP.IsLoopback() {
				continue
			}
			o.Lock()
			if n >= 64 && n <= 66 {
				o.Junk++
			}
			if n >= 20 && n <= 24 && string(buf[:4]) == "P4AW" {
				o.Special++
			}
			if n >= 16 {
				for i, padding := range []int{24, 32, 40, 48} {
					if n < padding+4 {
						continue
					}
					cipher, _ := chacha20.NewUnauthenticatedCipher(key, buf[:12])
					var header [4]byte
					cipher.XORKeyStream(header[:], buf[padding:padding+4])
					if binary.LittleEndian.Uint32(header[:]) != uint32(101+i*101) {
						continue
					}
					o.Protected++
					switch i {
					case 0:
						o.Init++
						if n > padding+148 {
							o.Trailers++
						}
					case 1:
						o.Response++
						if n > padding+92 {
							o.Trailers++
						}
					case 3:
						o.Transport++
						if from.Port != 51889 {
							o.Wire[n]++
						}
					}
					break
				}
			}
			o.Unlock()
			if from.Port == 51889 {
				if client != nil {
					_, _ = sock.WriteToUDP(buf[:n], client)
				}
			} else {
				client = from
				_, _ = sock.WriteToUDP(buf[:n], &net.UDPAddr{IP: net.IPv4(127, 0, 0, 1), Port: 51889})
			}
		}
	}()
	return sock, nil
}

func run(root string) error {
	info, err := os.Lstat(root)
	if err != nil || !filepath.IsAbs(root) || !info.IsDir() || info.Mode().Perm()&077 != 0 || os.Getuid() != 0 {
		return syscall.EINVAL
	}
	// Retained outside namespace FDs are authority anchors, never a PID lookup
	// after capability drop (the parent may no longer be ptrace-readable).
	for _, name := range []string{"net", "user"} {
		fd, e := strconv.Atoi(os.Getenv("P4_PARENT_" + name + "_FD"))
		if e != nil {
			return e
		}
		var held, current unix.Stat_t
		if unix.Fstat(fd, &held) != nil || unix.Stat("/proc/self/ns/"+name, &current) != nil || held.Ino == current.Ino {
			return syscall.EINVAL
		}
	}
	fd, err := strconv.Atoi(os.Getenv("P4_TUN_FD"))
	if err != nil {
		return err
	}
	// The supervising namespace owner creates and configures this FD. The
	// executable starts with zero capabilities; it never needs NET_ADMIN.
	tdev, name, err := tun.CreateUnmonitoredTUNFromFD(fd)
	if err != nil {
		return err
	}
	if name != "wg-p4" {
		tdev.Close()
		return syscall.EINVAL
	}
	defer tdev.Close()
	o := &observations{Wire: map[int]int{}, Plain: map[int]int{}}
	sock, err := relay(root, o)
	if err != nil {
		return err
	}
	defer sock.Close()
	dev := device.NewDevice(&observedTun{tdev, o}, conn.NewDefaultBind(), device.NewLogger(device.LogLevelSilent, ""))
	defer dev.Close()
	if err := dev.Up(); err != nil {
		return err
	}
	listener, err := net.ListenUnix("unix", &net.UnixAddr{Name: filepath.Join(root, "peer.sock"), Net: "unix"})
	if err != nil {
		return err
	}
	defer listener.Close()
	if err := os.Chmod(filepath.Join(root, "peer.sock"), 0600); err != nil {
		return err
	}
	stats, err := net.ListenUnix("unix", &net.UnixAddr{Name: filepath.Join(root, "stats.sock"), Net: "unix"})
	if err != nil {
		return err
	}
	defer stats.Close()
	if err := os.Chmod(filepath.Join(root, "stats.sock"), 0600); err != nil {
		return err
	}
	go func() {
		for {
			c, e := listener.Accept()
			if e != nil {
				return
			}
			go dev.IpcHandle(c)
		}
	}()
	go func() {
		for {
			c, e := stats.Accept()
			if e != nil {
				return
			}
			_ = json.NewEncoder(c).Encode(o.snapshot())
			_ = c.Close()
		}
	}()
	ended := make(chan os.Signal, 1)
	signal.Notify(ended, syscall.SIGTERM, os.Interrupt)
	<-ended
	return nil
}

func main() {
	unix.Umask(0077)
	if len(os.Args) != 2 || run(os.Args[1]) != nil {
		os.Stderr.WriteString("p4_awg_peer_refused\n")
		os.Exit(2)
	}
}
