// SPDX-License-Identifier: MIT
//! Synthetic filesystem/capture controls only; never executes a validator.
use super::*;
use crate::isolated_validation::{BUNDLES, RESOURCE_LIMIT, ValidationSnapshot};
use omavless_domain::private_store::{parse_candidate_private_store, parse_private_store};
use std::fs::{self, Metadata, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt, symlink};
use std::path::PathBuf;

const ID: &str = "00000000-0000-4000-8000-000000000001";
const LABEL: &str = "SYNTHETIC_PRIVATE_CAPTURE_CANARY";
const PAYLOAD: &[u8] = b"synthetic MRS bytes, not validated MRS content";

fn document(version: u8) -> String {
    serde_json::json!({"version":version,"profiles":[
        {"id":ID,"name":LABEL,"protocol":"vless",
         "uri":"vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp"}],
        "subscriptions":[],"activeId":"","lastId":ID,
        "startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"}}).to_string()
}

struct Cache(PathBuf);
impl Cache {
    fn new(bundle: &str) -> Self {
        // This older #756 lineage has the existing exclusive directory helper
        // only. The selected test recipe supplies a verified private HOME TMP.
        // No pathname socket is bound by these resource-only fixtures.
        let root = crate::test_temp::directory("p4-capture").unwrap();
        fs::DirBuilder::new()
            .mode(0o700)
            .create(root.join("ruleset"))
            .unwrap();
        for name in manifest(bundle).unwrap() {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(root.join("ruleset").join(name))
                .unwrap();
            file.write_all(PAYLOAD).unwrap();
        }
        Self(root)
    }
    fn uid(&self) -> u32 {
        nix::unistd::Uid::current().as_raw()
    }
}
impl Drop for Cache {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn shared_fd_capture_has_complete_bundle_membership_and_owned_immutable_bytes() {
    for (bundle, count) in BUNDLES.iter().zip([23, 5, 7]) {
        let cache = Cache::new(bundle);
        fs::write(
            cache.0.join("ruleset/extra-not-admitted.mrs"),
            b"not adopted",
        )
        .unwrap();
        for mode in ["rule", "global", "direct"] {
            let template = bundle.replace("\nmode: rule\n", &format!("\nmode: {mode}\n"));
            let names = manifest(&template).unwrap();
            let resources = capture_resources(&cache.0, cache.uid(), names.clone()).unwrap();
            assert_eq!(resources.len(), count);
            assert!(resources.iter().map(|(name, _)| name).eq(names.iter()));
            assert!(resources.iter().all(|(_, bytes)| bytes == PAYLOAD));
        }
        let names = manifest(bundle).unwrap();
        let captured = capture_resources(&cache.0, cache.uid(), names.clone()).unwrap();
        fs::write(
            cache.0.join("ruleset").join(&names[0]),
            b"changed after capture",
        )
        .unwrap();
        assert!(captured[0].1 == PAYLOAD);
    }
}

#[test]
fn candidate_refusals_finish_before_any_resource_acquisition() {
    let candidate = parse_candidate_private_store(&document(4)).unwrap();
    let data = Path::new("/synthetic-do-not-read");
    for desired in [
        DesiredState {
            connected: true,
            profile_id: ID.into(),
            ..DesiredState::default()
        },
        DesiredState {
            profile_id: ID.into(),
            ..DesiredState::default()
        },
        DesiredState {
            schema_version: 0,
            ..DesiredState::default()
        },
    ] {
        assert_eq!(
            CandidateValidationSnapshot::capture(
                data,
                0,
                &desired,
                &candidate,
                ID,
                BUNDLES[0],
                &[]
            )
            .err()
            .unwrap(),
            CandidateCaptureError::Preparation(PreparationError::InvalidOffIntent)
        );
    }
    assert_eq!(
        CandidateValidationSnapshot::capture(
            data,
            0,
            &DesiredState::default(),
            &candidate,
            ID,
            BUNDLES[0],
            &[]
        )
        .err()
        .unwrap(),
        CandidateCaptureError::Preparation(PreparationError::UnsupportedCore)
    );
    let mut active: serde_json::Value = serde_json::from_str(&document(4)).unwrap();
    active["activeId"] = ID.into();
    let active = parse_candidate_private_store(&active.to_string()).unwrap();
    assert_eq!(
        CandidateValidationSnapshot::capture(
            data,
            0,
            &DesiredState::default(),
            &active,
            ID,
            BUNDLES[0],
            &[]
        )
        .err()
        .unwrap(),
        CandidateCaptureError::Preparation(PreparationError::ActiveCandidate)
    );
}

#[test]
fn arbitrary_template_or_prerendered_config_never_selects_a_resource_manifest() {
    let candidate = parse_candidate_private_store(&document(4)).unwrap();
    let desired = DesiredState::default();
    let rendered = candidate
        .prepare_private_config_mode(ID, BUNDLES[0], "/work/mihomo.sock", "rule")
        .unwrap();
    let rendered = std::str::from_utf8(rendered.expose_private_bytes()).unwrap();
    for template in [
        rendered,
        "mode: rule\nproxies: []\n",
        "private malformed template",
    ] {
        assert_eq!(
            CandidateValidationSnapshot::capture(
                Path::new("/not-read"),
                0,
                &desired,
                &candidate,
                ID,
                template,
                &[]
            )
            .err()
            .unwrap(),
            CandidateCaptureError::Resources(ValidationError::UnsupportedTemplate)
        );
    }
    for bundle in BUNDLES {
        let modified = format!("{bundle}\n");
        assert_eq!(
            CandidateValidationSnapshot::capture(
                Path::new("/not-read"),
                0,
                &desired,
                &candidate,
                ID,
                &modified,
                &[]
            )
            .err()
            .unwrap(),
            CandidateCaptureError::Resources(ValidationError::UnsupportedTemplate)
        );
    }
}

#[test]
fn resource_capture_preserves_missing_symlink_owner_mode_and_size_refusals() {
    for fault in [
        "missing",
        "empty",
        "link",
        "writable",
        "oversize",
        "wrong-uid",
        "directory-link",
        "directory-writable",
    ] {
        let cache = Cache::new(BUNDLES[1]);
        let names = manifest(BUNDLES[1]).unwrap();
        let path = cache.0.join("ruleset").join(&names[0]);
        match fault {
            "missing" => fs::remove_file(&path).unwrap(),
            "empty" => fs::write(&path, b"").unwrap(),
            "link" => {
                fs::remove_file(&path).unwrap();
                symlink("missing", &path).unwrap();
            }
            "writable" => fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap(),
            "oversize" => OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap()
                .set_len(RESOURCE_LIMIT as u64 + 1)
                .unwrap(),
            "directory-link" => {
                fs::rename(cache.0.join("ruleset"), cache.0.join("held-rules")).unwrap();
                symlink("held-rules", cache.0.join("ruleset")).unwrap();
            }
            "directory-writable" => {
                fs::set_permissions(cache.0.join("ruleset"), fs::Permissions::from_mode(0o777))
                    .unwrap()
            }
            _ => {}
        }
        let uid = if fault == "wrong-uid" {
            cache.uid().wrapping_add(1)
        } else {
            cache.uid()
        };
        let result = capture_resources(&cache.0, uid, names);
        assert!(matches!(
            result,
            Err(ValidationError::UnsafeInput | ValidationError::ResourceUnavailable)
        ));
    }
}

#[test]
fn mandatory_resource_total_bound_is_not_replaced_by_per_file_limits() {
    let cache = Cache::new(BUNDLES[0]);
    let names = manifest(BUNDLES[0]).unwrap();
    for name in names.iter().take(9) {
        OpenOptions::new()
            .write(true)
            .open(cache.0.join("ruleset").join(name))
            .unwrap()
            .set_len(RESOURCE_LIMIT as u64)
            .unwrap();
    }
    assert_eq!(
        capture_resources(&cache.0, cache.uid(), names)
            .err()
            .unwrap(),
        ValidationError::ResourceUnavailable
    );
}

#[test]
fn v3_capture_order_and_private_rendering_are_unchanged_by_resource_factor() {
    let store = parse_private_store(&document(3)).unwrap();
    let cache = Cache::new(BUNDLES[1]);
    let missing = cache.0.join("missing-data");
    assert_eq!(
        ValidationSnapshot::capture(
            &missing,
            cache.uid(),
            &DesiredState::default(),
            &store,
            "custom template"
        )
        .err()
        .unwrap(),
        ValidationError::UnsupportedTemplate
    );
    assert_eq!(
        ValidationSnapshot::capture(
            &missing,
            cache.uid(),
            &DesiredState::default(),
            &store,
            BUNDLES[1]
        )
        .err()
        .unwrap(),
        ValidationError::UnsafeInput
    );
    for mode in [RoutingMode::Rule, RoutingMode::Global, RoutingMode::Direct] {
        let desired = DesiredState {
            connected: true,
            profile_id: ID.into(),
            mode,
            ..DesiredState::default()
        };
        let legacy =
            ValidationSnapshot::capture(&cache.0, cache.uid(), &desired, &store, BUNDLES[1])
                .unwrap();
        let expected = store
            .prepare_config_mode(ID, BUNDLES[1], "/work/mihomo.sock", mode.as_str())
            .unwrap();
        assert!(legacy.config == expected);
        assert!(legacy.resources.iter().all(|(_, bytes)| bytes == PAYLOAD));
    }
}

#[test]
fn fixed_capture_errors_never_release_private_input_fragments() {
    for error in [
        CandidateCaptureError::Preparation(PreparationError::InvalidOffIntent),
        CandidateCaptureError::Preparation(PreparationError::UnsupportedCore),
        CandidateCaptureError::Resources(ValidationError::UnsupportedTemplate),
        CandidateCaptureError::Resources(ValidationError::ResourceUnavailable),
    ] {
        let output = format!("{error:?} {error}");
        for private in [LABEL, "synthetic-do-not-read", "vless://", "192.0.2.1"] {
            assert!(!output.contains(private));
        }
    }
}

#[test]
#[ignore = "separately admitted exact public core DATA; capture only, no execution"]
fn authentic_candidate_capture_data_only_optin() {
    use sha2::{Digest, Sha256};
    let path = std::env::var_os("OMAVLESS_TEST_CANDIDATE_CORE_DATA")
        .expect("explicit public DATA is required");
    let path = PathBuf::from(path);
    let before = fs::symlink_metadata(&path).expect("selected DATA metadata is required");
    let uid = nix::unistd::Uid::current().as_raw();
    assert!(before.is_file());
    assert_eq!(before.uid(), uid);
    assert_eq!(before.mode() & 0o7777, 0o400);
    assert_eq!(before.nlink(), 1);
    assert_eq!(before.len(), 61_886_168);
    let core =
        crate::isolated_validation::read_bounded(&path, uid, 128 * 1024 * 1024, false).unwrap();
    let after = fs::symlink_metadata(&path).unwrap();
    let stable = |metadata: &Metadata| {
        (
            metadata.dev(),
            metadata.ino(),
            metadata.mode(),
            metadata.uid(),
            metadata.gid(),
            metadata.nlink(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        )
    };
    assert!(stable(&before) == stable(&after));
    assert_eq!(
        format!("{:x}", Sha256::digest(&core)),
        "ba7a74ed6bbc3098930e6e172fa9b7d4ef7e42a77f7a516046d61fbeb3bfcfe6"
    );
    let candidate = parse_candidate_private_store(&document(4)).unwrap();
    let mut captured = 0;
    for (bundle, count) in BUNDLES.iter().zip([23, 5, 7]) {
        let cache = Cache::new(bundle);
        for mode in [RoutingMode::Rule, RoutingMode::Global, RoutingMode::Direct] {
            let desired = DesiredState {
                generation: 7,
                mode,
                ..DesiredState::default()
            };
            // Keep existing canonical mode semantics: a rule-line bundle can
            // render global/direct without a stricter new raw-template policy.
            let snapshot = CandidateValidationSnapshot::capture(
                &cache.0, uid, &desired, &candidate, ID, bundle, &core,
            )
            .unwrap();
            let expected = candidate
                .prepare_private_config_mode(ID, bundle, "/work/mihomo.sock", mode.as_str())
                .unwrap();
            assert!(snapshot.expose_private_config() == expected.expose_private_bytes());
            assert!(snapshot.expose_captured_core_bytes() == core);
            assert_eq!(snapshot.expose_captured_template(), *bundle);
            assert_eq!(snapshot.expose_private_resources().len(), count);
            assert!(
                snapshot
                    .expose_private_resources()
                    .iter()
                    .all(|(_, bytes)| bytes == PAYLOAD)
            );
            assert_eq!(snapshot.mode(), mode);
            assert_eq!(snapshot.intent_generation(), 7);
            assert_eq!(
                format!("{snapshot:?}"),
                "CandidateValidationSnapshot([REDACTED])"
            );
            captured += 1;
        }
    }
    assert_eq!(captured, 9);
    eprintln!(
        "candidate capture DATA: captured={captured}; synthetic MRS; validator_executed=false"
    );
}
