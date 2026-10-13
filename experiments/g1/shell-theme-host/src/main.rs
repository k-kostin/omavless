// SPDX-License-Identifier: MIT
//! SOURCE PROPOSAL ONLY. Not built/registered/launched. Review before native use.
//! Same public pinned host entry; one fixed readonly public-data extension.
use gpui_shell::{HostError, HostModule, HostValue};
use std::os::unix::fs::MetadataExt;
use std::{fs, path::PathBuf, sync::Arc};
#[path = "../../theme_fixture.rs"]
mod theme_fixture;
#[path = "../../theme_source.rs"]
mod theme_source;

fn selected() -> Option<Arc<theme_fixture::Sampler>> {
    // Fresh synthetic process HOME prevents the SDK's initial ambient
    // theme/storage paths from reaching the real user's directories.
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let uid = fs::metadata("/proc/self").ok()?.uid();
    if uid == 0
        || !home.is_absolute()
        || !home.file_name()?.to_str()?.starts_with("g1h-")
        || home.parent()?.file_name()? != "ovt"
        || home.parent()?.parent()?.file_name()? != ".cache"
    {
        return None;
    }
    for directory in [
        &home,
        &home.join(".cache"),
        &home.join(".cache/ovt"),
        &home.join("app"),
    ] {
        let metadata = fs::symlink_metadata(directory).ok()?;
        if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o777 != 0o700 {
            return None;
        }
    }
    if fs::symlink_metadata(home.join(".config")).is_ok()
        || fs::symlink_metadata(home.join(".local")).is_ok()
    {
        return None;
    }
    let app = home.join("app");
    if std::env::args_os().skip(1).collect::<Vec<_>>() != [app.clone().into_os_string()] {
        return None;
    }
    // Exact existing manifest. No other manifest can request generic grants
    // from the supported host CLI. The bounded read is rooted in a held FD.
    if !theme_fixture::OwnedFixture::open_root(&app)
        .ok()?
        .has_trial_manifest()
    {
        return None;
    }
    let fixture = theme_fixture::OwnedFixture::open_root(&home.join(".cache/ovt/g1-theme")).ok()?;
    Some(theme_fixture::Sampler::new(fixture))
}

fn main() {
    let Some(sampler) = selected() else {
        eprintln!("G1 owned public theme fixture unavailable");
        std::process::exit(2);
    };
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
