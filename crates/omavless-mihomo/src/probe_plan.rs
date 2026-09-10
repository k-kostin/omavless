// SPDX-License-Identifier: MIT
//! Pure isolated-probe planning only. No DNS, process, socket, store or job owner.
//! Addresses are supplied by a future validated/pinned resolver. Typed addresses
//! prevent hostname injection, but this module does not certify their provenance
//! or public reachability. Rendered configurations contain reusable credentials.

use crate::{ErrorKind, MAX_PROBE_TARGETS, MihomoError, Result, merge_probe_response};
use omavless_profile::canonical::CanonicalProfile;
use serde_json::Value;
use std::collections::BTreeMap;
use std::net::IpAddr;

pub const MAX_PROFILES: usize = 256;
pub const MAX_ADDRESSES: usize = 4;
pub const MAX_CONFIG_BYTES: usize = 4 * 1024 * 1024;
pub const PROBE_URLS: [&str; 3] = [
    "https://www.gstatic.com/generate_204",
    "https://cp.cloudflare.com/generate_204",
    "https://www.google.com/generate_204",
];
pub const HTTP_TIMEOUT_MS: u32 = 5000;

/// No Debug: neither canonical input nor rendered output belongs in diagnostics.
pub struct PinnedProfile<'a> {
    pub profile: &'a CanonicalProfile,
    pub addresses: &'a [IpAddr],
}

pub struct ProbeChunk {
    config: String,
    targets: Vec<(String, usize)>,
}

impl ProbeChunk {
    /// Private disposable content. A future owner must write it atomically 0600
    /// under a private 0700 directory, never argv, logs or ordinary IPC.
    #[must_use]
    pub fn private_config(&self) -> &str {
        &self.config
    }

    pub fn aliases(&self) -> impl Iterator<Item = &str> {
        self.targets.iter().map(|(alias, _)| alias.as_str())
    }
}

pub struct ProbePlan {
    chunks: Vec<ProbeChunk>,
    resolved: Vec<bool>,
}

impl ProbePlan {
    pub fn new(profiles: &[PinnedProfile<'_>]) -> Result<Self> {
        if profiles.len() > MAX_PROFILES
            || profiles.iter().any(|profile| {
                profile.addresses.len() > MAX_ADDRESSES
                    || profile
                        .addresses
                        .iter()
                        .enumerate()
                        .any(|(index, address)| profile.addresses[..index].contains(address))
            })
        {
            return Err(invalid());
        }
        let mut chunks = Vec::new();
        let mut targets = Vec::new();
        let mut proxies = String::new();
        for (profile_index, profile) in profiles.iter().enumerate() {
            for (address_index, address) in profile.addresses.iter().enumerate() {
                let alias = format!("p{profile_index:04}a{address_index}");
                let rendered = profile
                    .profile
                    .render_mihomo_proxy(&alias, Some(&address.to_string()));
                if proxies.len().saturating_add(rendered.len()) > MAX_CONFIG_BYTES - 8192 {
                    return Err(invalid());
                }
                if !proxies.is_empty() {
                    proxies.push('\n');
                }
                proxies.push_str(&rendered);
                targets.push((alias, profile_index));
                if targets.len() == MAX_PROBE_TARGETS {
                    chunks.push(make_chunk(
                        std::mem::take(&mut targets),
                        std::mem::take(&mut proxies),
                    ));
                }
            }
        }
        if !targets.is_empty() {
            chunks.push(make_chunk(targets, proxies));
        }
        Ok(Self {
            chunks,
            resolved: profiles.iter().map(|p| !p.addresses.is_empty()).collect(),
        })
    }

    #[must_use]
    pub fn chunks(&self) -> &[ProbeChunk] {
        &self.chunks
    }

    #[must_use]
    pub fn collector(&self) -> ProbeCollector<'_> {
        ProbeCollector {
            plan: self,
            rounds: vec![0; self.chunks.len()],
            completed: vec![0; self.chunks.len()],
            samples: self
                .chunks
                .iter()
                .map(|chunk| {
                    chunk
                        .aliases()
                        .map(|a| (a.to_owned(), Vec::new()))
                        .collect()
                })
                .collect(),
        }
    }
}

fn make_chunk(targets: Vec<(String, usize)>, proxies: String) -> ProbeChunk {
    let members = targets
        .iter()
        .map(|(alias, _)| format!("    - \"{alias}\"\n"))
        .collect::<String>();
    ProbeChunk {
        config: format!(
            "log-level: warning\nipv6: true\nunified-delay: true\ntcp-concurrent: true\nfind-process-mode: off\nrouting-mark: 524288\nexternal-controller-unix: controller.sock\nproxies:\n{proxies}\nproxy-groups:\n  - name: \"OMAVLESS_TEST\"\n    type: select\n    proxies:\n{members}rules:\n  - MATCH,DIRECT\n"
        ),
        targets,
    }
}

/// Results are positional, avoiding a second profile-ID parser or store model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeResult {
    pub resolved: bool,
    pub reachable: bool,
    pub latency_ms: i32,
}

pub struct ProbeCollector<'a> {
    plan: &'a ProbePlan,
    rounds: Vec<usize>,
    completed: Vec<usize>,
    samples: Vec<BTreeMap<String, Vec<u32>>>,
}

impl ProbeCollector<'_> {
    /// Exactly one response per public URL, in order, for each bounded chunk.
    /// Controller parsing/response-byte caps remain the future executor's duty.
    pub fn record(
        &mut self,
        chunk: usize,
        url_index: usize,
        status: u16,
        payload: &Value,
    ) -> Result<bool> {
        if chunk >= self.rounds.len()
            || url_index >= PROBE_URLS.len()
            || self.rounds[chunk] != url_index
        {
            return Err(invalid());
        }
        self.rounds[chunk] += 1;
        let accepted = merge_probe_response(&mut self.samples[chunk], status, payload);
        if accepted {
            self.completed[chunk] += 1;
        }
        Ok(accepted)
    }

    pub fn finish(self) -> Result<Vec<ProbeResult>> {
        if self.rounds.iter().any(|rounds| *rounds != PROBE_URLS.len())
            || self.completed.contains(&0)
        {
            return Err(MihomoError::new(ErrorKind::ControllerRejected));
        }
        let mut profiles = vec![Vec::new(); self.plan.resolved.len()];
        for (chunk, samples) in self.plan.chunks.iter().zip(self.samples) {
            for (alias, index) in &chunk.targets {
                profiles[*index].extend_from_slice(&samples[alias]);
            }
        }
        Ok(profiles
            .into_iter()
            .zip(&self.plan.resolved)
            .map(|(mut samples, resolved)| {
                samples.sort_unstable();
                let latency_ms = if samples.is_empty() {
                    -1
                } else {
                    let middle = samples.len() / 2;
                    let doubled = if samples.len() % 2 == 1 {
                        2 * samples[middle]
                    } else {
                        samples[middle - 1] + samples[middle]
                    };
                    // Python round(statistics.median): nearest integer, ties to even.
                    let lower = doubled / 2;
                    (lower + u32::from(doubled % 2 == 1 && lower % 2 == 1)) as i32
                };
                ProbeResult {
                    resolved: *resolved,
                    reachable: latency_ms >= 0,
                    latency_ms,
                }
            })
            .collect())
    }
}

fn invalid() -> MihomoError {
    MihomoError::new(ErrorKind::InvalidArgument)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omavless_profile::canonical::parse_canonical;
    use serde_json::json;

    fn fixture() -> CanonicalProfile {
        parse_canonical("trojan://synthetic@example.invalid:443?sni=cdn.example.invalid").unwrap()
    }

    #[test]
    fn maximum_plan_chunks_are_bounded_unique_and_no_tun() {
        let profile = fixture();
        let addresses =
            ["192.0.2.1", "192.0.2.2", "2001:db8::1", "2001:db8::2"].map(|s| s.parse().unwrap());
        let inputs: Vec<_> = (0..MAX_PROFILES)
            .map(|_| PinnedProfile {
                profile: &profile,
                addresses: &addresses,
            })
            .collect();
        let plan = ProbePlan::new(&inputs).unwrap();
        assert_eq!(plan.chunks.len(), 16);
        let mut aliases = std::collections::BTreeSet::new();
        for chunk in plan.chunks() {
            assert_eq!(chunk.aliases().count(), 64);
            for alias in chunk.aliases() {
                assert!(aliases.insert(alias));
            }
            assert!(chunk.config.len() < MAX_CONFIG_BYTES);
            for absent in [
                "\ntun:",
                "\ndns:",
                "external-controller:",
                "mixed-port:",
                "socks-port:",
            ] {
                assert!(!chunk.config.contains(absent));
            }
            assert!(chunk.config.contains("cdn.example.invalid"));
        }
    }

    #[test]
    fn excessive_profiles_addresses_and_duplicates_rejected() {
        let profile = fixture();
        let inputs: Vec<_> = (0..=MAX_PROFILES)
            .map(|_| PinnedProfile {
                profile: &profile,
                addresses: &[],
            })
            .collect();
        assert!(ProbePlan::new(&inputs).is_err());
        let addresses = ["192.0.2.1".parse().unwrap(); 5];
        for addresses in [&addresses[..], &addresses[..2]] {
            let error = ProbePlan::new(&[PinnedProfile {
                profile: &profile,
                addresses,
            }])
            .err()
            .unwrap();
            assert_eq!(error.kind(), ErrorKind::InvalidArgument);
            assert!(!error.to_string().contains("synthetic"));
        }
    }

    #[test]
    fn resolved_timeout_and_unresolved_are_distinct() {
        let profile = fixture();
        let addresses = ["192.0.2.1".parse().unwrap()];
        let plan = ProbePlan::new(&[
            PinnedProfile {
                profile: &profile,
                addresses: &[],
            },
            PinnedProfile {
                profile: &profile,
                addresses: &addresses,
            },
        ])
        .unwrap();
        let mut collector = plan.collector();
        for index in 0..3 {
            assert!(
                collector
                    .record(
                        0,
                        index,
                        504,
                        &json!({"message":"get delay: all proxies timeout"})
                    )
                    .unwrap()
            );
        }
        assert_eq!(
            collector.finish().unwrap(),
            vec![
                ProbeResult {
                    resolved: false,
                    reachable: false,
                    latency_ms: -1
                },
                ProbeResult {
                    resolved: true,
                    reachable: false,
                    latency_ms: -1
                }
            ]
        );
    }

    #[test]
    fn incomplete_invalid_and_duplicate_rounds_fail_closed() {
        let profile = fixture();
        let addresses = ["192.0.2.1".parse().unwrap()];
        let plan = ProbePlan::new(&[PinnedProfile {
            profile: &profile,
            addresses: &addresses,
        }])
        .unwrap();
        assert!(plan.collector().finish().is_err());
        let mut collector = plan.collector();
        assert!(collector.record(1, 0, 200, &json!({})).is_err());
        assert!(collector.record(0, 1, 200, &json!({})).is_err());
        for index in 0..3 {
            assert!(
                !collector
                    .record(0, index, 500, &json!({"private":"not echoed"}))
                    .unwrap()
            );
        }
        assert!(collector.record(0, 3, 200, &json!({})).is_err());
        assert!(collector.finish().is_err());
    }

    #[test]
    fn median_ties_are_python_even_and_response_samples_stay_bounded() {
        let profile = fixture();
        let addresses = ["192.0.2.1".parse().unwrap()];
        let plan = ProbePlan::new(&[PinnedProfile {
            profile: &profile,
            addresses: &addresses,
        }])
        .unwrap();
        for (left, right, expected) in [(10, 11, 10), (11, 12, 12), (1, 60000, 30000)] {
            let mut collector = plan.collector();
            collector
                .record(0, 0, 200, &json!({"p0000a0":left,"unknown":1}))
                .unwrap();
            assert!(
                collector
                    .record(0, 0, 200, &json!({"p0000a0":500}))
                    .is_err()
            );
            collector
                .record(0, 1, 200, &json!({"p0000a0":right}))
                .unwrap();
            collector
                .record(0, 2, 200, &json!({"p0000a0":60001}))
                .unwrap();
            assert_eq!(collector.finish().unwrap()[0].latency_ms, expected);
        }
    }

    #[test]
    fn empty_plan_completes_without_requests() {
        let plan = ProbePlan::new(&[]).unwrap();
        assert!(plan.chunks().is_empty());
        assert!(plan.collector().finish().unwrap().is_empty());
    }

    #[test]
    fn one_profile_split_across_chunks_reduces_all_address_samples() {
        let profile = fixture();
        let addresses =
            ["192.0.2.1", "192.0.2.2", "192.0.2.3", "192.0.2.4"].map(|s| s.parse().unwrap());
        let mut inputs: Vec<_> = (0..63)
            .map(|_| PinnedProfile {
                profile: &profile,
                addresses: &addresses[..1],
            })
            .collect();
        inputs.push(PinnedProfile {
            profile: &profile,
            addresses: &addresses,
        });
        let plan = ProbePlan::new(&inputs).unwrap();
        assert_eq!(plan.chunks.len(), 2);
        let mut collector = plan.collector();
        for round in 0..3 {
            collector
                .record(0, round, 200, &json!({"p0063a0":10}))
                .unwrap();
            collector
                .record(
                    1,
                    round,
                    200,
                    &json!({"p0063a1":20,"p0063a2":30,"p0063a3":40}),
                )
                .unwrap();
        }
        let results = collector.finish().unwrap();
        assert_eq!(results[63].latency_ms, 25);
        assert_eq!(results[0].latency_ms, -1);
        assert!(results.iter().all(|result| result.resolved));
    }
}
