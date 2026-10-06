// SPDX-License-Identifier: MIT
fn main() {
    if std::env::args().collect::<Vec<_>>()
        != [
            std::env::args().next().unwrap_or_default(),
            "--development-service".into(),
        ]
    {
        eprintln!("Development image witness invocation refused.");
        std::process::exit(2);
    }
    match omavless_image_witness::serve_development() {
        Ok(()) => (),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2)
        }
    }
}
