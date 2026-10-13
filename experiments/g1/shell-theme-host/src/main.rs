// SPDX-License-Identifier: MIT
//! SOURCE PROPOSAL ONLY. Not built/registered/launched. Review before native use.
//! Same public pinned host entry; one fixed readonly public-data extension.
use gpui_shell::{HostError, HostModule, HostValue};
use std::os::unix::fs::MetadataExt;
use std::{fs, path::PathBuf, sync::Arc};
#[path = "../../theme_environment.rs"]
mod theme_environment;
#[path = "../../theme_fixture.rs"]
mod theme_fixture;
#[path = "../../theme_source.rs"]
mod theme_source;

struct Selected {
    sampler: Arc<theme_fixture::Sampler>,
    // Hold selected scope identities for the entire SDK run. The sole public
    // fixture writer must not replace these roots while the host is active;
    // this is not a hostile same-UID sandbox around the SDK's path-based I/O.
    _scopes: Vec<theme_fixture::OwnedFixture>,
}

fn selected() -> Option<Selected> {
    // Fresh synthetic process HOME prevents the SDK's initial ambient
    // theme/storage paths from reaching the real user's directories.
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let uid = fs::metadata("/proc/self").ok()?.uid();
    if uid == 0
        || !home.is_absolute()
        || !home.file_name()?.to_str()?.starts_with("g1h-")
        || home.parent()?.file_name()? != "ovt"
        || home.parent()?.parent()?.file_name()? != ".cache"
        || fs::canonicalize(&home).ok()? != home
        || !theme_environment::admitted(&home, |key| std::env::var_os(key))
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
    let mut scopes = vec![theme_fixture::OwnedFixture::open_root(&home).ok()?];
    for child in ["sdk-data", "sdk-config", "sdk-cache", "sdk-state"] {
        let directory = home.join(child);
        scopes.push(theme_fixture::OwnedFixture::open_empty_root(&directory).ok()?);
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
    // Exact existing manifest prevents additional manifest-requested grants.
    // The supported CLI STILL adds app/store reads, store writes and exit even
    // with storage:false. These inherited grants await explicit approval.
    // All store/config XDG destinations above are fresh owned HOME children.
    let app_scope = theme_fixture::OwnedFixture::open_root(&app).ok()?;
    if !app_scope.has_trial_manifest() {
        return None;
    }
    scopes.push(app_scope);
    // Same public identity function as grant_local_access. sdk-data was just
    // opened owned/no-symlink/empty, so its store descendants do not preexist.
    let _store = theme_environment::store_path(&home, &gpui_shell::bundle_id_for_path(&app))?;
    let fixture = theme_fixture::OwnedFixture::open_root(&home.join(".cache/ovt/g1-theme")).ok()?;
    Some(Selected {
        sampler: theme_fixture::Sampler::new(fixture),
        _scopes: scopes,
    })
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
