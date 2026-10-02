// SPDX-License-Identifier: MIT
// Developer-only synthetic peer; never installed or called by application IPC.
package main

import (
	"bytes"
	"encoding/base64"
	"encoding/binary"
	"encoding/json"
	"io"
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
	Junk, Special, SpecialMask, Init, Response, Transport, Protected, Trailers int
	Wire, Plain                                                                map[int]int
}

func (o *observations) snapshot() map[string]int {
	o.Lock()
	defer o.Unlock()
	padding := 0
	for size, n := range o.Plain {
		padding += min(n, o.Wire[size+48+32+37])
	}
	return map[string]int{"junk": o.Junk, "special": o.Special, "special_mask": o.SpecialMask, "init": o.Init,
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
	// The upstream Linux NativeTun returns the number of bytes written (not
	// packets). Record the input packet shapes only after a successful batch;
	// never use that byte count as a slice bound or change its return value.
	if err == nil && n > 0 {
		for _, packet := range bufs {
			t.observations.Plain[len(packet)-offset]++
		}
	}
	return n, err
}

// The relay observes only our own encrypted datagrams. No packet bytes, keys,
// addresses or peer identities are returned. Header decoding independently
// checks the configured padding/magic values rather than trusting a YAML flag.
func relay(root string, o *observations) (*net.UDPConn, error) {
	fd, err := unix.Open(filepath.Join(root, "header.key"), unix.O_RDONLY|unix.O_NOFOLLOW|unix.O_NONBLOCK|unix.O_CLOEXEC, 0)
	if err != nil {
		return nil, err
	}
	file := os.NewFile(uintptr(fd), "header.key")
	defer file.Close()
	var before, after unix.Stat_t
	if unix.Fstat(fd, &before) != nil || before.Mode&unix.S_IFMT != unix.S_IFREG || before.Mode&0077 != 0 || before.Uid != uint32(os.Getuid()) || before.Nlink != 1 || before.Size != 44 {
		return nil, syscall.EINVAL
	}
	encoded, err := io.ReadAll(io.LimitReader(file, 45))
	if err != nil {
		return nil, err
	}
	if unix.Fstat(fd, &after) != nil || before.Dev != after.Dev || before.Ino != after.Ino || before.Mode != after.Mode || before.Uid != after.Uid || before.Nlink != after.Nlink || before.Size != after.Size || before.Mtim != after.Mtim || before.Ctim != after.Ctim {
		return nil, syscall.EINVAL
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
			if n >= 20 && n <= 24 && string(buf[:4]) == "P4AW" && buf[4] == byte(49+n-20) && bytes.Equal(buf[5:n], bytes.Repeat([]byte{'x'}, n-5)) {
				o.Special++
				o.SpecialMask |= 1 << (n - 20)
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
					if !bytes.Equal(header[:], buf[padding:padding+4]) {
						o.Protected++
					}
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
		var fs unix.Statfs_t
		label, e := os.Readlink("/proc/self/fd/" + strconv.Itoa(fd))
		if e != nil || len(label) < len(name)+2 || label[:len(name)+2] != name+":[" || unix.Fstatfs(fd, &fs) != nil || fs.Type != unix.NSFS_MAGIC || unix.Fstat(fd, &held) != nil || unix.Stat("/proc/self/ns/"+name, &current) != nil || held.Ino == current.Ino {
			return syscall.EINVAL
		}
	}
	capabilities := [2]unix.CapUserData{}
	if unix.Capget(&unix.CapUserHeader{Version: unix.LINUX_CAPABILITY_VERSION_3}, &capabilities[0]) != nil {
		return syscall.EPERM
	}
	for _, c := range capabilities {
		if c.Effective|c.Permitted|c.Inheritable != 0 {
			return syscall.EPERM
		}
	}
	nnp, e := unix.PrctlRetInt(unix.PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0)
	if e != nil || nnp != 1 {
		return syscall.EPERM
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
