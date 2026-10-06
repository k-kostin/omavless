// SPDX-License-Identifier: MIT
//! Exact close-qualified build receipt. Decoding is data, NEVER a permit.
//! The original retained package/child/controller qualification is separate.
use serde::Deserialize;

pub(crate) const SCHEMA: &str = "omavless-managed-dns-close-pair-v1";
pub(crate) const K1_SCHEMA: &str = "omavless-managed-dns-k1-close-pair-v1";
pub(crate) const K1_PATCH: &str =
    "be7929ec02c71c57b65b050abb495315b33f2653184afabd30ec97b868d17cc4";
const K1_SOURCE: &str = "08194a275d315db7ca502e50f960c80af6dc163b";
pub(crate) const CLOSE_PATCH: &str =
    "0858827e1af00c3ed3196f021b0dbc76ce34a8de7aa7130d7085614d149acc8f";
const DNS_PATCH: &str = "d5ebe9d6b37f6b76599fc3c2dd25adbfb774ca0121beeb79c5768a9a08d7ff37";
const TUN_PATCH: &str = "2556c82aafbeb598a817d43042cf2069c6f209433c7a506395df581b4e31e2ab";
const MIHOMO: &str = "ab405bad5beeeac8b003bb01f60f134f6df54471";
const TUN: &str = "b50ae28a1409c7bce8e96e6c6966cf57d8ace754";
pub(crate) struct Hashes {
    pub core: [u8; 32],
    pub broker: [u8; 32],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: String,
    architecture: String,
    omavless_commit: String,
    mihomo_commit: String,
    mihomo_tag: String,
    sing_tun_commit: String,
    sing_tun_tag: String,
    patch_sha256: Patches,
    go_version: String,
    go_binary_sha256: String,
    cargo_lock_sha256: String,
    go_build_tags: String,
    go_dependency_mode: String,
    rustc_version: String,
    cargo_version: String,
    sha256: Members,
    package_flavor: String,
    broker_feature: String,
    conditional_close_abi: u8,
    go_cgo: bool,
    go_buildvcs: bool,
    #[serde(default, deserialize_with = "present_string")]
    managed_device: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    enrollment_policy: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    managed_device_source: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Patches {
    #[serde(rename = "mihomo-dns-broker.patch")]
    dns: String,
    #[serde(rename = "sing-tun-descriptor.patch")]
    tun: String,
    #[serde(rename = "mihomo-conditional-close.patch")]
    close: String,
    #[serde(
        rename = "mihomo-k1-device.patch",
        default,
        deserialize_with = "present_string"
    )]
    k1: Option<String>,
}
fn present_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    // Missing field may be None, but an explicit null/bool/number is invalid;
    // it cannot masquerade as the exact three-field schema.
    String::deserialize(d).map(Some)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Members {
    mihomo: String,
    #[serde(rename = "omavless-dns-broker")]
    broker: String,
    #[serde(rename = "corresponding-source.tar.xz")]
    source: String,
    #[serde(rename = "mihomo.LICENSE")]
    mihomo_license: String,
    #[serde(rename = "sing-tun.LICENSE")]
    tun_license: String,
    #[serde(rename = "omavless.LICENSE")]
    runtime_license: String,
}
fn hex(raw: &str, bytes: usize) -> bool {
    raw.len() == bytes * 2
        && raw
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && raw.bytes().any(|b| b != b'0')
}
fn hash(raw: &str) -> Result<[u8; 32], ()> {
    if !hex(raw, 32) {
        return Err(());
    }
    let mut result = [0; 32];
    for (out, pair) in result.iter_mut().zip(raw.as_bytes().as_chunks::<2>().0) {
        let digit = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
        *out = digit(pair[0]) * 16 + digit(pair[1]);
    }
    Ok(result)
}
fn tool(raw: &str, prefix: &str) -> bool {
    raw.starts_with(prefix) && raw.len() <= 128 && raw.bytes().all(|b| (32..=126).contains(&b))
}
fn go_tool(raw: &str, arch: &str) -> bool {
    let Some(version) = raw
        .strip_prefix("go version go1.")
        .and_then(|s| s.strip_suffix(&format!(" linux/{arch}")))
    else {
        return false;
    };
    let (version, suffix) = version
        .split_once('-')
        .map_or((version, None), |(v, s)| (v, Some(s)));
    let Some((minor, patch)) = version.split_once('.') else {
        return false;
    };
    [minor, patch]
        .iter()
        .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        && suffix.is_none_or(|s| {
            !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b':')
        })
        && raw.len() <= 128
}

pub(crate) fn decode(raw: &[u8], architecture: &str) -> Result<Hashes, ()> {
    if raw.is_empty() || raw.len() > 8192 {
        return Err(());
    }
    let goarch = match architecture {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => return Err(()),
    };
    let r: Receipt = serde_json::from_slice(raw).map_err(|_| ())?;
    let k1 = r.schema == K1_SCHEMA;
    if !matches!(r.schema.as_str(), SCHEMA | K1_SCHEMA)
        || r.architecture != architecture
        || !hex(&r.omavless_commit, 20)
        || r.mihomo_commit != MIHOMO
        || r.mihomo_tag != "v1.19.31"
        || r.sing_tun_commit != TUN
        || r.sing_tun_tag != "v0.4.24"
        || r.patch_sha256.dns != DNS_PATCH
        || r.patch_sha256.tun != TUN_PATCH
        || r.patch_sha256.close != CLOSE_PATCH
        || r.package_flavor
            != if k1 {
                "release-close-k1"
            } else {
                "release-close"
            }
        || r.broker_feature
            != if k1 {
                "k1-managed-device"
            } else {
                "release-package"
            }
        || r.conditional_close_abi != 1
        || r.go_cgo
        || r.go_buildvcs
        || r.go_build_tags
            != if k1 {
                "with_gvisor,omavless_k1_device"
            } else {
                "with_gvisor"
            }
        || r.go_dependency_mode != "vendor"
        || !go_tool(&r.go_version, goarch)
        || !tool(&r.rustc_version, "rustc ")
        || !tool(&r.cargo_version, "cargo ")
    {
        return Err(());
    }
    if k1 {
        if r.patch_sha256.k1.as_deref() != Some(K1_PATCH)
            || r.managed_device.as_deref() != Some("omavless0")
            || r.enrollment_policy.as_deref() != Some("omavless0-ipv4-development-v1")
            || r.managed_device_source.as_deref() != Some(K1_SOURCE)
        {
            return Err(());
        }
    } else if r.patch_sha256.k1.is_some()
        || r.managed_device.is_some()
        || r.enrollment_policy.is_some()
        || r.managed_device_source.is_some()
    {
        return Err(());
    }
    for value in [
        &r.go_binary_sha256,
        &r.cargo_lock_sha256,
        &r.sha256.source,
        &r.sha256.mihomo_license,
        &r.sha256.tun_license,
        &r.sha256.runtime_license,
    ] {
        hash(value)?;
    }
    Ok(Hashes {
        core: hash(&r.sha256.mihomo)?,
        broker: hash(&r.sha256.broker)?,
    })
}

#[cfg(test)]
pub(crate) fn fixture(architecture: &str) -> serde_json::Value {
    use serde_json::json;
    let goarch = if architecture == "x86_64" {
        "amd64"
    } else {
        "arm64"
    };
    json!({"schema":SCHEMA,"architecture":architecture,"omavless_commit":"a".repeat(40),
        "mihomo_commit":MIHOMO,"mihomo_tag":"v1.19.31","sing_tun_commit":TUN,"sing_tun_tag":"v0.4.24",
        "patch_sha256":{"mihomo-dns-broker.patch":DNS_PATCH,"sing-tun-descriptor.patch":TUN_PATCH,
            "mihomo-conditional-close.patch":CLOSE_PATCH},
        "go_version":format!("go version go1.27.0 linux/{goarch}"),"go_binary_sha256":"b".repeat(64),
        "cargo_lock_sha256":"c".repeat(64),"go_build_tags":"with_gvisor","go_dependency_mode":"vendor",
        "rustc_version":"rustc 1.98.1","cargo_version":"cargo 1.98.1","package_flavor":"release-close",
        "broker_feature":"release-package","conditional_close_abi":1,"go_cgo":false,"go_buildvcs":false,
        "sha256":{"mihomo":"1".repeat(64),"omavless-dns-broker":"2".repeat(64),
            "corresponding-source.tar.xz":"3".repeat(64),"mihomo.LICENSE":"4".repeat(64),
            "sing-tun.LICENSE":"5".repeat(64),"omavless.LICENSE":"6".repeat(64)}})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn four_family_requires_exact_source_overlay_tag_feature_device_and_new_consent_policy() {
        for arch in ["x86_64", "aarch64"] {
            let three = fixture(arch);
            let mut four = three.clone();
            four["schema"] = serde_json::json!(K1_SCHEMA);
            four["package_flavor"] = serde_json::json!("release-close-k1");
            four["go_build_tags"] = serde_json::json!("with_gvisor,omavless_k1_device");
            four["broker_feature"] = serde_json::json!("k1-managed-device");
            four["patch_sha256"]["mihomo-k1-device.patch"] = serde_json::json!(K1_PATCH);
            four["managed_device"] = serde_json::json!("omavless0");
            four["enrollment_policy"] = serde_json::json!("omavless0-ipv4-development-v1");
            four["managed_device_source"] = serde_json::json!(K1_SOURCE);
            assert!(decode(&serde_json::to_vec(&four).unwrap(), arch).is_ok());
            assert!(decode(&serde_json::to_vec(&three).unwrap(), arch).is_ok());
            for key in [
                "schema",
                "package_flavor",
                "go_build_tags",
                "broker_feature",
            ] {
                let mut mixed = four.clone();
                mixed[key] = three[key].clone();
                assert!(decode(&serde_json::to_vec(&mixed).unwrap(), arch).is_err());
            }
            for key in [
                "managed_device",
                "enrollment_policy",
                "managed_device_source",
            ] {
                let mut missing = four.clone();
                missing.as_object_mut().unwrap().remove(key);
                assert!(decode(&serde_json::to_vec(&missing).unwrap(), arch).is_err());
                let mut wrong = four.clone();
                wrong[key] = serde_json::json!("Meta");
                assert!(decode(&serde_json::to_vec(&wrong).unwrap(), arch).is_err());
                let mut extra = three.clone();
                extra[key] = serde_json::Value::Null;
                assert!(decode(&serde_json::to_vec(&extra).unwrap(), arch).is_err());
            }
            for bad in [
                serde_json::Value::Null,
                serde_json::json!(false),
                serde_json::json!(DNS_PATCH),
            ] {
                let mut changed = four.clone();
                changed["patch_sha256"]["mihomo-k1-device.patch"] = bad;
                assert!(decode(&serde_json::to_vec(&changed).unwrap(), arch).is_err());
                let mut extended = three.clone();
                extended["patch_sha256"]["mihomo-k1-device.patch"] = serde_json::Value::Null;
                assert!(decode(&serde_json::to_vec(&extended).unwrap(), arch).is_err());
            }
            let mut missing_patch = four.clone();
            missing_patch["patch_sha256"]
                .as_object_mut()
                .unwrap()
                .remove("mihomo-k1-device.patch");
            assert!(decode(&serde_json::to_vec(&missing_patch).unwrap(), arch).is_err());
            let raw = serde_json::to_string(&four).unwrap();
            let duplicate = raw.replace(
                "\"mihomo-k1-device.patch\":",
                "\"mihomo-k1-device.patch\":\"invalid\",\"mihomo-k1-device.patch\":",
            );
            assert!(decode(duplicate.as_bytes(), arch).is_err());
            let mut old = four;
            old["enrollment_policy"] = serde_json::json!("meta-ipv4-release-v1");
            assert!(decode(&serde_json::to_vec(&old).unwrap(), arch).is_err());
        }
    }
    #[test]
    fn exact_family_requires_all_three_patches_abi_and_native_architecture() {
        for arch in ["x86_64", "aarch64"] {
            let value = fixture(arch);
            assert_eq!(
                decode(&serde_json::to_vec(&value).unwrap(), arch)
                    .unwrap()
                    .core,
                [0x11; 32]
            );
            for (key, bad) in [
                ("schema", serde_json::json!(1)),
                ("conditional_close_abi", serde_json::json!(true)),
                ("conditional_close_abi", serde_json::json!(2)),
                ("package_flavor", serde_json::json!("release")),
                ("go_cgo", serde_json::json!(true)),
                ("go_buildvcs", serde_json::json!(true)),
                ("architecture", serde_json::json!("foreign")),
            ] {
                let mut changed = value.clone();
                changed[key] = bad;
                assert!(decode(&serde_json::to_vec(&changed).unwrap(), arch).is_err());
            }
            let mut old = value;
            old["patch_sha256"]
                .as_object_mut()
                .unwrap()
                .remove("mihomo-conditional-close.patch");
            assert!(decode(&serde_json::to_vec(&old).unwrap(), arch).is_err());
        }
    }
    #[test]
    fn duplicate_unknown_and_unproven_developer_promotion_are_not_receipts() {
        let value = fixture("x86_64");
        let raw = serde_json::to_string(&value).unwrap();
        for changed in [raw.replacen('{', "{\"schema\":\"foreign\",", 1),
            raw.replace("\"conditional_close_abi\":1", "\"conditional_close_abi\":1,\"conditional_close_abi\":1"),
            raw.replace("\"mihomo-conditional-close.patch\":", "\"mihomo-conditional-close.patch\":\"invalid\",\"mihomo-conditional-close.patch\":"),
            raw.replacen('{', "{\"production_adoption\":true,", 1)] {
            assert!(decode(changed.as_bytes(), "x86_64").is_err());
        }
        for key in ["schema", "broker_feature", "go_binary_sha256", "sha256"] {
            let mut missing = value.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(decode(&serde_json::to_vec(&missing).unwrap(), "x86_64").is_err());
        }
        assert!(decode(&vec![b' '; 8193], "x86_64").is_err());
    }
}
