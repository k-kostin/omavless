//! Explicit opt-in candidate only; no environment/path/config CLI overrides.
fn main() -> std::process::ExitCode {
    let mut args = std::env::args_os().skip(1);
    let action = match (args.next(), args.next(), args.next()) {
        (Some(command), None, None) if command == "--serve" => {
            return match omavless_dns_broker::serve() {
                Ok(()) => std::process::ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("{error}");
                    std::process::ExitCode::FAILURE
                }
            };
        }
        (Some(command), Some(uid), None) if command == "--enroll" => {
            omavless_dns_broker::admin::Action::enroll(uid.as_os_str())
        }
        (Some(command), None, None) if command == "--revoke" => {
            Some(omavless_dns_broker::admin::Action::Revoke)
        }
        _ => None,
    };
    let Some(action) = action else {
        eprintln!("Usage: omavless-dns-broker --serve|--enroll UID|--revoke");
        return std::process::ExitCode::from(2);
    };
    match omavless_dns_broker::admin::run(action) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
