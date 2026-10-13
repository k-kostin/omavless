// SPDX-License-Identifier: MIT
//! SOURCE PROPOSAL ONLY. Not built/registered/launched. Review before native use.
//! Same public pinned host entry; one fixed readonly public-data extension.
use gpui_shell::{HostError, HostModule, HostValue};
use std::sync::Arc;
#[path = "../../theme_client.rs"]
mod theme_client;
#[path = "../../theme_environment.rs"]
mod theme_environment;
#[path = "../../theme_fixture.rs"]
mod theme_fixture;
#[path = "../../theme_source.rs"]
mod theme_source;

fn selected() -> Option<theme_client::PublicClient> {
    let selected = theme_client::PublicClient::select()?;
    if std::env::args_os().skip(1).collect::<Vec<_>>() != [selected.app.clone().into_os_string()] {
        return None;
    }
    // Same public identity function as grant_local_access. sdk-data was just
    // opened owned/no-symlink/empty, so its store descendants do not preexist.
    let _store = theme_environment::store_path(
        selected.app.parent()?,
        &gpui_shell::bundle_id_for_path(&selected.app),
    )?;
    Some(selected)
}

fn main() {
    let Some(selected) = selected() else {
        eprintln!("G1 owned public theme fixture unavailable");
        std::process::exit(2);
    };
    let sampler = Arc::clone(&selected.sampler);
    let module = HostModule::new("g1-synthetic-theme")
        .declarations("export function samplePalette(): Promise<{sequence:number,status:string,palette:Record<string,string>}>;")
        .async_function("samplePalette", move |arguments| {
            if !arguments.is_empty() { return Err(HostError::new("samplePalette takes no arguments")); }
            let permit = sampler.reserve().ok_or_else(|| HostError::new("Synthetic theme sample busy"))?;
            Ok(async move {
                let (sequence, prepared) = permit.sample().ok_or_else(|| HostError::new("Synthetic theme sequence exhausted"))?;
                let mut palette = vec![("mode".into(), HostValue::from(prepared.palette.mode()))];
                palette.extend(prepared.palette.fields().map(|(key, value)| (key.into(), HostValue::from(format!("#{value:06x}")))));
                Ok(HostValue::Object(vec![("sequence".into(), HostValue::Number(sequence as f64)),
                    ("status".into(), HostValue::from(prepared.status.name())), ("palette".into(), HostValue::Object(palette))]))
            })
        });
    if gpui_shell::export_module(module).is_err() {
        eprintln!("G1 synthetic theme extension unavailable");
        std::process::exit(2);
    }
    gpui_shell::host::main_with_components(gpui_shell::FrozenComponentRegistry::default());
    gpui_shell::clear_exported_modules();
}
