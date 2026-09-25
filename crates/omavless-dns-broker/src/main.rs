//! Explicit opt-in candidate only; no environment/path/config CLI overrides.
fn main() -> std::process::ExitCode {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--serve")) || args.next().is_some() {
        eprintln!("Usage: omavless-dns-broker --serve");
        return std::process::ExitCode::from(2);
    }
    match omavless_dns_broker::serve() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
