// SPDX-License-Identifier: MIT
fn main() {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    #[cfg(feature = "product-epochs")]
    if let [flag, uid] = arguments.as_slice()
        && flag == "--enroll-product"
    {
        match omavless_image_witness::enroll_product(uid) {
            Ok(()) => {
                println!("Product image enrollment completed.");
                return;
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        }
    }
    #[cfg(feature = "product-epochs")]
    if let [flag, uid] = arguments.as_slice()
        && flag == "--product-enrollment-seal"
    {
        admin_result(enrollment_seal_command(uid));
        return;
    }
    #[cfg(feature = "product-epochs")]
    if let [flag, uid, predecessor, core, runtime] = arguments.as_slice()
        && flag == "--upgrade-product"
    {
        admin_result(upgrade_command(uid, predecessor, core, runtime));
        return;
    }
    #[cfg(feature = "product-epochs")]
    if let [flag, uid, operation] = arguments.as_slice()
        && flag == "--inspect-product-upgrade"
    {
        admin_result(inspect_upgrade_command(uid, operation));
        return;
    }
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

#[cfg(feature = "product-epochs")]
fn admin_result(result: Result<(), omavless_image_witness::Error>) {
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

#[cfg(feature = "product-epochs")]
fn enrollment_seal_command(uid: &str) -> Result<(), omavless_image_witness::Error> {
    use std::io::Write;
    let seal = omavless_image_witness::enrollment_seal(uid)?;
    writeln!(std::io::stdout().lock(), "Product enrollment seal: {seal}")
        .map_err(|_| omavless_image_witness::Error::Unavailable)
}

#[cfg(feature = "product-epochs")]
fn upgrade_command(
    uid: &str,
    predecessor: &str,
    core: &str,
    runtime: &str,
) -> Result<(), omavless_image_witness::Error> {
    use std::io::Write;
    let request = omavless_image_witness::UpgradeRequest::parse(uid, predecessor, core, runtime)?;
    let prepared = omavless_image_witness::prepare_upgrade(request)?;
    let mut output = std::io::stdout().lock();
    // Output failure precedes every filesystem mutation. The ID is also a pure
    // function of the closed inputs, so a lost reply never requires a resend.
    writeln!(
        output,
        "Product image upgrade operation: {}",
        prepared.operation_id()
    )
    .and_then(|()| output.flush())
    .map_err(|_| omavless_image_witness::Error::Unavailable)?;
    prepared.commit()?;
    writeln!(output, "Product image upgrade committed.")
        .map_err(|_| omavless_image_witness::Error::Unavailable)
}

#[cfg(feature = "product-epochs")]
fn inspect_upgrade_command(
    uid: &str,
    operation: &str,
) -> Result<(), omavless_image_witness::Error> {
    use std::io::Write;
    let label = match omavless_image_witness::inspect_upgrade(uid, operation)? {
        omavless_image_witness::Inspection::NotRecorded => "not recorded",
        omavless_image_witness::Inspection::Incomplete => "retained incomplete",
        omavless_image_witness::Inspection::ObservedCommitted => "observed committed",
    };
    writeln!(std::io::stdout().lock(), "Product image upgrade: {label}")
        .map_err(|_| omavless_image_witness::Error::Unavailable)
}
