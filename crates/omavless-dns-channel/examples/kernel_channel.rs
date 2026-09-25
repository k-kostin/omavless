// SPDX-License-Identifier: MIT
//! TEST ONLY, fresh user/net/PID/mount namespaces. No resolved or DNS effects.
//! Fixed statuses deliberately simulate DNS completion; not a deployed broker.
use omavless_dns_channel::{Error, Listener, Response};
use omavless_dns_tun::HeldTun;
use std::{
    io::{self, BufRead, Read, Write},
    path::Path,
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

fn emit(value: &str) -> Result<(), ()> {
    println!("{value}");
    io::stdout().flush().map_err(|_| ())
}

fn expect(commands: &Receiver<String>, wanted: &str) -> Result<(), ()> {
    match commands.recv_timeout(Duration::from_secs(15)) {
        Ok(command) if command == wanted => Ok(()),
        _ => Err(()),
    }
}

fn run() -> Result<(), ()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 7
        || args[1] != "--fixture"
        || !matches!(args[2].as_str(), "success" | "reject" | "loss")
        || !rustix::process::geteuid().is_root()
    {
        return Err(());
    }
    for (kind, original) in ["net", "user", "pid", "mnt"].into_iter().zip(&args[3..]) {
        let current = std::fs::read_link(format!("/proc/thread-self/ns/{kind}")).map_err(|_| ())?;
        if !original.starts_with(&format!("{kind}:["))
            || !original.ends_with(']')
            || current.to_str() == Some(original)
        {
            return Err(());
        }
    }
    let listener =
        Listener::bind(Path::new("/run/omavless-dns/control.sock"), 0).map_err(|_| ())?;
    emit("fixture_ready")?;
    let mut session = listener.accept().map_err(|_| ())?;
    let descriptor = session
        .receive_acquire()
        .map_err(|_| ())?
        .try_clone_to_owned()
        .map_err(|_| ())?;
    let held = HeldTun::admit(descriptor).map_err(|_| ())?;
    held.recheck().map_err(|_| ())?;
    emit("proof_admitted")?;

    let (send, commands) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let input = io::stdin();
        let mut locked = input.lock();
        for _ in 0..3 {
            let mut command = String::new();
            if (&mut locked).take(32).read_line(&mut command).is_err()
                || !command.ends_with('\n')
                || send.send(command).is_err()
            {
                return;
            }
        }
    });

    if args[2] == "reject" {
        expect(&commands, "reject\n")?;
        held.recheck().map_err(|_| ())?;
        session.reply(Response::Rejected).map_err(|_| ())?;
        emit("lease_rejected")?;
    } else {
        expect(&commands, "ready\n")?;
        held.recheck().map_err(|_| ())?;
        session.reply(Response::Applying).map_err(|_| ())?;
        // Synthetic completion only: this example never writes DNS.
        session.reply(Response::Ready).map_err(|_| ())?;
        emit("lease_ready")?;
        if args[2] == "loss" {
            expect(&commands, "drop_channel\n")?;
            drop(session);
            held.recheck().map_err(|_| ())?;
            emit("channel_lost")?;
            expect(&commands, "drop_proof\n")?;
            held.recheck().map_err(|_| ())?;
            drop(held);
            return emit("proof_dropped");
        }
        let until = Instant::now() + Duration::from_secs(20);
        loop {
            match session.receive_release() {
                Ok(()) => break,
                Err(Error::Idle) if Instant::now() < until => continue,
                _ => return Err(()),
            }
        }
        held.recheck().map_err(|_| ())?;
        session.reply(Response::Releasing).map_err(|_| ())?;
        session.reply(Response::Released).map_err(|_| ())?;
        emit("lease_released")?;
    }
    expect(&commands, "drop_proof\n")?;
    held.recheck().map_err(|_| ())?;
    drop(session);
    drop(held);
    emit("proof_dropped")
}

fn main() {
    if run().is_err() {
        eprintln!("fixture_failed");
        std::process::exit(1);
    }
}
