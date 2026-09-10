// SPDX-License-Identifier: MIT
//! Explicit fixed HTTPS observation under current routing, not a tunnel/leak test.
use serde_json::{Value, json};
use std::net::IpAddr;
use std::time::{Duration, Instant};
use ureq::tls::{RootCerts, TlsConfig};

pub(crate) const DEADLINE: Duration = Duration::from_secs(3);
const TARGETS: [&str; 2] = ["https://checkip.amazonaws.com", "https://api.ipify.org"];

fn public_ip(bytes: &[u8]) -> Option<IpAddr> {
    if bytes.len() > 64 {
        return None;
    }
    let ip: IpAddr = std::str::from_utf8(bytes).ok()?.trim().parse().ok()?;
    // A response is an observation, not an assertion of global routability.
    // Refuse clearly local/non-address responses rather than showing HTML/text.
    if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
        return None;
    }
    Some(ip)
}

fn collect_with<F>(mut fetch: F) -> Value
where
    F: FnMut(&str, Duration) -> Option<Vec<u8>>,
{
    let started = Instant::now();
    for target in TARGETS {
        let Some(remaining) = DEADLINE
            .checked_sub(started.elapsed())
            .filter(|d| !d.is_zero())
        else {
            break;
        };
        if let Some(ip) = fetch(target, remaining).as_deref().and_then(public_ip)
            && started.elapsed() < DEADLINE
        {
            return json!({"schemaVersion":1,"scope":"current_route_https","https":true,
                "observedIp":ip.to_string(),"elapsedMs":started.elapsed().as_millis() as u64,"code":"ok"});
        }
    }
    json!({"schemaVersion":1,"scope":"current_route_https","https":false,
        "observedIp":null,"elapsedMs":started.elapsed().as_millis().min(3000) as u64,"code":"request_failed"})
}

pub(crate) fn collect() -> Value {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(DEADLINE))
        .max_redirects(0)
        .http_status_as_error(false)
        .max_response_header_size(8192)
        .proxy(None)
        .accept("text/plain")
        .accept_encoding("identity")
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .new_agent();
    collect_with(|target, remaining| {
        let mut response = agent
            .get(target)
            .config()
            .timeout_global(Some(remaining))
            .build()
            .call()
            .ok()?;
        if response.status().as_u16() != 200 {
            return None;
        }
        response
            .body_mut()
            .with_config()
            .limit(64)
            .read_to_vec()
            .ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "explicit external HTTPS acceptance; fixed public targets, current host route"]
    fn fixed_live_https_acceptance() {
        // Call the exact production collector, including its aggregate deadline.
        // Never format this private result in assertions or diagnostic output.
        let result = collect();
        assert_eq!(result["schemaVersion"], 1);
        assert_eq!(result["scope"], "current_route_https");
        assert!(
            result["https"] == true && result["code"] == "ok",
            "fixed current-route HTTPS observation failed; VPN health is not inferred"
        );
        assert!(
            result["observedIp"]
                .as_str()
                .is_some_and(|ip| public_ip(ip.as_bytes()).is_some()),
            "successful HTTPS observation must contain a valid private IP result"
        );
        assert!(result["elapsedMs"].as_u64().is_some_and(|ms| ms < 3000));
    }
    #[test]
    fn explicit_observation_has_bounded_ip_only() {
        let r = collect_with(|_, _| Some(b"203.0.113.7\n".to_vec()));
        assert_eq!(r["https"], true);
        assert_eq!(r["observedIp"], "203.0.113.7");
        assert_eq!(r["scope"], "current_route_https");
        assert!(r.get("tunVerified").is_none());
        assert!(r.to_string().len() < 256);
    }
    #[test]
    fn fixed_targets_fallback_and_private_error_non_echo() {
        let mut calls = Vec::new();
        let r = collect_with(|target, budget| {
            assert!(budget <= DEADLINE);
            calls.push(target.to_owned());
            Some(b"https://private.invalid/password=secret".to_vec())
        });
        assert_eq!(calls, TARGETS);
        assert_eq!(r["https"], false);
        assert!(r["observedIp"].is_null());
        assert!(!r.to_string().contains("private"));
        assert!(!r.to_string().contains("secret"));
    }
    #[test]
    fn response_shape_validation_matches_and_tightens_legacy_parser() {
        assert!(public_ip(b"2001:db8::7\n").is_some());
        for raw in [
            "",
            "127.0.0.1",
            "::",
            "224.0.0.1",
            "999.1.1.1",
            "1.2.3.4\n5.6.7.8",
            ":::",
            "<b>1.2.3.4</b>",
        ] {
            assert!(public_ip(raw.as_bytes()).is_none());
        }
        assert!(public_ip(&[b'1'; 65]).is_none());
    }
}
