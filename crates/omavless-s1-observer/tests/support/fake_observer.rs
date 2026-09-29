// SPDX-License-Identifier: MIT

// Compiled by runner unit tests into a private temporary directory. The
// synthetic payload file contains no host observations or credentials.
use std::io::Write;
use std::process::{Command, Stdio};

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("valid") => {
            let bytes = std::fs::read(args.next().unwrap()).unwrap();
            std::io::stdout().write_all(&bytes).unwrap();
        }
        Some("malformed") => std::io::stdout().write_all(b"invalid frame").unwrap(),
        Some("oversized-stdout") => {
            std::io::stdout().write_all(&vec![b'x'; 100_000]).unwrap();
        }
        Some("oversized-stderr") => {
            std::io::stderr().write_all(&vec![b's'; 100_000]).unwrap();
        }
        Some("failed") => std::process::exit(7),
        Some("sleep") => std::thread::sleep(std::time::Duration::from_secs(2)),
        Some("pipe-holder") => {
            // The direct child exits, but its descendant inherits both pipes.
            // The parent runner must remain bounded and terminate the group.
            Command::new(std::env::current_exe().unwrap())
                .arg("sleep")
                .stdin(Stdio::null())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap();
        }
        _ => std::process::exit(8),
    }
}
