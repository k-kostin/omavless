// SPDX-License-Identifier: MIT

use omavless_parity::{MAX_REPORT_BYTES, compare_reports, parse_report};
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn read_report(path: &Path) -> Result<omavless_parity::Report, ()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ())?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(());
    }
    let length = usize::try_from(metadata.len()).map_err(|_| ())?;
    if length > MAX_REPORT_BYTES {
        return Err(());
    }
    let input = fs::read(path).map_err(|_| ())?;
    parse_report(&input).map_err(|_| ())
}

fn run() -> Result<bool, ()> {
    let arguments: Vec<_> = env::args_os().collect();
    if arguments.len() != 4 || arguments[1] != "compare" {
        return Err(());
    }
    let reference = read_report(Path::new(&arguments[2]))?;
    let candidate = read_report(Path::new(&arguments[3]))?;
    let summary = compare_reports(&reference, &candidate).map_err(|_| ())?;
    let output = serde_json::to_string(&summary).map_err(|_| ())?;
    println!("{output}");
    Ok(summary.matched)
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(()) => {
            eprintln!("usage: omavless-parity compare REFERENCE.json CANDIDATE.json");
            ExitCode::from(2)
        }
    }
}
