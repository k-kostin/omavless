// SPDX-License-Identifier: MIT
//! Managed subset for Mihomo ab405bad / AWG engine 0c1c6f40ecd7.
//! Data parsing/export is deliberately broader than execution permission.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedCompatibilityError {
    Ipv4Only,
    KeepaliveRange,
    HeaderRanges,
    LegacyHeaders,
    Padding,
    LegacyPadding,
    JunkBudget,
    SpecialJunk,
    NumericParameters,
}

impl fmt::Display for ManagedCompatibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ipv4Only => "Managed WireGuard connections currently require IPv4",
            Self::KeepaliveRange => "The managed core cannot preserve a variable peer keepalive",
            Self::HeaderRanges => "The managed core requires nonoverlapping uint32 headers",
            Self::LegacyHeaders => {
                "The managed core cannot preserve these legacy header parameters"
            }
            Self::Padding => "The managed core cannot use these padding parameters",
            Self::LegacyPadding => "The managed core cannot use these legacy packet sizes",
            Self::JunkBudget => "Junk packet parameters exceed the managed execution budget",
            Self::SpecialJunk => {
                "This special junk expression is not supported for managed connections"
            }
            Self::NumericParameters => "The managed core cannot interpret these numeric parameters",
        })
    }
}
impl std::error::Error for ManagedCompatibilityError {}

fn range(value: &str) -> Option<(u32, u32)> {
    let (lo, hi) = value.split_once('-').unwrap_or((value, value));
    // Pinned UAPI uses strconv.ParseUint: signs/whitespace are not accepted.
    if lo.is_empty()
        || hi.is_empty()
        || !lo.bytes().all(|byte| byte.is_ascii_digit())
        || !hi.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let (lo, hi) = (lo.parse().ok()?, hi.parse().ok()?);
    (lo <= hi).then_some((lo, hi))
}

// The pinned v3 obf.go builders consume b/t/r/rc/rd exactly as below. Dynamic
// d/ds/dz and the different legacy generator grammar remain explicit refusal.
// Refuse ignored text/extra arguments rather than claim exact interpretation.
fn static_v3_junk(spec: &str) -> bool {
    let mut remaining = spec.trim();
    let mut bytes = 0_usize;
    let mut tags = 0;
    while !remaining.is_empty() {
        let Some(body) = remaining.strip_prefix('<') else {
            return false;
        };
        let Some((tag, rest)) = body.split_once('>') else {
            return false;
        };
        let parts: Vec<_> = tag.split_ascii_whitespace().collect();
        let length = match parts.as_slice() {
            ["t"] => 4,
            ["b", value] => {
                let hex = value.strip_prefix("0x").unwrap_or(value);
                if hex.is_empty()
                    || hex.len() % 2 != 0
                    || !hex.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return false;
                }
                hex.len() / 2
            }
            ["r" | "rc" | "rd", value] if value.bytes().all(|b| b.is_ascii_digit()) => {
                let Ok(n) = value.parse::<usize>() else {
                    return false;
                };
                n
            }
            _ => return false,
        };
        let Some(total) = bytes.checked_add(length) else {
            return false;
        };
        bytes = total;
        tags += 1;
        if bytes > MAX_SPECIAL_JUNK_BYTES || tags > 128 {
            return false;
        }
        remaining = rest.trim_start();
    }
    tags > 0
}

impl WireGuardProfile {
    /// One credential-derived predicate shared by all managed admission paths.
    /// It never rewrites or repairs private material and opens no resources.
    pub fn managed_compatibility(&self) -> Result<(), ManagedCompatibilityError> {
        use ManagedCompatibilityError as E;
        if self.addresses.iter().any(|ip| ip.address.is_ipv6())
            || self.allowed_ips.iter().any(|ip| ip.address.is_ipv6())
            || self.endpoint.kind == HostKind::Ipv6
        {
            return Err(E::Ipv4Only);
        }
        if matches!(self.persistent_keepalive, Some(Keepalive::Range {minimum,maximum}) if minimum != maximum)
        {
            return Err(E::KeepaliveRange);
        }
        let Some(awg) = &self.awg else {
            return Ok(());
        };
        let field = |key: &str| awg.values.get(key).map(String::as_str);
        let headers = ["h1", "h2", "h3", "h4"].map(|key| field(key).and_then(range));
        if headers.iter().any(Option::is_none) {
            return Err(E::HeaderRanges);
        }
        let v3 = matches!(awg.generation, AwgGeneration::V3 | AwgGeneration::V3_1);
        // device_v1/device.go substitutes 1/2/3/4 whenever Min <= 4.
        // Admit only identity mappings, never silently lower a private range.
        if !v3
            && headers.iter().enumerate().any(|(index, header)| {
                let (lo, hi) = header.expect("checked header");
                lo <= 4 && (lo != index as u32 + 1 || hi != lo)
            })
        {
            return Err(E::LegacyHeaders);
        }
        for (i, a) in headers.iter().enumerate() {
            for b in &headers[i + 1..] {
                let (a, b) = (a.expect("checked header"), b.expect("checked header"));
                if a.0 <= b.1 && b.0 <= a.1 {
                    return Err(E::HeaderRanges);
                }
            }
        }
        let number = |key: &str| field(key).and_then(|v| v.parse::<u32>().ok());
        let (Some(jc), Some(jmin), Some(jmax)) = (number("jc"), number("jmin"), number("jmax"))
        else {
            return Err(E::JunkBudget);
        };
        if jc > 64 || jmin > jmax || jmax > 4096 {
            return Err(E::JunkBudget);
        }
        for key in [
            "contentpaddingaddition",
            "rekeyaftertime",
            "rekeytimeout",
            "rejectaftertime",
            "keepalivetimeout",
            "maxhandshakeattempts",
        ] {
            if field(key).is_some_and(|value| range(value).is_none()) {
                return Err(E::NumericParameters);
            }
        }
        let keyed = field("headerprotectionkey").is_some_and(|key| {
            STANDARD
                .decode(key)
                .is_ok_and(|bytes| bytes.iter().any(|byte| *byte != 0))
        });
        for key in ["s1", "s2", "s3", "s4"] {
            if let Some(value) = number(key)
                && (value > u32::from(u16::MAX) || (v3 && keyed && value < 12))
            {
                return Err(E::Padding);
            }
        }
        if !v3 {
            // Linux device_v1/device.go validates the complete packet sizes
            // at lazy UAPI initialization, not during Mihomo's config test.
            let sizes = [("s1", 148_u32), ("s2", 92), ("s3", 64), ("s4", 32)]
                .map(|(key, header)| header + number(key).unwrap_or(0));
            for (index, size) in sizes.iter().enumerate() {
                if *size >= 65535 || sizes[index + 1..].contains(size) {
                    return Err(E::LegacyPadding);
                }
            }
        }
        for key in ["i1", "i2", "i3", "i4", "i5"] {
            if let Some(spec) = field(key)
                && (!v3 || !static_v3_junk(spec))
            {
                return Err(E::SpecialJunk);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wireguard::private_record::parse_private_wireguard_record;

    fn config(generation: u8) -> String {
        let mut fields = String::new();
        if generation > 0 {
            fields.push_str("Jc = 4\nJmin = 10\nJmax = 30\nS1 = 20\nS2 = 25\nH1 = 100\nH2 = 200\nH3 = 300\nH4 = 400\n");
        }
        if generation >= 2 {
            fields.push_str("S3 = 30\nS4 = 35\n");
        }
        if generation >= 3 {
            fields.push_str(&format!("HeaderProtectionKey = {}\nContentPaddingAddition = 12-100\nRekeyAfterTime = 100-120\nRekeyTimeout = 3-7\nRejectAfterTime = 150-180\nKeepaliveTimeout = 5-15\nMaxHandshakeAttempts = 15-20\n",STANDARD.encode([15_u8;32])));
        }
        if generation == 4 {
            fields.push_str("RandomTrailers = true\nDisableCookies = false\n");
        }
        format!(
            "[Interface]\nPrivateKey = {}\nAddress = 10.8.0.2/32\n{fields}[Peer]\nPublicKey = {}\nAllowedIPs = 0.0.0.0/0\nEndpoint = 192.0.2.2:51820\nPersistentKeepalive = 25\n",
            STANDARD.encode([7_u8; 32]),
            STANDARD.encode([9_u8; 32])
        )
    }

    fn result(input: &str) -> Result<(), ManagedCompatibilityError> {
        parse_wireguard_config(input)
            .unwrap()
            .managed_compatibility()
    }

    #[test]
    fn explicit_disabled_keepalive_survives_private_roundtrip_and_managed_render() {
        let input = config(0).replace("PersistentKeepalive = 25", "PersistentKeepalive = 0");
        let profile = parse_wireguard_config(&input).unwrap();
        let original = profile.private_record().unwrap();
        let restored = parse_private_wireguard_record(original.expose_private_bytes()).unwrap();
        let exported = restored.private_config().unwrap();
        assert!(
            std::str::from_utf8(exported.expose_private_bytes())
                .unwrap()
                .contains("PersistentKeepalive = 0\n")
        );
        let reparsed =
            parse_wireguard_config(std::str::from_utf8(exported.expose_private_bytes()).unwrap())
                .unwrap();
        assert_eq!(reparsed.managed_compatibility(), Ok(()));
        let rendered = reparsed.render_mihomo_proxy("Test", None).unwrap();
        assert!(
            rendered
                .lines()
                .any(|line| line == "  persistent-keepalive: 0")
        );
        assert_eq!(
            reparsed.private_record().unwrap().expose_private_bytes(),
            original.expose_private_bytes()
        );
        let omitted =
            parse_wireguard_config(&config(0).replace("PersistentKeepalive = 25\n", "")).unwrap();
        assert!(
            !omitted
                .render_mihomo_proxy("Test", None)
                .unwrap()
                .contains("persistent-keepalive:")
        );
        assert_ne!(
            profile.subscription_identity(),
            omitted.subscription_identity()
        );
    }

    #[test]
    fn scalar_keepalive_bounds_do_not_widen_range_or_family_admission() {
        for generation in 0..=4 {
            let base = config(generation);
            for value in ["0", "1", "65535"] {
                let input = base.replace(
                    "PersistentKeepalive = 25",
                    &format!("PersistentKeepalive = {value}"),
                );
                let profile = parse_wireguard_config(&input).unwrap();
                assert_eq!(profile.managed_compatibility(), Ok(()));
                assert!(
                    profile
                        .render_mihomo_proxy("Test", None)
                        .unwrap()
                        .lines()
                        .any(|line| line == format!("  persistent-keepalive: {value}"))
                );
                assert_eq!(
                    profile.render_mihomo_proxy("Test", Some("2001:db8::2")),
                    Err(ManagedCompatibilityError::Ipv4Only)
                );
            }
            for value in ["-1", "65536", "off", "false", "0-0", "0-25"] {
                let input = base.replace(
                    "PersistentKeepalive = 25",
                    &format!("PersistentKeepalive = {value}"),
                );
                assert_eq!(
                    parse_wireguard_config(&input).unwrap_err(),
                    WireGuardError::InvalidKeepalive
                );
            }
            let ranged = base.replace("PersistentKeepalive = 25", "PersistentKeepalive = 1-2");
            if generation == 0 {
                assert_eq!(
                    parse_wireguard_config(&ranged).unwrap_err(),
                    WireGuardError::InvalidKeepalive
                );
            } else {
                let profile = parse_wireguard_config(&ranged).unwrap();
                assert_eq!(
                    profile.managed_compatibility(),
                    Err(ManagedCompatibilityError::KeepaliveRange)
                );
                assert!(
                    std::str::from_utf8(profile.private_config().unwrap().expose_private_bytes())
                        .unwrap()
                        .contains("PersistentKeepalive = 1-2\n")
                );
            }
        }
    }

    #[test]
    fn one_peer_ipv4_fixed_keepalive_subset_is_representable_without_mutating_data() {
        for generation in 0..=4 {
            let p = parse_wireguard_config(&config(generation)).unwrap();
            let before = p.private_record().unwrap();
            assert_eq!(p.managed_compatibility(), Ok(()));
            let yaml = p.render_mihomo_proxy("Test", None).unwrap();
            assert!(yaml.contains("peers:\n    - server:"));
            assert!(yaml.contains("ip-version: ipv4"));
            assert!(yaml.contains("persistent-keepalive: 25"));
            assert!(
                p.private_record().unwrap().expose_private_bytes() == before.expose_private_bytes()
            );
            assert_eq!(
                p.render_mihomo_proxy("Test", Some("2001:db8::2")),
                Err(ManagedCompatibilityError::Ipv4Only)
            );
        }
    }

    #[test]
    fn decimal_padding_does_not_become_yaml_octal_and_private_spelling_survives() {
        let input = config(3).replace("S1 = 20", "S1 = 00012");
        let profile = parse_wireguard_config(&input).unwrap();
        assert_eq!(profile.managed_compatibility(), Ok(()));
        let original = profile.private_record().unwrap();
        let rendered = profile.render_mihomo_proxy("Test", None).unwrap();
        assert!(rendered.lines().any(|line| line == "    s1: 12"));
        assert!(!rendered.contains("s1: 00012"));
        let exported = profile.private_config().unwrap();
        assert!(
            std::str::from_utf8(exported.expose_private_bytes())
                .unwrap()
                .contains("S1 = 00012\n")
        );
        let restored = parse_private_wireguard_record(original.expose_private_bytes()).unwrap();
        assert_eq!(
            restored.render_mihomo_proxy("Test", None).unwrap(),
            rendered
        );
        assert_eq!(
            profile.private_record().unwrap().expose_private_bytes(),
            original.expose_private_bytes()
        );
    }

    #[test]
    fn every_awg_numeric_yaml_field_uses_decimal_without_rewriting_private_data() {
        for generation in 1..=4 {
            for (field, plain, spelling, output) in [
                ("Jc", "4", "0004", "    jc: 4"),
                ("Jmin", "10", "0010", "    jmin: 10"),
                ("Jmax", "30", "+0030", "    jmax: 30"),
                ("S1", "20", "00020", "    s1: 20"),
                ("S2", "25", "00025", "    s2: 25"),
                ("S3", "30", "00030", "    s3: 30"),
                ("S4", "35", "00035", "    s4: 35"),
            ] {
                let base = config(generation);
                let original_line = format!("{field} = {plain}\n");
                if generation == 1 && matches!(field, "S3" | "S4") {
                    assert!(!base.contains(&original_line));
                    continue;
                }
                let private_line = format!("{field} = {spelling}\n");
                let input = base.replace(&original_line, &private_line);
                assert_ne!(input, base);
                let profile = parse_wireguard_config(&input).unwrap();
                assert_eq!(profile.managed_compatibility(), Ok(()));
                assert!(
                    profile
                        .render_mihomo_proxy("Test", None)
                        .unwrap()
                        .lines()
                        .any(|line| line == output)
                );
                assert!(
                    std::str::from_utf8(profile.private_config().unwrap().expose_private_bytes())
                        .unwrap()
                        .contains(&private_line)
                );
            }
        }
    }

    #[test]
    fn ipv6_and_nonconstant_keepalive_are_data_not_execution_permission() {
        let base = config(3);
        for input in [
            base.replace("10.8.0.2/32", "10.8.0.2/32, fd00::2/128"),
            base.replace("0.0.0.0/0", "0.0.0.0/0, ::/0"),
            base.replace("192.0.2.2:51820", "[2001:db8::2]:51820"),
        ] {
            let p = parse_wireguard_config(&input).unwrap();
            assert_eq!(
                p.managed_compatibility(),
                Err(ManagedCompatibilityError::Ipv4Only)
            );
            assert!(p.private_config().is_ok());
        }
        let ranged = base.replace("PersistentKeepalive = 25", "PersistentKeepalive = 25-35");
        let p = parse_wireguard_config(&ranged).unwrap();
        assert_eq!(
            p.managed_compatibility(),
            Err(ManagedCompatibilityError::KeepaliveRange)
        );
        assert!(
            std::str::from_utf8(p.private_config().unwrap().expose_private_bytes())
                .unwrap()
                .contains("PersistentKeepalive = 25-35")
        );
        assert_eq!(
            result(&base.replace("PersistentKeepalive = 25", "PersistentKeepalive = 25-25")),
            Ok(())
        );
    }

    #[test]
    fn existing_address_admission_never_silently_selects_one_of_two_ipv4_addresses() {
        let input = config(0).replace("10.8.0.2/32", "10.8.0.2/32, 10.8.1.2/32");
        assert!(matches!(
            parse_wireguard_config(&input),
            Err(WireGuardError::TooManyAddresses)
        ));
    }

    #[test]
    fn uint32_headers_preserve_range_limits_and_reject_pairwise_overlap() {
        let base = config(3);
        for value in ["4294967295", "4294967290-4294967295"] {
            assert_eq!(
                result(&base.replace("H1 = 100", &format!("H1 = {value}"))),
                Ok(())
            );
        }
        for value in ["200", "199-200", "200-201", "100 - 101", "+100"] {
            assert_eq!(
                result(&base.replace("H1 = 100", &format!("H1 = {value}"))),
                Err(ManagedCompatibilityError::HeaderRanges)
            );
        }
        for value in ["4294967296", "2-1"] {
            assert!(
                parse_wireguard_config(&base.replace("H1 = 100", &format!("H1 = {value}")))
                    .is_err()
            );
        }
    }

    #[test]
    fn legacy_low_headers_refuse_wire_substitution_but_keep_data_and_v3_distinct() {
        for generation in [1, 2] {
            let base = config(generation);
            for index in 1..=4 {
                let original = format!("H{index} = {}", index * 100);
                for value in ["0".to_owned(), (index % 4 + 1).to_string(), "1-5".into()] {
                    let input = base.replace(&original, &format!("H{index} = {value}"));
                    let p = parse_wireguard_config(&input).unwrap();
                    let before = p.private_record().unwrap();
                    assert_eq!(
                        p.managed_compatibility(),
                        Err(ManagedCompatibilityError::LegacyHeaders)
                    );
                    assert!(
                        p.private_record().unwrap().expose_private_bytes()
                            == before.expose_private_bytes()
                    );
                    assert!(
                        std::str::from_utf8(p.private_config().unwrap().expose_private_bytes())
                            .unwrap()
                            .contains(&format!("H{index} = {value}\n"))
                    );
                }
                for value in [index.to_string(), format!("{index}-{index}"), "5-99".into()] {
                    assert_eq!(
                        result(&base.replace(&original, &format!("H{index} = {value}"))),
                        Ok(())
                    );
                }
            }
        }
        for generation in [3, 4] {
            for value in ["0", "2", "1-5"] {
                assert_eq!(
                    result(&config(generation).replace("H1 = 100", &format!("H1 = {value}"))),
                    Ok(())
                );
            }
        }
    }

    #[test]
    fn keyed_padding_uses_uint16_and_nonce_boundary_twelve_not_thirteen() {
        let base = config(3);
        for value in [12, 65535] {
            assert_eq!(
                result(&base.replace("S1 = 20", &format!("S1 = {value}"))),
                Ok(())
            );
        }
        for value in [0, 11, 65536] {
            assert_eq!(
                result(&base.replace("S1 = 20", &format!("S1 = {value}"))),
                Err(ManagedCompatibilityError::Padding)
            );
        }
        let zero = base
            .replace(&STANDARD.encode([15_u8; 32]), &STANDARD.encode([0_u8; 32]))
            .replace("S1 = 20", "S1 = 0");
        assert_eq!(result(&zero), Ok(()));
    }

    #[test]
    fn legacy_padding_rejects_lazy_uapi_packet_collision_and_total_size_limits() {
        for generation in [1, 2] {
            let base = config(generation);
            let collision = base.replace("S2 = 25", "S2 = 76");
            let p = parse_wireguard_config(&collision).unwrap();
            let before = p.private_record().unwrap();
            assert_eq!(
                p.managed_compatibility(),
                Err(ManagedCompatibilityError::LegacyPadding)
            );
            assert!(
                p.private_record().unwrap().expose_private_bytes() == before.expose_private_bytes()
            );
            assert!(
                std::str::from_utf8(p.private_config().unwrap().expose_private_bytes())
                    .unwrap()
                    .contains("S2 = 76\n")
            );
            let fields = if generation == 1 { 2 } else { 4 };
            for (index, (base_size, old_padding)) in [(148, 20), (92, 25), (64, 30), (32, 35)]
                .into_iter()
                .enumerate()
                .take(fields)
            {
                let original = format!("S{} = {old_padding}", index + 1);
                let change =
                    |padding| base.replace(&original, &format!("S{} = {padding}", index + 1));
                assert_eq!(result(&change(65534 - base_size)), Ok(()));
                assert_eq!(
                    result(&change(65535 - base_size)),
                    Err(ManagedCompatibilityError::LegacyPadding)
                );
            }
        }
        let base = config(2);
        for (key, old, value) in [
            ("s2", 25, 76),
            ("s3", 30, 104),
            ("s4", 35, 136),
            ("s3", 30, 53),
            ("s4", 35, 85),
            ("s4", 35, 62),
        ] {
            assert_eq!(
                result(&base.replace(
                    &format!("{} = {old}", key.to_ascii_uppercase()),
                    &format!("{} = {value}", key.to_ascii_uppercase())
                )),
                Err(ManagedCompatibilityError::LegacyPadding)
            );
        }
        for generation in [3, 4] {
            assert_eq!(
                result(&config(generation).replace("S2 = 25", "S2 = 76")),
                Ok(())
            );
        }
    }

    #[test]
    fn v3_junk_grammar_is_exact_bounded_and_not_legacy_or_dynamic_guesswork() {
        let base = config(3);
        let with = |spec: &str| base.replace("[Peer]", &format!("I1 = {spec}\n[Peer]"));
        assert_eq!(result(&with("<b 0x0102><t><r 2><rc 3><rd 4>")), Ok(()));
        for spec in [
            "<d>",
            "<ds>",
            "<dz 4>",
            "<r -1>",
            "<r 4097>",
            "<b 0xgg>",
            "<b 0x1>",
            "<t ignored>",
            "ignored<r 2>",
            "<r 2>ignored",
            "<r 4096><t>",
            "<unknown>",
        ] {
            assert_eq!(
                result(&with(spec)),
                Err(ManagedCompatibilityError::SpecialJunk)
            );
        }
        assert_eq!(
            result(&config(2).replace("[Peer]", "I1 = <r 2>\n[Peer]")),
            Err(ManagedCompatibilityError::SpecialJunk)
        );
    }

    #[test]
    fn active_junk_and_numeric_execution_budget_refuses_without_repairing_strings() {
        let base = config(3);
        for (from, to) in [
            ("Jc = 4", "Jc = 65"),
            ("Jmax = 30", "Jmax = 4097"),
            ("Jmin = 10", "Jmin = 31"),
        ] {
            assert_eq!(
                result(&base.replace(from, to)),
                Err(ManagedCompatibilityError::JunkBudget)
            );
        }
        assert_eq!(
            result(&base.replace("RekeyTimeout = 3-7", "RekeyTimeout = 3 - 7")),
            Err(ManagedCompatibilityError::NumericParameters)
        );
        assert!(
            !format!(
                "{} {:?}",
                ManagedCompatibilityError::SpecialJunk,
                ManagedCompatibilityError::SpecialJunk
            )
            .contains("192.0.2")
        );
    }
}
