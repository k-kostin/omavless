// SPDX-License-Identifier: MIT
use super::*;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream, UdpSocket};

const QUESTION: &[u8] = b"\x05probe\x02k1\x07invalid\0\0\x01\0\x01";
const REQUEST: &[u8] = b"GET /k1 HTTP/1.1\r\nHost: probe.k1.invalid\r\nConnection: close\r\n\r\n";
const HTTP: &[u8] = b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: text/plain\r\nContent-Length: 18\r\n\r\nK1 synthetic HTTP\n";
fn dns_query() -> Vec<u8> {
    [b"\x4b\x31\x01\0\0\x01\0\0\0\0\0\0".as_slice(), QUESTION].concat()
}
fn dns_answer() -> Vec<u8> {
    [
        b"\x4b\x31\x85\x80\0\x01\0\x01\0\0\0\0".as_slice(),
        QUESTION,
        b"\xc0\x0c\0\x01\0\x01\0\0\0\0\0\x04\xc0\0\x02\x50",
    ]
    .concat()
}
fn remaining(deadline: Instant) -> Result<Duration, ()> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or(())
}
fn traffic() -> Result<(), ()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let resolver = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 53), 53));
    let dns = UdpSocket::bind((Ipv4Addr::new(198, 18, 0, 1), 40531)).map_err(|_| ())?;
    dns.set_read_timeout(Some(Duration::from_secs(12)))
        .map_err(|_| ())?;
    dns.set_write_timeout(Some(Duration::from_secs(12)))
        .map_err(|_| ())?;
    let query = dns_query();
    if dns.send_to(&query, resolver).map_err(|_| ())? != query.len() {
        return Err(());
    }
    let mut response = [0; 513];
    let (count, peer) = dns.recv_from(&mut response).map_err(|_| ())?;
    if peer != resolver || response[..count] != dns_answer() {
        return Err(());
    }
    drop(dns);
    let target = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 80), 80));
    let mut http = TcpStream::connect_timeout(&target, remaining(deadline)?).map_err(|_| ())?;
    http.set_write_timeout(Some(remaining(deadline)?))
        .map_err(|_| ())?;
    http.write_all(REQUEST).map_err(|_| ())?;
    let mut reply = Vec::new();
    loop {
        http.set_read_timeout(Some(remaining(deadline)?))
            .map_err(|_| ())?;
        let mut part = [0; 256];
        let count = http.read(&mut part).map_err(|_| ())?;
        if count == 0 {
            break;
        }
        reply.extend_from_slice(&part[..count]);
        if reply.len() > HTTP.len() {
            return Err(());
        }
    }
    if reply != HTTP {
        return Err(());
    }
    remaining(deadline)?;
    Ok(())
}

#[test]
#[ignore = "VM-only original child of the protected native interval; network effects"]
fn fixed_native_traffic() {
    let result = (|| {
        if nix::unistd::getuid().as_raw() != 1000 || nix::unistd::geteuid().as_raw() != 1000 {
            return Err(());
        }
        nix::sys::resource::setrlimit(nix::sys::resource::Resource::RLIMIT_FSIZE, BOUND, BOUND)
            .map_err(|_| ())?;
        let mut release = [0];
        std::io::stdin().read_exact(&mut release).map_err(|_| ())?;
        if release != *b"G" {
            return Err(());
        }
        traffic()?;
        std::io::stderr().write_all(SUCCESS).map_err(|_| ())?;
        std::io::stderr().flush().map_err(|_| ())
    })();
    // Bypass variable libtest completion/filtered counts. Parent accepts only
    // this fixed success record, original zero and the fixed terse preamble.
    std::process::exit(if result.is_ok() { 0 } else { 1 });
}

#[test]
fn fixed_dns_protocol_is_exact() {
    assert_eq!(dns_query().len(), 34);
    assert_eq!(dns_answer().len(), 50);
    assert_eq!(&dns_answer()[46..], &[192, 0, 2, 80]);
    assert!(
        REQUEST
            .windows(b"Host: probe.k1.invalid\r\n".len())
            .any(|s| s == b"Host: probe.k1.invalid\r\n")
    );
}
#[test]
fn child_privilege_and_parent_are_closed() {
    let good = "PPid:\t12\nUid:\t1000\t1000\t1000\t1000\nCapPrm:\t0000000000000000\nCapEff:\t0000000000000000\nCapAmb:\t0000000000000000\n";
    assert!(capless_child(good, 12, 1000));
    assert!(!capless_child(good, 13, 1000));
    assert!(!capless_child(
        &good.replace("CapEff:\t0000000000000000", "CapEff:\t0000000000003400"),
        12,
        1000
    ));
    assert!(!capless_child(
        &format!("{good}CapEff:\t0000000000000000\n"),
        12,
        1000
    ));
}
#[test]
fn terminal_capture_rejects_replacement_and_overflow() {
    let temp = tempfile::tempdir().unwrap();
    let uid = nix::unistd::getuid().as_raw();
    let capture = Capture::create(temp.path().join("out"), uid).unwrap();
    capture.file.write_at(b"fixed", 0).unwrap();
    assert_eq!(capture.whole().unwrap(), b"fixed");
    capture.file.set_len(BOUND + 1).unwrap();
    assert!(capture.whole().is_err());
    capture.file.set_len(5).unwrap();
    fs::rename(&capture.path, temp.path().join("old")).unwrap();
    fs::write(&capture.path, b"fixed").unwrap();
    assert!(capture.whole().is_err());
}

#[test]
fn nonterminal_drop_and_unwind_retain_original_capture_descriptor() {
    use std::os::fd::AsRawFd;
    for unwind in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let capture =
            Capture::create(temp.path().join("out"), nix::unistd::getuid().as_raw()).unwrap();
        let original = capture.file.as_raw_fd();
        let owner = Interval {
            original: Some(Resources {
                capacity: Capacity { ceiling: TOTAL_FDS },
                program: None,
                directory: None,
                path: temp.path().to_owned(),
                stdout: Some(capture),
                stderr: None,
                child: None,
                image: None,
                deadline: Instant::now() + BUDGET,
                reaped: false,
            }),
            completed: false,
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _retained = owner;
            if unwind {
                panic!("inert original-prefix cut");
            }
        }));
        assert_eq!(result.is_err(), unwind);
        assert!(fs::metadata(format!("/proc/self/fd/{original}")).is_ok());
        // This is the test's original synthetic file, not an actor/capture from
        // an uncertain VM scope. Close only after proving the Drop retained it.
        nix::unistd::close(original).unwrap();
    }
}

#[test]
fn aggregate_capacity_counts_old_graph_and_temporary_roles() {
    assert!(Capacity::from_inventory(232, 256).is_ok());
    assert!(Capacity::from_inventory(233, 4096).is_err());
    assert!(Capacity::from_inventory(232, 255).is_err());
    assert!(Capacity::from_inventory(usize::MAX, u64::MAX).is_err());
    assert_eq!(PEAK_INTERVAL_FDS, 24);
}
