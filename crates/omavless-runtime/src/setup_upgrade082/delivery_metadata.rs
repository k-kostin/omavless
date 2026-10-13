// SPDX-License-Identifier: MIT
//! Closed frontend DATA input. Release authenticity is established by the
//! trusted frontend before invoking its extracted destination helper; this
//! decoder checks consistency, never authenticity or effect authority.
use super::*;
use serde::Deserialize;

const MAXIMUM: u64 = 2048;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Metadata {
    #[serde(rename = "schemaVersion")]
    schema: u8,
    #[serde(rename = "appSHA")]
    pub(super) app_sha: String,
    #[serde(rename = "dnsSHA")]
    pub(super) dns_sha: String,
    #[serde(rename = "sourceCommit")]
    pub(super) source: String,
    pub(super) version: String,
    pub(super) arch: String,
}
fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
impl Metadata {
    pub(super) fn read(reader: &mut impl Read) -> Result<Self> {
        let mut bytes = Vec::new();
        reader
            .take(MAXIMUM + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Blocker::DeliveryMetadataInvalid)?;
        if bytes.len() > MAXIMUM as usize {
            return Err(Blocker::DeliveryMetadataInvalid);
        }
        let value: Self =
            serde_json::from_slice(&bytes).map_err(|_| Blocker::DeliveryMetadataInvalid)?;
        value.validate()?;
        Ok(value)
    }
    fn validate(&self) -> Result<()> {
        if self.schema != 1
            || !hex(&self.app_sha, 64)
            || !hex(&self.dns_sha, 64)
            || !hex(&self.source, 40)
            || self.version != env!("CARGO_PKG_VERSION")
            || self.version.len() > 32
            || self.version.split('.').count() != 3
            || !self
                .version
                .split('.')
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
            || self.arch != std::env::consts::ARCH
            || !matches!(self.arch.as_str(), "x86_64" | "aarch64")
        {
            return Err(Blocker::DeliveryMetadataInvalid);
        }
        Ok(())
    }
    pub(super) fn developer() -> Result<Self> {
        let (app, dns) = super::package_phase::pins(std::env::consts::ARCH)?;
        let value = Self {
            schema: 1,
            app_sha: app.to_owned(),
            dns_sha: dns.to_owned(),
            source: super::package_phase::SOURCE.to_owned(),
            version: super::package_phase::VERSION.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
        };
        value.validate()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn good() -> serde_json::Value {
        serde_json::json!({"schemaVersion":1,"appSHA":"a".repeat(64),"dnsSHA":"b".repeat(64),
            "sourceCommit":"c".repeat(40),"version":env!("CARGO_PKG_VERSION"),"arch":std::env::consts::ARCH})
    }
    #[test]
    fn closed_frame_rejects_injection_types_unknown_duplicate_and_trailing_input() {
        assert!(Metadata::read(&mut good().to_string().as_bytes()).is_ok());
        for key in ["path", "url", "uid", "command", "args", "helperPath"] {
            let mut value = good();
            value[key] = serde_json::json!("synthetic-secret");
            assert!(matches!(
                Metadata::read(&mut value.to_string().as_bytes()),
                Err(Blocker::DeliveryMetadataInvalid)
            ));
        }
        let original = good().to_string();
        for value in [
            original.replacen('{', "{\"schemaVersion\":1,", 1),
            original.replacen('{', "{\"app\\u0053HA\":\"duplicate\",", 1),
            format!("{original}{{}}"),
            "x".repeat(MAXIMUM as usize + 1),
            "null".to_owned(),
        ] {
            assert!(matches!(
                Metadata::read(&mut value.as_bytes()),
                Err(Blocker::DeliveryMetadataInvalid)
            ));
        }
    }
    #[test]
    fn version_arch_source_and_hash_mismatches_refuse_without_diagnostics() {
        for (key, value) in [
            ("schemaVersion", serde_json::json!(2)),
            ("version", serde_json::json!("9.9.9")),
            ("arch", serde_json::json!("unknown")),
            ("sourceCommit", serde_json::json!("../../synthetic-secret")),
            ("appSHA", serde_json::json!("A".repeat(64))),
            ("dnsSHA", serde_json::json!(true)),
        ] {
            let mut input = good();
            input[key] = value;
            assert!(matches!(
                Metadata::read(&mut input.to_string().as_bytes()),
                Err(Blocker::DeliveryMetadataInvalid)
            ));
        }
        assert!(
            !Blocker::DeliveryMetadataInvalid
                .to_string()
                .contains("synthetic-secret")
        );
    }
}
