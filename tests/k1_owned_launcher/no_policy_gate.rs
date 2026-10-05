// External opt-in binary only. Included after the exact exported library tree.
// Never selected by source/inert controls; zero child pins still refuse.
fn phase(bytes: &[u8], deadline: std::time::Instant) -> Result<(), ()> {
    if std::time::Instant::now() >= deadline { return Err(()); }
    // One safe write syscall on the original borrowed stdout; no buffering,
    // flush, partial-write continuation or EINTR retry.
    let result = rustix::io::write(rustix::stdio::stdout(), bytes);
    if std::time::Instant::now() >= deadline { return Err(()); }
    match result { Ok(count) if count == bytes.len() => Ok(()), _ => Err(()) }
}

fn main() -> std::process::ExitCode {
    use std::mem::ManuallyDrop;
    use launch_acquisition::owned_launcher::Prototype;

    let Some(deadline) = std::time::Instant::now().checked_add(std::time::Duration::from_secs(5)) else {
        return std::process::ExitCode::from(2);
    };

    // This fixed entry accepts no path, descriptor, PID, mode or operation.
    let mut args = std::env::args_os();
    if args.next().is_none()
        || args.next().as_deref() != Some(std::ffi::OsStr::new("--fixed-owned-no-policy"))
        || args.next().is_some()
    {
        return std::process::ExitCode::from(2);
    }
    // The separately reviewed runner supplies a private regular output file.
    // These are original-call phase records, not post-failure queries. Output
    // failure refuses immediately; there is no second attempt or cleanup.
    if phase(b"K1_OWNER_OPEN_BEGIN\n", deadline).is_err() {
        return std::process::ExitCode::from(2);
    }
    let owner = match Prototype::open_fixed() {
        Ok(owner) => ManuallyDrop::new(owner),
        Err(_) => return std::process::ExitCode::from(2),
    };
    if phase(b"K1_OWNER_OPEN_OK\nK1_OWNER_FINISH_BEGIN\n", deadline).is_err() {
        return std::process::ExitCode::from(2);
    }
    if ManuallyDrop::into_inner(owner).finish().is_err() {
        return std::process::ExitCode::from(2);
    }
    if phase(b"K1_OWNER_FINISH_OK\n", deadline).is_err() {
        return std::process::ExitCode::from(2);
    }
    std::process::ExitCode::SUCCESS
}
