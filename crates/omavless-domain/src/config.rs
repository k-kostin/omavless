// SPDX-License-Identifier: MIT

use crate::routing::{CustomRule, RoutingError, inject_custom_rules};
use std::fmt;

pub const PROFILE_MARKER: &str = "{{OMAVLESS_PROXY}}";
pub const MAX_TEMPLATE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_PROFILE_YAML_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    InvalidTemplate,
    InvalidProfile,
    InvalidSocket,
    Routing(RoutingError),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidTemplate => "Routing template is invalid",
            Self::InvalidProfile => "Rendered profile is invalid",
            Self::InvalidSocket => "Private controller path is invalid",
            Self::Routing(_) => "Custom routing rules are invalid",
        })
    }
}
impl std::error::Error for ConfigError {}

pub fn strip_controller_config(text: &str) -> String {
    let lines: Vec<_> = text.split_inclusive('\n').collect();
    let mut kept = String::with_capacity(text.len());
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let key = line.split_once(':').map(|(key, _)| key).unwrap_or("");
        let controller = !line.starts_with([' ', '\t'])
            && (key == "secret"
                || key == "external-controller"
                || key
                    .strip_prefix("external-controller-")
                    .is_some_and(|suffix| {
                        !suffix.is_empty()
                            && suffix.bytes().all(|byte| {
                                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
                            })
                    }));
        if !controller {
            kept.push_str(line);
            index += 1;
            continue;
        }
        index += 1;
        while index < lines.len() {
            let following = lines[index];
            if !following.trim().is_empty() && !following.starts_with([' ', '\t', '#']) {
                break;
            }
            index += 1;
        }
    }
    kept
}

pub fn assemble_runtime_config(
    template: &str,
    profile_yaml: &str,
    controller_socket: &str,
    rules: &[CustomRule],
) -> Result<String, ConfigError> {
    if template.len() > MAX_TEMPLATE_BYTES || template.matches(PROFILE_MARKER).count() != 1 {
        return Err(ConfigError::InvalidTemplate);
    }
    if profile_yaml.is_empty() || profile_yaml.len() > MAX_PROFILE_YAML_BYTES {
        return Err(ConfigError::InvalidProfile);
    }
    if controller_socket.is_empty()
        || controller_socket.len() > 4096
        || controller_socket
            .chars()
            .any(|character| character.is_control())
    {
        return Err(ConfigError::InvalidSocket);
    }
    let profile = template.replacen(PROFILE_MARKER, profile_yaml, 1);
    let stripped = strip_controller_config(&profile);
    let routed = inject_custom_rules(&stripped, rules).map_err(ConfigError::Routing)?;
    let quoted =
        serde_json::to_string(controller_socket).map_err(|_| ConfigError::InvalidSocket)?;
    Ok(format!("external-controller-unix: {quoted}\n{routed}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn assembly_removes_every_inherited_controller_and_injects_once() {
        let template = "external-controller: 0.0.0.0:9090\nsecret:\n  old\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,DIRECT\n";
        let rule = CustomRule::parse("domain", "proxy", "example.invalid").unwrap();
        let output =
            assemble_runtime_config(template, "  - name: safe\n", "/run/user/1/a.sock", &[rule])
                .unwrap();
        assert!(output.starts_with("external-controller-unix: \"/run/user/1/a.sock\"\n"));
        assert!(!output.contains("0.0.0.0"));
        assert!(!output.contains("old"));
        assert_eq!(output.matches("external-controller").count(), 1);
        assert!(
            output.find("DOMAIN,example.invalid,PROXY").unwrap()
                < output.find("MATCH,DIRECT").unwrap()
        );
    }
    #[test]
    fn paths_are_quoted_and_invalid_shapes_fail_closed() {
        let output = assemble_runtime_config(
            "proxies:\n{{OMAVLESS_PROXY}}\nrules:\n",
            "  - name: x\n",
            "/tmp/a\"b.sock",
            &[],
        )
        .unwrap();
        assert!(output.contains("a\\\"b.sock"));
        assert_eq!(
            assemble_runtime_config("{{OMAVLESS_PROXY}}{{OMAVLESS_PROXY}}", "x", "/tmp/x", &[])
                .unwrap_err(),
            ConfigError::InvalidTemplate
        );
    }
}
