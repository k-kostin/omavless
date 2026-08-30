// SPDX-License-Identifier: MIT

use omavless_control_protocol::{
    API_VERSION, FrameKind, encode_request, make_request, read_unary_frame,
};
use serde_json::json;
use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

const USAGE: &str = "Usage: omavless-control-protocol-probe hello|request|response";

fn run() -> Result<(), &'static str> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments == ["-h"] || arguments == ["--help"] {
        println!("{USAGE}");
        return Ok(());
    }
    if arguments.len() != 1 {
        return Err("Invalid probe command. Use --help.");
    }
    if arguments[0] == "hello" {
        let request = make_request(
            "hello-1",
            "system.hello",
            json!({"versions": [API_VERSION]}),
        )
        .map_err(|_| "Probe validation failed")?;
        let frame = encode_request(&request).map_err(|_| "Probe validation failed")?;
        io::stdout()
            .write_all(&frame)
            .map_err(|_| "Probe I/O failed")?;
        return Ok(());
    }

    let (kind, label) = if arguments[0] == "request" {
        (FrameKind::Request, "request")
    } else if arguments[0] == "response" {
        (FrameKind::Response, "response")
    } else {
        return Err("Invalid probe command. Use --help.");
    };
    read_unary_frame(&mut io::stdin().lock(), kind).map_err(|error| error.code().message())?;
    println!("VALID {label}");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}
