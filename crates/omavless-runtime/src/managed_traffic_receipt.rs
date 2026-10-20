// SPDX-License-Identifier: MIT
//! Exact normal-release traffic-readiness receipt. Decoding is data, not a
//! runtime-readiness witness, artifact authentication or close authorization.
//! Actual retained package/file identity and the live read-only API are separate.
use serde::Deserialize;

pub(crate) const SCHEMA: &str = "omavless-managed-dns-traffic-ready-pair-v1";
pub(crate) const TRAFFIC_PATCH: &str =
    "e9a2f1ea78046b6f2cfafe574bc2d989a8381d57d3464a0aff2d71507424beb1";
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
    #[serde(deserialize_with = "object")]
    patch_sha256: Patches,
    go_version: String,
    go_binary_sha256: String,
    cargo_lock_sha256: String,
    go_build_tags: String,
    go_dependency_mode: String,
    rustc_version: String,
    cargo_version: String,
    #[serde(deserialize_with = "object")]
    sha256: Members,
    package_flavor: String,
    broker_feature: String,
    traffic_ready_abi: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Patches {
    #[serde(rename = "mihomo-dns-broker.patch")]
    dns: String,
    #[serde(rename = "sing-tun-descriptor.patch")]
    tun: String,
    #[serde(rename = "mihomo-traffic-ready.patch")]
    traffic: String,
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

fn object<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    // serde_json's struct decoder also accepts positional arrays. Require a
    // JSON object first, then retain derived field/type/duplicate validation.
    struct Object<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Object<T> {
        type Value = T;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a receipt object")
        }

        fn visit_map<M: serde::de::MapAccess<'de>>(self, map: M) -> Result<T, M::Error> {
            T::deserialize(serde::de::value::MapAccessDeserializer::new(map))
        }
    }
    deserializer.deserialize_map(Object(std::marker::PhantomData))
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
    // Object-only streaming deserialization at all three depths preserves
    // duplicate/type checks. Parsing to Value first could erase duplicates.
    let mut deserializer = serde_json::Deserializer::from_slice(raw);
    let r: Receipt = object(&mut deserializer).map_err(|_| ())?;
    deserializer.end().map_err(|_| ())?;
    if r.schema != SCHEMA
        || r.architecture != architecture
        || !hex(&r.omavless_commit, 20)
        || r.mihomo_commit != MIHOMO
        || r.mihomo_tag != "v1.19.31"
        || r.sing_tun_commit != TUN
        || r.sing_tun_tag != "v0.4.24"
        || r.patch_sha256.dns != DNS_PATCH
        || r.patch_sha256.tun != TUN_PATCH
        || r.patch_sha256.traffic != TRAFFIC_PATCH
        || r.package_flavor != "release"
        || r.broker_feature != "release-package"
        || r.traffic_ready_abi != 1
        || r.go_build_tags != "with_gvisor"
        || r.go_dependency_mode != "vendor"
        || !go_tool(&r.go_version, goarch)
        || !tool(&r.rustc_version, "rustc ")
        || !tool(&r.cargo_version, "cargo ")
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
            "mihomo-traffic-ready.patch":TRAFFIC_PATCH},
        "go_version":format!("go version go1.27.0 linux/{goarch}"),"go_binary_sha256":"b".repeat(64),
        "cargo_lock_sha256":"c".repeat(64),"go_build_tags":"with_gvisor","go_dependency_mode":"vendor",
        "rustc_version":"rustc 1.98.1","cargo_version":"cargo 1.98.1","package_flavor":"release",
        "broker_feature":"release-package","traffic_ready_abi":1,
        "sha256":{"mihomo":"1".repeat(64),"omavless-dns-broker":"2".repeat(64),
            "corresponding-source.tar.xz":"3".repeat(64),"mihomo.LICENSE":"4".repeat(64),
            "sing-tun.LICENSE":"5".repeat(64),"omavless.LICENSE":"6".repeat(64)}})
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn rejected(value: &Value, arch: &str) {
        assert!(decode(&serde_json::to_vec(value).unwrap(), arch).is_err());
    }

    #[test]
    fn exact_three_patch_normal_release_is_accepted_on_both_architectures() {
        for arch in ["x86_64", "aarch64"] {
            let hashes = decode(&serde_json::to_vec(&fixture(arch)).unwrap(), arch).unwrap();
            assert_eq!(hashes.core, [0x11; 32]);
            assert_eq!(hashes.broker, [0x22; 32]);
        }
    }

    #[test]
    fn architecture_and_go_target_must_match_the_native_request() {
        for arch in ["x86_64", "aarch64"] {
            let value = fixture(arch);
            let foreign = if arch == "x86_64" {
                "aarch64"
            } else {
                "x86_64"
            };
            rejected(&value, foreign);
            for bad in ["", "amd64", "arm64", "riscv64", "foreign"] {
                rejected(&value, bad);
            }
            let mut changed = value;
            changed["go_version"] = fixture(foreign)["go_version"].clone();
            rejected(&changed, arch);
        }
    }

    #[test]
    fn every_top_level_field_is_required_and_wrong_types_are_rejected() {
        for arch in ["x86_64", "aarch64"] {
            let value = fixture(arch);
            for key in value.as_object().unwrap().keys() {
                let mut missing = value.clone();
                missing.as_object_mut().unwrap().remove(key);
                rejected(&missing, arch);
                for bad in [Value::Null, json!(true), json!([]), json!({})] {
                    let mut changed = value.clone();
                    changed[key] = bad;
                    rejected(&changed, arch);
                }
                if key != "traffic_ready_abi" {
                    let mut changed = value.clone();
                    changed[key] = json!(1);
                    rejected(&changed, arch);
                }
            }
        }
    }

    #[test]
    fn abi_requires_exact_integer_one_not_bool_float_string_or_overflow() {
        for arch in ["x86_64", "aarch64"] {
            for bad in [
                json!(0),
                json!(2),
                json!(-1),
                json!(256),
                json!(true),
                json!("1"),
                json!(1.0),
            ] {
                let mut changed = fixture(arch);
                changed["traffic_ready_abi"] = bad;
                rejected(&changed, arch);
            }
        }
    }

    #[test]
    fn both_nested_structs_require_all_fields_and_string_types() {
        for arch in ["x86_64", "aarch64"] {
            let value = fixture(arch);
            for group in ["patch_sha256", "sha256"] {
                for key in value[group].as_object().unwrap().keys() {
                    let mut missing = value.clone();
                    missing[group].as_object_mut().unwrap().remove(key);
                    rejected(&missing, arch);
                    for bad in [Value::Null, json!(1), json!(true), json!([]), json!({})] {
                        let mut changed = value.clone();
                        changed[group][key] = bad;
                        rejected(&changed, arch);
                    }
                }
            }
        }
    }

    #[test]
    fn full_positional_arrays_cannot_replace_any_of_the_three_objects() {
        // Declaration order, NOT map sorting: each array is otherwise a complete
        // valid serde struct sequence. Empty-array negatives do not cover this.
        const ROOT: &[&str] = &[
            "schema",
            "architecture",
            "omavless_commit",
            "mihomo_commit",
            "mihomo_tag",
            "sing_tun_commit",
            "sing_tun_tag",
            "patch_sha256",
            "go_version",
            "go_binary_sha256",
            "cargo_lock_sha256",
            "go_build_tags",
            "go_dependency_mode",
            "rustc_version",
            "cargo_version",
            "sha256",
            "package_flavor",
            "broker_feature",
            "traffic_ready_abi",
        ];
        const PATCHES: &[&str] = &[
            "mihomo-dns-broker.patch",
            "sing-tun-descriptor.patch",
            "mihomo-traffic-ready.patch",
        ];
        const MEMBERS: &[&str] = &[
            "mihomo",
            "omavless-dns-broker",
            "corresponding-source.tar.xz",
            "mihomo.LICENSE",
            "sing-tun.LICENSE",
            "omavless.LICENSE",
        ];
        let positional = |value: &Value, keys: &[&str]| {
            Value::Array(keys.iter().map(|key| value[*key].clone()).collect())
        };
        let mut accepted = Vec::new();
        for arch in ["x86_64", "aarch64"] {
            let value = fixture(arch);
            assert!(decode(&serde_json::to_vec(&value).unwrap(), arch).is_ok());
            if decode(
                &serde_json::to_vec(&positional(&value, ROOT)).unwrap(),
                arch,
            )
            .is_ok()
            {
                accepted.push(format!("{arch}/root"));
            }
            for (group, keys) in [("patch_sha256", PATCHES), ("sha256", MEMBERS)] {
                let mut changed = value.clone();
                changed[group] = positional(&value[group], keys);
                if decode(&serde_json::to_vec(&changed).unwrap(), arch).is_ok() {
                    accepted.push(format!("{arch}/{group}"));
                }
            }
        }
        assert!(
            accepted.is_empty(),
            "positional arrays admitted: {accepted:?}"
        );
    }

    #[test]
    fn extra_fields_at_every_depth_are_refused_even_when_null() {
        for arch in ["x86_64", "aarch64"] {
            for group in [None, Some("patch_sha256"), Some("sha256")] {
                for bad in [Value::Null, json!(false), json!("unknown")] {
                    let mut changed = fixture(arch);
                    let target = match group {
                        Some(key) => &mut changed[key],
                        None => &mut changed,
                    };
                    target["unknown"] = bad;
                    rejected(&changed, arch);
                }
            }
        }
    }

    #[test]
    fn duplicate_fields_at_all_three_depths_are_not_collapsed() {
        for arch in ["x86_64", "aarch64"] {
            let value = fixture(arch);
            let raw = serde_json::to_string(&value).unwrap();
            for object in [
                value.as_object().unwrap(),
                value["patch_sha256"].as_object().unwrap(),
                value["sha256"].as_object().unwrap(),
            ] {
                for (key, original) in object {
                    let name = serde_json::to_string(key).unwrap();
                    let needle = format!("{name}:");
                    assert_eq!(raw.matches(&needle).count(), 1);
                    let duplicate = format!("{name}:{original},{name}:");
                    let changed = raw.replacen(&needle, &duplicate, 1);
                    assert!(decode(changed.as_bytes(), arch).is_err());
                }
            }
        }
    }

    #[test]
    fn exact_source_tags_flavor_feature_and_vendor_configuration_are_pinned() {
        for arch in ["x86_64", "aarch64"] {
            for (key, bad) in [
                ("schema", json!(1)),
                ("schema", json!("omavless-managed-dns-pair-v1")),
                ("mihomo_commit", json!("d".repeat(40))),
                ("mihomo_tag", json!("v1.19.32")),
                ("sing_tun_commit", json!("e".repeat(40))),
                ("sing_tun_tag", json!("v0.4.25")),
                ("package_flavor", json!("experimental")),
                ("broker_feature", json!("k1-managed-device")),
                ("go_build_tags", json!("with_gvisor,omavless_k1_device")),
                ("go_build_tags", json!("")),
                ("go_dependency_mode", json!("mod")),
            ] {
                let mut changed = fixture(arch);
                changed[key] = bad;
                rejected(&changed, arch);
            }
        }
    }

    #[test]
    fn unknown_patch_hash_and_patch_replacement_are_not_this_family() {
        for arch in ["x86_64", "aarch64"] {
            for key in [
                "mihomo-dns-broker.patch",
                "sing-tun-descriptor.patch",
                "mihomo-traffic-ready.patch",
            ] {
                for bad in ["f".repeat(64), "0".repeat(64), "F".repeat(64)] {
                    let mut changed = fixture(arch);
                    changed["patch_sha256"][key] = json!(bad);
                    rejected(&changed, arch);
                }
            }
            let mut changed = fixture(arch);
            changed["patch_sha256"]
                .as_object_mut()
                .unwrap()
                .remove("mihomo-traffic-ready.patch");
            changed["patch_sha256"]["mihomo-conditional-close.patch"] =
                json!("0858827e1af00c3ed3196f021b0dbc76ce34a8de7aa7130d7085614d149acc8f");
            rejected(&changed, arch);
        }
    }

    #[test]
    fn no_close_or_k1_or_diagnostic_family_can_be_promoted_by_relabeling() {
        for arch in ["x86_64", "aarch64"] {
            for schema in [
                "omavless-managed-dns-close-pair-v1",
                "omavless-managed-dns-k1-close-pair-v1",
            ] {
                let mut changed = fixture(arch);
                changed["schema"] = json!(schema);
                rejected(&changed, arch);
            }
            for flavor in ["release-close", "release-close-k1"] {
                let mut changed = fixture(arch);
                changed["package_flavor"] = json!(flavor);
                rejected(&changed, arch);
            }
            for (field, extra) in [
                ("conditional_close_abi", json!(1)),
                ("go_cgo", json!(false)),
                ("go_buildvcs", json!(false)),
                ("managed_device", json!("omavless0")),
                ("enrollment_policy", json!("omavless0-ipv4-development-v1")),
                (
                    "managed_device_source",
                    json!("08194a275d315db7ca502e50f960c80af6dc163b"),
                ),
                ("production_adoption", json!(true)),
            ] {
                let mut changed = fixture(arch);
                changed[field] = extra;
                rejected(&changed, arch);
            }
            for name in ["mihomo-conditional-close.patch", "mihomo-k1-device.patch"] {
                let mut changed = fixture(arch);
                changed["patch_sha256"][name] = Value::Null;
                rejected(&changed, arch);
            }
        }
    }

    #[test]
    fn all_digest_fields_require_nonzero_lowercase_fixed_width_hex() {
        for arch in ["x86_64", "aarch64"] {
            let value = fixture(arch);
            let mut paths = vec![
                (None, "omavless_commit", 40),
                (None, "go_binary_sha256", 64),
                (None, "cargo_lock_sha256", 64),
            ];
            for key in value["sha256"].as_object().unwrap().keys() {
                paths.push((Some("sha256"), key.as_str(), 64));
            }
            for (group, key, width) in paths {
                for bad in [
                    String::new(),
                    "a".repeat(width - 1),
                    "a".repeat(width + 1),
                    "0".repeat(width),
                    "A".repeat(width),
                    "g".repeat(width),
                ] {
                    let mut changed = value.clone();
                    match group {
                        Some(group) => changed[group][key] = json!(bad),
                        None => changed[key] = json!(bad),
                    }
                    rejected(&changed, arch);
                }
            }
        }
    }

    #[test]
    fn bounded_tool_version_text_preserves_close_decoder_safeguards() {
        for arch in ["x86_64", "aarch64"] {
            for (key, bad) in [
                ("rustc_version", "cargo 1.98.1".to_owned()),
                ("cargo_version", "rustc 1.98.1".to_owned()),
                ("rustc_version", "rustc x\nprivate".to_owned()),
                ("cargo_version", "cargo x\u{7f}".to_owned()),
                ("cargo_version", format!("cargo {}", "a".repeat(129))),
                ("go_version", "go version go1.27 linux/amd64".to_owned()),
                ("go_version", "go version go1.27.0- linux/amd64".to_owned()),
                ("go_version", "go version go1.27.0 linux/amd64\n".to_owned()),
            ] {
                let mut changed = fixture(arch);
                changed[key] = json!(bad);
                rejected(&changed, arch);
            }
            let goarch = if arch == "x86_64" { "amd64" } else { "arm64" };
            let mut suffix = fixture(arch);
            suffix["go_version"] = json!(format!("go version go1.27.0-X:nodwarf5 linux/{goarch}"));
            assert!(decode(&serde_json::to_vec(&suffix).unwrap(), arch).is_ok());
        }
    }

    #[test]
    fn input_bounds_utf8_and_exactly_one_json_document_are_required() {
        assert!(decode(&[], "x86_64").is_err());
        assert!(decode(&vec![b' '; 8193], "x86_64").is_err());
        assert!(decode(&[0xff], "x86_64").is_err());
        for raw in [b"null".as_slice(), b"[]", b"true", b"{}"] {
            assert!(decode(raw, "x86_64").is_err());
        }
        let raw = serde_json::to_string(&fixture("x86_64")).unwrap();
        assert!(decode(format!("{raw}{{}}").as_bytes(), "x86_64").is_err());
        assert!(decode(format!("{raw}\n").as_bytes(), "x86_64").is_ok());
    }
}
