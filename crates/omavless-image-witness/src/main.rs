// SPDX-License-Identifier: MIT
fn main() {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let serve = match arguments.as_slice() {
        [flag] if flag == "--development-service" => omavless_image_witness::serve_development,
        [flag] if flag == "--development-runtime-service" => {
            omavless_image_witness::serve_development_runtime
        }
        #[cfg(feature = "product-epochs")]
        [flag] if flag == "--product-epoch-service" => omavless_image_witness::serve_product_epochs,
        _ => {
            eprintln!("Development image witness invocation refused.");
            std::process::exit(2);
        }
    };
    match serve() {
        Ok(()) => (),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2)
        }
    }
}
