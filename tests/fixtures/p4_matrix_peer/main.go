// SPDX-License-Identifier: MIT
// Developer fixture only: pinned AWG engine, including its standard-WG mode.
package main

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
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
	"golang.org/x/sys/unix"
)

// No packet bytes or addresses enter the receipt. Length is the IP-declared
// length, checked against the actual decrypted TUN packet, never app payload.
func shape(p []byte) (family, size, fragment int, okay bool) {
	if len(p) < 20 {
		return
	}
	switch p[0] >> 4 {
	case 4:
		family, size = 4, int(binary.BigEndian.Uint16(p[2:4]))
		hlen := int(p[0]&15) * 4
		if hlen < 20 || size < hlen || size != len(p) {
			return 0, 0, 0, false
		}
		if binary.BigEndian.Uint16(p[6:8])&0x3fff != 0 {
			fragment = 1
		}
	case 6:
		if len(p) < 40 {
			return 0, 0, 0, false
		}
		family, size = 6, 40+int(binary.BigEndian.Uint16(p[4:6]))
		if size != len(p) {
			return 0, 0, 0, false
		}
		next, pos := p[6], 40
		for depth := 0; depth < 8; depth++ {
			switch next {
			case 44:
				if pos+8 > size {
					return 0, 0, 0, false
				}
				fragment = 1
				return family, size, fragment, true
			case 0, 43, 60:
				if pos+2 > size {
					return 0, 0, 0, false
				}
				n, step := p[pos], (int(p[pos+1])+1)*8
				if pos+step > size {
					return 0, 0, 0, false
				}
				next, pos = n, pos+step
			default:
				return family, size, fragment, true
			}
		}
		return 0, 0, 0, false
	default:
		return 0, 0, 0, false
	}
	return family, size, fragment, true
}

type observations struct {
	sync.Mutex
	Packets     map[string]int `json:"packets"`
	Outer       map[string]int `json:"outer"`
	Malformed   int            `json:"malformed"`
	WriteErrors int            `json:"write_errors"`
	Flows       []flow         `json:"flows"`
}

type flow struct {
	Direction   string `json:"direction"`
	Family      int    `json:"family"`
	Size        int    `json:"size"`
	ID          uint32 `json:"id"`
	Offset      int    `json:"offset"`
	More        bool   `json:"more"`
	Fragment    bool   `json:"fragment"`
	PayloadSize int    `json:"fragment_payload_size"`
	SourcePort  int    `json:"source_port"`
	DestPort    int    `json:"dest_port"`
	PayloadSHA  string `json:"payload_sha256"`
}

func udpFlow(direction string, p []byte) (f flow, okay bool) {
	family, size, _, valid := shape(p)
	if !valid {
		return
	}
	f.Direction, f.Family, f.Size = direction, family, size
	pos, proto := 20, byte(0)
	var src, dst net.IP
	if family == 4 {
		src, dst = net.IP(p[12:16]), net.IP(p[16:20])
		pos, proto = int(p[0]&15)*4, p[9]
		bits := binary.BigEndian.Uint16(p[6:8])
		f.Offset = int(bits&0x1fff) * 8
		f.More = bits&0x2000 != 0
		f.Fragment = f.Offset != 0 || f.More
		f.ID = uint32(binary.BigEndian.Uint16(p[4:6]))
	} else {
		src, dst = net.IP(p[8:24]), net.IP(p[24:40])
		pos, proto = 40, p[6]
		for depth := 0; depth < 8; depth++ {
			if proto == 44 {
				if pos+8 > size {
					return f, false
				}
				bits := binary.BigEndian.Uint16(p[pos+2 : pos+4])
				f.Offset = int(bits & 0xfff8)
				f.More = bits&1 != 0
				f.Fragment = true
				f.ID = binary.BigEndian.Uint32(p[pos+4 : pos+8])
				proto = p[pos]
				pos += 8
				break
			}
			if proto != 0 && proto != 43 && proto != 60 {
				break
			}
			if pos+2 > size {
				return f, false
			}
			step := (int(p[pos+1]) + 1) * 8
			if pos+step > size {
				return f, false
			}
			proto = p[pos]
			pos += step
		}
	}
	server, client := net.ParseIP("10.203.0.1"), net.ParseIP("10.203.0.2")
	if family == 6 {
		server, client = net.ParseIP("fd20:203::1"), net.ParseIP("fd20:203::2")
	}
	if proto != 17 || !(direction == "rx" && src.Equal(client) && dst.Equal(server) || direction == "tx" && src.Equal(server) && dst.Equal(client)) {
		return f, false
	}
	f.PayloadSize = size - pos
	if f.Offset == 0 {
		if pos+8 > size {
			return f, false
		}
		f.SourcePort = int(binary.BigEndian.Uint16(p[pos : pos+2]))
		f.DestPort = int(binary.BigEndian.Uint16(p[pos+2 : pos+4]))
		if direction == "rx" && f.DestPort != 8090 || direction == "tx" && f.SourcePort != 8090 {
			return f, false
		}
		if !f.Fragment {
			if int(binary.BigEndian.Uint16(p[pos+4:pos+6])) != size-pos {
				return f, false
			}
			h := sha256.Sum256(p[pos+8:])
			f.PayloadSHA = hex.EncodeToString(h[:])
		}
	}
	return f, true
}

func (o *observations) packet(direction string, p []byte) {
	family, size, fragment, okay := shape(p)
	o.Lock()
	defer o.Unlock()
	if !okay {
		o.Malformed++
		return
	}
	key := direction + ":" + strconv.Itoa(family) + ":" + strconv.Itoa(size) + ":" + strconv.Itoa(fragment)
	if len(o.Packets) >= 256 && o.Packets[key] == 0 {
		o.Malformed++
		return
	}
	o.Packets[key]++
	if f, valid := udpFlow(direction, p); valid {
		if len(o.Flows) >= 256 {
			o.Malformed++
		} else {
			o.Flows = append(o.Flows, f)
		}
	}
}

func (o *observations) snapshot() map[string]any {
	o.Lock()
	defer o.Unlock()
	p, w := map[string]int{}, map[string]int{}
	for key, count := range o.Packets {
		p[key] = count
	}
	for key, count := range o.Outer {
		w[key] = count
	}
	flows := append([]flow{}, o.Flows...)
	return map[string]any{"packets": p, "outer": w, "malformed": o.Malformed, "write_errors": o.WriteErrors, "flows": flows}
}

type observedTun struct {
	tun.Device
	seen *observations
}

func (t *observedTun) Read(bufs [][]byte, sizes []int, offset int) (int, error) {
	n, err := t.Device.Read(bufs, sizes, offset)
	if n >= 0 && n <= len(bufs) && n <= len(sizes) {
		for i := 0; i < n; i++ {
			if sizes[i] >= 0 && offset >= 0 && offset+sizes[i] <= len(bufs[i]) {
				t.seen.packet("tx", bufs[i][offset:offset+sizes[i]])
			}
		}
	}
	return n, err
}

func (t *observedTun) Write(bufs [][]byte, offset int) (int, error) {
	n, err := t.Device.Write(bufs, offset)
	if err == nil && n > 0 {
		// Linux NativeTun returns bytes, not packets. Preserve that contract.
		for _, p := range bufs {
			if offset >= 0 && offset <= len(p) {
				t.seen.packet("rx", p[offset:])
			}
		}
	} else if err != nil {
		t.seen.Lock()
		t.seen.WriteErrors++
		t.seen.Unlock()
	}
	return n, err
}

func relay(family string, o *observations) (*net.UDPConn, error) {
	address := "127.0.0.1"
	if family == "6" {
		address = "::1"
	} else if family != "4" {
		return nil, syscall.EINVAL
	}
	ip := net.ParseIP(address)
	sock, err := net.ListenUDP("udp"+family, &net.UDPAddr{IP: ip, Port: 51888})
	if err != nil {
		return nil, err
	}
	go func() {
		buf := make([]byte, 4096)
		var client *net.UDPAddr
		for {
			n, from, e := sock.ReadFromUDP(buf)
			if e != nil {
				return
			}
			if n == len(buf) || !from.IP.Equal(ip) || n == 0 {
				continue
			}
			actual := "6"
			if from.IP.To4() != nil {
				actual = "4"
			}
			direction := "client"
			if from.Port == 51889 {
				direction = "peer"
			}
			o.Lock()
			key := direction + ":" + actual + ":loopback:" + strconv.Itoa(n)
			if len(o.Outer) < 256 || o.Outer[key] != 0 {
				o.Outer[key]++
			} else {
				o.Malformed++
			}
			o.Unlock()
			if direction == "peer" {
				if client != nil {
					_, _ = sock.WriteToUDP(buf[:n], client)
				}
			} else {
				client = from
				_, _ = sock.WriteToUDP(buf[:n], &net.UDPAddr{IP: ip, Port: 51889})
			}
		}
	}()
	return sock, nil
}

func authority(root string) error {
	info, err := os.Lstat(root)
	if err != nil || !filepath.IsAbs(root) || !info.IsDir() || info.Mode().Perm()&077 != 0 || os.Getuid() != 0 {
		return syscall.EINVAL
	}
	for _, name := range []string{"net", "user"} {
		fd, e := strconv.Atoi(os.Getenv("P4_PARENT_" + name + "_FD"))
		if e != nil {
			return e
		}
		var held, current unix.Stat_t
		var fs unix.Statfs_t
		label, e := os.Readlink("/proc/self/fd/" + strconv.Itoa(fd))
		if e != nil || len(label) < len(name)+2 || label[:len(name)+2] != name+":[" || unix.Fstatfs(fd, &fs) != nil || fs.Type != unix.NSFS_MAGIC || unix.Fstat(fd, &held) != nil || unix.Stat("/proc/self/ns/"+name, &current) != nil || held.Ino == current.Ino {
			return syscall.EPERM
		}
	}
	caps := [2]unix.CapUserData{}
	if unix.Capget(&unix.CapUserHeader{Version: unix.LINUX_CAPABILITY_VERSION_3}, &caps[0]) != nil {
		return syscall.EPERM
	}
	for _, c := range caps {
		if c.Effective|c.Permitted|c.Inheritable != 0 {
			return syscall.EPERM
		}
	}
	nnp, e := unix.PrctlRetInt(unix.PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0)
	if e != nil || nnp != 1 {
		return syscall.EPERM
	}
	return nil
}

func run(root, family string) error {
	if e := authority(root); e != nil {
		return e
	}
	fd, e := strconv.Atoi(os.Getenv("P4_TUN_FD"))
	if e != nil {
		return e
	}
	tdev, name, e := tun.CreateUnmonitoredTUNFromFD(fd)
	if e != nil {
		return e
	}
	defer tdev.Close()
	if name != "wg-p4" {
		return syscall.EINVAL
	}
	o := &observations{Packets: map[string]int{}, Outer: map[string]int{}}
	relay, e := relay(family, o)
	if e != nil {
		return e
	}
	defer relay.Close()
	dev := device.NewDevice(&observedTun{tdev, o}, conn.NewDefaultBind(), device.NewLogger(device.LogLevelSilent, ""))
	defer dev.Close()
	if e = dev.Up(); e != nil {
		return e
	}
	listener, e := net.ListenUnix("unix", &net.UnixAddr{Name: filepath.Join(root, "peer.sock"), Net: "unix"})
	if e != nil {
		return e
	}
	defer listener.Close()
	stats, e := net.ListenUnix("unix", &net.UnixAddr{Name: filepath.Join(root, "stats.sock"), Net: "unix"})
	if e != nil {
		return e
	}
	defer stats.Close()
	for _, name := range []string{"peer.sock", "stats.sock"} {
		if e = os.Chmod(filepath.Join(root, name), 0600); e != nil {
			return e
		}
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
	if len(os.Args) != 3 || run(os.Args[1], os.Args[2]) != nil {
		os.Stderr.WriteString("p4_matrix_peer_refused\n")
		os.Exit(2)
	}
}
