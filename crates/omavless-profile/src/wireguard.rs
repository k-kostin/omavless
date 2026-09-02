// SPDX-License-Identifier: MIT

//! Strict, bounded WireGuard/AmneziaWG import primitives for the future Rust
//! runtime.
//!
//! The current Python runtime does not call this module yet.  Keeping parsing,
//! generation detection, private canonical state and Mihomo rendering together
//! prevents a second parser from growing in the compatibility layer.

use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;
use std::net::IpAddr;
use std::str::FromStr;

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use flate2::bufread::ZlibDecoder;

use crate::profile_uri::{canonical_host, parse_endpoint};
use crate::vless::HostKind;
use crate::vless_canonical::sha256_hex;

pub const MAX_WIREGUARD_CONFIG_BYTES: usize = 64 * 1024;
pub const MAX_VPN_LINK_BYTES: usize = 128 * 1024;
const MAX_CONFIG_LINES: usize = 512;
const MAX_LINE_BYTES: usize = 8 * 1024;
// Provider-generated split-tunnel configs routinely expand the complement of
// regional prefixes into several hundred AllowedIPs entries. Keep that valid
// use case while retaining a hard parser/memory bound.
const MAX_LIST_ITEMS: usize = 2_048;
const MAX_SPECIAL_JUNK_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireGuardError {
    InvalidInput,
    InputTooLarge,
    InvalidSection,
    DuplicateSection,
    MissingSection,
    ExtraPeer,
    InvalidLine,
    DuplicateField,
    UnsupportedField,
    DangerousDirective,
    MissingRequiredField,
    InvalidKey,
    InvalidAddress,
    TooManyAddresses,
    InvalidDns,
    InvalidMtu,
    InvalidAllowedIp,
    InvalidEndpoint,
    InvalidKeepalive,
    IncompleteAmnezia,
    MixedAmneziaGeneration,
    UnsupportedAmneziaGeneration,
    InvalidAmneziaValue,
    InvalidVpnLink,
    InvalidVpnEncoding,
    InvalidCompressedPayload,
    DecompressedTooLarge,
    UnsupportedVpnContainer,
}

impl WireGuardError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::InputTooLarge => "input_too_large",
            Self::InvalidSection => "invalid_section",
            Self::DuplicateSection => "duplicate_section",
            Self::MissingSection => "missing_section",
            Self::ExtraPeer => "extra_peer",
            Self::InvalidLine => "invalid_line",
            Self::DuplicateField => "duplicate_field",
            Self::UnsupportedField => "unsupported_field",
            Self::DangerousDirective => "dangerous_directive",
            Self::MissingRequiredField => "missing_required_field",
            Self::InvalidKey => "invalid_key",
            Self::InvalidAddress => "invalid_address",
            Self::TooManyAddresses => "too_many_addresses",
            Self::InvalidDns => "invalid_dns",
            Self::InvalidMtu => "invalid_mtu",
            Self::InvalidAllowedIp => "invalid_allowed_ip",
            Self::InvalidEndpoint => "invalid_endpoint",
            Self::InvalidKeepalive => "invalid_keepalive",
            Self::IncompleteAmnezia => "incomplete_amnezia_generation",
            Self::MixedAmneziaGeneration => "mixed_amnezia_generation",
            Self::UnsupportedAmneziaGeneration => "unsupported_amnezia_generation",
            Self::InvalidAmneziaValue => "invalid_amnezia_value",
            Self::InvalidVpnLink => "invalid_vpn_link",
            Self::InvalidVpnEncoding => "invalid_vpn_encoding",
            Self::InvalidCompressedPayload => "invalid_compressed_payload",
            Self::DecompressedTooLarge => "decompressed_payload_too_large",
            Self::UnsupportedVpnContainer => "unsupported_vpn_container",
        }
    }
}

impl fmt::Display for WireGuardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "WireGuard input is invalid",
            Self::InputTooLarge => "WireGuard input is too large",
            Self::InvalidSection => "WireGuard config contains an invalid section",
            Self::DuplicateSection => "WireGuard config contains duplicate sections",
            Self::MissingSection => "WireGuard config requires one Interface and one Peer section",
            Self::ExtraPeer => "WireGuard config contains more than one peer",
            Self::InvalidLine => "WireGuard config contains an invalid line",
            Self::DuplicateField => "WireGuard config contains duplicate fields",
            Self::UnsupportedField => "WireGuard config contains unsupported fields",
            Self::DangerousDirective => {
                "WireGuard config contains host-executable or routing directives"
            }
            Self::MissingRequiredField => "WireGuard config is missing a required field",
            Self::InvalidKey => "WireGuard config contains an invalid key",
            Self::InvalidAddress => "WireGuard config contains an invalid interface address",
            Self::TooManyAddresses => "WireGuard config contains too many interface addresses",
            Self::InvalidDns => "WireGuard config contains an invalid DNS value",
            Self::InvalidMtu => "WireGuard config contains an invalid MTU",
            Self::InvalidAllowedIp => "WireGuard config contains an invalid allowed IP prefix",
            Self::InvalidEndpoint => "WireGuard config contains an invalid peer endpoint",
            Self::InvalidKeepalive => "WireGuard config contains an invalid keepalive interval",
            Self::IncompleteAmnezia => "AmneziaWG parameters do not form a complete generation",
            Self::MixedAmneziaGeneration => "AmneziaWG config mixes incompatible generations",
            Self::UnsupportedAmneziaGeneration => "That AmneziaWG generation is not supported yet",
            Self::InvalidAmneziaValue => "AmneziaWG config contains an invalid parameter",
            Self::InvalidVpnLink => "AmneziaVPN guest link is invalid",
            Self::InvalidVpnEncoding => "AmneziaVPN guest link encoding is invalid",
            Self::InvalidCompressedPayload => {
                "AmneziaVPN guest payload is not a valid compressed config"
            }
            Self::DecompressedTooLarge => "AmneziaVPN guest payload expands beyond the safe limit",
            Self::UnsupportedVpnContainer => {
                "Structured AmneziaVPN containers are not supported by this adapter yet"
            }
        })
    }
}

impl std::error::Error for WireGuardError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireGuardFlavor {
    Standard,
    Amnezia(AwgGeneration),
}

impl WireGuardFlavor {
    #[must_use]
    pub const fn protocol_name(self) -> &'static str {
        match self {
            Self::Standard => "wireguard",
            Self::Amnezia(_) => "amneziawg",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AwgGeneration {
    V1,
    V2,
    V3,
    V3_1,
}

impl AwgGeneration {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V1 => "1",
            Self::V2 => "2",
            Self::V3 => "3",
            Self::V3_1 => "3.1",
        }
    }

    #[must_use]
    pub const fn mihomo_version(self) -> u8 {
        match self {
            Self::V3 | Self::V3_1 => 3,
            Self::V1 | Self::V2 => 0,
        }
    }

    /// Baseline verified by this adapter. This is deliberately not advertised
    /// as the first upstream release that ever implemented the generation.
    #[must_use]
    pub const fn verified_mihomo_baseline(self) -> &'static str {
        "1.19.30"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireGuardFacts {
    pub flavor: WireGuardFlavor,
    pub endpoint_kind: HostKind,
    pub endpoint_port: u16,
    pub has_ipv4: bool,
    pub has_ipv6: bool,
    pub dns_count: usize,
    pub allowed_ip_count: usize,
    pub has_preshared_key: bool,
    pub has_custom_mtu: bool,
    pub keepalive_range_normalized: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct IpPrefix {
    address: IpAddr,
    prefix: u8,
}

impl IpPrefix {
    fn parse(value: &str) -> Result<Self, ()> {
        let (address, prefix_text) = value.split_once('/').ok_or(())?;
        let address = IpAddr::from_str(address.trim()).map_err(|_| ())?;
        let prefix = prefix_text.trim().parse::<u8>().map_err(|_| ())?;
        let maximum = if address.is_ipv4() { 32 } else { 128 };
        if prefix > maximum {
            return Err(());
        }
        Ok(Self { address, prefix })
    }

    fn canonical(&self) -> String {
        format!("{}/{}", self.address, self.prefix)
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Endpoint {
    host: String,
    port: u16,
    kind: HostKind,
}

#[derive(Clone, PartialEq, Eq)]
struct AwgOptions {
    generation: AwgGeneration,
    values: BTreeMap<String, String>,
}

#[derive(Clone, PartialEq, Eq)]
enum Keepalive {
    Fixed(u16),
    /// AmneziaWG 3 accepts a randomized interval. Mihomo 1.19.30 exposes an
    /// integer here, so the renderer deterministically selects the lower safe
    /// bound and surfaces that fact in the public projection.
    Range {
        minimum: u16,
        maximum: u16,
    },
}

impl Keepalive {
    const fn rendered(&self) -> u16 {
        match self {
            Self::Fixed(value) => *value,
            Self::Range { minimum, .. } => *minimum,
        }
    }

    fn canonical(&self) -> String {
        match self {
            Self::Fixed(value) => value.to_string(),
            Self::Range { minimum, maximum } => format!("{minimum}-{maximum}"),
        }
    }
}

pub struct WireGuardProfile {
    private_key: String,
    addresses: Vec<IpPrefix>,
    dns: Vec<String>,
    mtu: Option<u16>,
    public_key: String,
    preshared_key: String,
    allowed_ips: Vec<IpPrefix>,
    endpoint: Endpoint,
    persistent_keepalive: Option<Keepalive>,
    awg: Option<AwgOptions>,
}

impl fmt::Debug for WireGuardProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WireGuardProfile")
            .field("facts", &self.facts())
            .field(
                "private_value_count",
                &(3 + usize::from(!self.preshared_key.is_empty())),
            )
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Interface,
    Peer,
}

struct ParsedSections {
    interface: BTreeMap<String, String>,
    peer: BTreeMap<String, String>,
}

const DANGEROUS_FIELDS: [&str; 7] = [
    "preup",
    "postup",
    "predown",
    "postdown",
    "table",
    "saveconfig",
    "fwmark",
];

const INTERFACE_FIELDS: [&str; 33] = [
    "privatekey",
    "address",
    "dns",
    "mtu",
    "jc",
    "jmin",
    "jmax",
    "s1",
    "s2",
    "s3",
    "s4",
    "h1",
    "h2",
    "h3",
    "h4",
    "i1",
    "i2",
    "i3",
    "i4",
    "i5",
    "j1",
    "j2",
    "j3",
    "itime",
    "headerprotectionkey",
    "contentpaddingaddition",
    "rekeyaftertime",
    "rekeytimeout",
    "rejectaftertime",
    "keepalivetimeout",
    "maxhandshakeattempts",
    "randomtrailers",
    "disablecookies",
];

const PEER_FIELDS: [&str; 5] = [
    "publickey",
    "presharedkey",
    "allowedips",
    "endpoint",
    "persistentkeepalive",
];

const AWG_BASE_FIELDS: [&str; 9] = ["jc", "jmin", "jmax", "s1", "s2", "h1", "h2", "h3", "h4"];
const AWG_EXTENDED_FIELDS: [&str; 7] = ["s3", "s4", "i1", "i2", "i3", "i4", "i5"];
const AWG_V1_5_ONLY_FIELDS: [&str; 4] = ["j1", "j2", "j3", "itime"];
const AWG_V3_FIELDS: [&str; 7] = [
    "headerprotectionkey",
    "contentpaddingaddition",
    "rekeyaftertime",
    "rekeytimeout",
    "rejectaftertime",
    "keepalivetimeout",
    "maxhandshakeattempts",
];
const AWG_V3_1_FIELDS: [&str; 2] = ["randomtrailers", "disablecookies"];

fn parse_sections(input: &str) -> Result<ParsedSections, WireGuardError> {
    if input.is_empty() || input.as_bytes().contains(&0) {
        return Err(WireGuardError::InvalidInput);
    }
    if input.len() > MAX_WIREGUARD_CONFIG_BYTES {
        return Err(WireGuardError::InputTooLarge);
    }

    let mut interface = BTreeMap::new();
    let mut peer = BTreeMap::new();
    let mut section = None;
    let mut saw_interface = false;
    let mut saw_peer = false;

    for (index, raw_line) in input.lines().enumerate() {
        if index >= MAX_CONFIG_LINES || raw_line.len() > MAX_LINE_BYTES {
            return Err(WireGuardError::InputTooLarge);
        }
        let mut line = raw_line.trim();
        if index == 0 {
            line = line.strip_prefix('\u{feff}').unwrap_or(line).trim_start();
        }
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') || line.ends_with(']') {
            section = match line.to_ascii_lowercase().as_str() {
                "[interface]" if !saw_interface && !saw_peer => {
                    saw_interface = true;
                    Some(Section::Interface)
                }
                "[interface]" => return Err(WireGuardError::DuplicateSection),
                "[peer]" if saw_interface && !saw_peer => {
                    saw_peer = true;
                    Some(Section::Peer)
                }
                "[peer]" if saw_peer => return Err(WireGuardError::ExtraPeer),
                "[peer]" => return Err(WireGuardError::InvalidSection),
                _ => return Err(WireGuardError::InvalidSection),
            };
            continue;
        }

        let active = section.ok_or(WireGuardError::InvalidSection)?;
        let (name, value) = line.split_once('=').ok_or(WireGuardError::InvalidLine)?;
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        if name.is_empty()
            || value.bytes().any(|byte| byte.is_ascii_control())
            || DANGEROUS_FIELDS.contains(&name.as_str())
        {
            return if DANGEROUS_FIELDS.contains(&name.as_str()) {
                Err(WireGuardError::DangerousDirective)
            } else {
                Err(WireGuardError::InvalidLine)
            };
        }
        let accepted = match active {
            Section::Interface => INTERFACE_FIELDS.contains(&name.as_str()),
            Section::Peer => PEER_FIELDS.contains(&name.as_str()),
        };
        if !accepted {
            return Err(WireGuardError::UnsupportedField);
        }
        let target = match active {
            Section::Interface => &mut interface,
            Section::Peer => &mut peer,
        };
        if target.insert(name, value.to_owned()).is_some() {
            return Err(WireGuardError::DuplicateField);
        }
    }

    if !saw_interface || !saw_peer {
        return Err(WireGuardError::MissingSection);
    }
    Ok(ParsedSections { interface, peer })
}

fn required<'a>(
    fields: &'a BTreeMap<String, String>,
    name: &str,
) -> Result<&'a str, WireGuardError> {
    fields
        .get(name)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(WireGuardError::MissingRequiredField)
}

fn wireguard_key(value: &str) -> Result<String, WireGuardError> {
    if value.len() != 44 || !value.ends_with('=') {
        return Err(WireGuardError::InvalidKey);
    }
    let decoded = STANDARD
        .decode(value)
        .map_err(|_| WireGuardError::InvalidKey)?;
    if decoded.len() != 32 || STANDARD.encode(decoded) != value {
        return Err(WireGuardError::InvalidKey);
    }
    Ok(value.to_owned())
}

fn comma_list(value: &str, error: WireGuardError) -> Result<Vec<&str>, WireGuardError> {
    let values = value.split(',').map(str::trim).collect::<Vec<_>>();
    if values.is_empty()
        || values.len() > MAX_LIST_ITEMS
        || values.iter().any(|item| item.is_empty())
    {
        return Err(error);
    }
    Ok(values)
}

fn ip_prefixes(value: &str, error: WireGuardError) -> Result<Vec<IpPrefix>, WireGuardError> {
    comma_list(value, error)?
        .into_iter()
        .map(|item| IpPrefix::parse(item).map_err(|()| error))
        .collect()
}

fn parse_addresses(value: &str) -> Result<Vec<IpPrefix>, WireGuardError> {
    let addresses = ip_prefixes(value, WireGuardError::InvalidAddress)?;
    let ipv4 = addresses
        .iter()
        .filter(|item| item.address.is_ipv4())
        .count();
    let ipv6 = addresses
        .iter()
        .filter(|item| item.address.is_ipv6())
        .count();
    if ipv4 > 1 || ipv6 > 1 {
        return Err(WireGuardError::TooManyAddresses);
    }
    Ok(addresses)
}

fn parse_dns(value: Option<&String>) -> Result<Vec<String>, WireGuardError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    comma_list(value, WireGuardError::InvalidDns)?
        .into_iter()
        .map(|item| {
            IpAddr::from_str(item)
                .map(|address| address.to_string())
                .map_err(|_| WireGuardError::InvalidDns)
        })
        .collect()
}

fn parse_mtu(value: Option<&String>) -> Result<Option<u16>, WireGuardError> {
    let Some(value) = value else {
        return Ok(None);
    };
    value
        .parse::<u16>()
        .ok()
        .filter(|value| (576..=9_000).contains(value))
        .map(Some)
        .ok_or(WireGuardError::InvalidMtu)
}

fn parse_keepalive(
    value: Option<&String>,
    allow_range: bool,
) -> Result<Option<Keepalive>, WireGuardError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if let Some((minimum, maximum)) = value.split_once('-') {
        let minimum = minimum.trim().parse::<u16>().ok();
        let maximum = maximum.trim().parse::<u16>().ok();
        return match (allow_range, minimum, maximum) {
            (true, Some(minimum), Some(maximum)) if minimum != 0 && minimum <= maximum => {
                Ok(Some(Keepalive::Range { minimum, maximum }))
            }
            _ => Err(WireGuardError::InvalidKeepalive),
        };
    }
    value
        .parse::<u16>()
        .ok()
        .filter(|value| *value != 0)
        .map(Keepalive::Fixed)
        .map(Some)
        .ok_or(WireGuardError::InvalidKeepalive)
}

fn present(fields: &BTreeMap<String, String>, names: &[&str]) -> bool {
    names
        .iter()
        .any(|name| fields.get(*name).is_some_and(|value| !value.is_empty()))
}

fn all_present(fields: &BTreeMap<String, String>, names: &[&str]) -> bool {
    names
        .iter()
        .all(|name| fields.get(*name).is_some_and(|value| !value.is_empty()))
}

fn unsigned(value: &str) -> Result<(), WireGuardError> {
    value
        .parse::<u32>()
        .ok()
        .filter(|number| *number <= 1_000_000)
        .map(|_| ())
        .ok_or(WireGuardError::InvalidAmneziaValue)
}

fn numeric_range(value: &str) -> Result<(), WireGuardError> {
    if let Some((minimum, maximum)) = value.split_once('-') {
        let minimum = minimum
            .trim()
            .parse::<u32>()
            .map_err(|_| WireGuardError::InvalidAmneziaValue)?;
        let maximum = maximum
            .trim()
            .parse::<u32>()
            .map_err(|_| WireGuardError::InvalidAmneziaValue)?;
        if minimum > maximum || maximum > 1_000_000 {
            return Err(WireGuardError::InvalidAmneziaValue);
        }
        Ok(())
    } else {
        unsigned(value)
    }
}

fn boolean(value: &str) -> Result<bool, WireGuardError> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(WireGuardError::InvalidAmneziaValue),
    }
}

fn awg_options(fields: &BTreeMap<String, String>) -> Result<Option<AwgOptions>, WireGuardError> {
    let has_base = present(fields, &AWG_BASE_FIELDS);
    let has_extended = present(fields, &AWG_EXTENDED_FIELDS);
    let has_v1_5_only = present(fields, &AWG_V1_5_ONLY_FIELDS);
    let has_v3 = present(fields, &AWG_V3_FIELDS);
    let has_v3_1 = present(fields, &AWG_V3_1_FIELDS);
    if !(has_base || has_extended || has_v1_5_only || has_v3 || has_v3_1) {
        return Ok(None);
    }
    if !all_present(fields, &AWG_BASE_FIELDS) {
        return Err(WireGuardError::IncompleteAmnezia);
    }
    let has_s3_s4 = present(fields, &AWG_EXTENDED_FIELDS[..2]);
    let has_special_junk = present(fields, &AWG_EXTENDED_FIELDS[2..]);
    let looks_like_v1_5 = has_v1_5_only || (has_special_junk && !has_s3_s4 && !has_v3 && !has_v3_1);
    if looks_like_v1_5 {
        return if has_v3 || has_v3_1 {
            Err(WireGuardError::MixedAmneziaGeneration)
        } else {
            Err(WireGuardError::UnsupportedAmneziaGeneration)
        };
    }
    if has_v3 || has_v3_1 {
        if !all_present(fields, &AWG_EXTENDED_FIELDS[..2]) || !all_present(fields, &AWG_V3_FIELDS) {
            return Err(WireGuardError::IncompleteAmnezia);
        }
    } else if has_extended && !all_present(fields, &AWG_EXTENDED_FIELDS[..2]) {
        return Err(WireGuardError::IncompleteAmnezia);
    }

    for name in ["jc", "jmin", "jmax", "s1", "s2", "s3", "s4"] {
        if let Some(value) = fields.get(name) {
            unsigned(value)?;
        }
    }
    for name in ["h1", "h2", "h3", "h4"] {
        numeric_range(required(fields, name)?)?;
    }
    for name in ["i1", "i2", "i3", "i4", "i5"] {
        if fields
            .get(name)
            .is_some_and(|value| value.len() > MAX_SPECIAL_JUNK_BYTES)
        {
            return Err(WireGuardError::InvalidAmneziaValue);
        }
    }
    if let Some(value) = fields.get("headerprotectionkey") {
        wireguard_key(value)?;
    }
    for name in [
        "contentpaddingaddition",
        "rekeyaftertime",
        "rekeytimeout",
        "rejectaftertime",
        "keepalivetimeout",
        "maxhandshakeattempts",
    ] {
        if let Some(value) = fields.get(name) {
            numeric_range(value)?;
        }
    }
    for name in AWG_V3_1_FIELDS {
        if let Some(value) = fields.get(name) {
            boolean(value)?;
        }
    }

    let generation = if has_v3_1 {
        AwgGeneration::V3_1
    } else if has_v3 {
        AwgGeneration::V3
    } else if has_extended
        || ["h1", "h2", "h3", "h4"]
            .iter()
            .any(|name| fields.get(*name).is_some_and(|value| value.contains('-')))
    {
        AwgGeneration::V2
    } else {
        AwgGeneration::V1
    };

    let mut values = BTreeMap::new();
    for name in AWG_BASE_FIELDS
        .into_iter()
        .chain(AWG_EXTENDED_FIELDS)
        .chain(AWG_V3_FIELDS)
        .chain(AWG_V3_1_FIELDS)
    {
        if let Some(value) = fields.get(name).filter(|value| !value.is_empty()) {
            values.insert(name.to_owned(), value.clone());
        }
    }
    Ok(Some(AwgOptions { generation, values }))
}

pub fn parse_wireguard_config(input: &str) -> Result<WireGuardProfile, WireGuardError> {
    let sections = parse_sections(input)?;
    let private_key = wireguard_key(required(&sections.interface, "privatekey")?)?;
    let addresses = parse_addresses(required(&sections.interface, "address")?)?;
    let dns = parse_dns(sections.interface.get("dns"))?;
    let mtu = parse_mtu(sections.interface.get("mtu"))?;
    let public_key = wireguard_key(required(&sections.peer, "publickey")?)?;
    let preshared_key = sections
        .peer
        .get("presharedkey")
        .map_or(Ok(String::new()), |value| wireguard_key(value))?;
    let allowed_ips = ip_prefixes(
        required(&sections.peer, "allowedips")?,
        WireGuardError::InvalidAllowedIp,
    )?;
    let endpoint_text = required(&sections.peer, "endpoint")?;
    let (host, port, kind) =
        parse_endpoint(endpoint_text, None).map_err(|()| WireGuardError::InvalidEndpoint)?;
    let (host, canonical_kind) =
        canonical_host(&host).map_err(|()| WireGuardError::InvalidEndpoint)?;
    if kind != canonical_kind {
        return Err(WireGuardError::InvalidEndpoint);
    }
    let awg = awg_options(&sections.interface)?;
    let persistent_keepalive =
        parse_keepalive(sections.peer.get("persistentkeepalive"), awg.is_some())?;

    Ok(WireGuardProfile {
        private_key,
        addresses,
        dns,
        mtu,
        public_key,
        preshared_key,
        allowed_ips,
        endpoint: Endpoint { host, port, kind },
        persistent_keepalive,
        awg,
    })
}

fn decode_urlsafe_payload(value: &str) -> Result<Vec<u8>, WireGuardError> {
    let unpadded = value.trim_end_matches('=');
    if unpadded.is_empty()
        || value.len() - unpadded.len() > 2
        || value[..unpadded.len()]
            .bytes()
            .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')))
        || value[unpadded.len()..].bytes().any(|byte| byte != b'=')
    {
        return Err(WireGuardError::InvalidVpnEncoding);
    }
    URL_SAFE_NO_PAD
        .decode(unpadded)
        .map_err(|_| WireGuardError::InvalidVpnEncoding)
}

fn decode_amnezia_vpn_link(input: &str) -> Result<String, WireGuardError> {
    if input.len() > MAX_VPN_LINK_BYTES {
        return Err(WireGuardError::InputTooLarge);
    }
    let input = input.trim();
    let payload = input
        .get(..6)
        .filter(|prefix| prefix.eq_ignore_ascii_case("vpn://"))
        .and_then(|_| input.get(6..))
        .ok_or(WireGuardError::InvalidVpnLink)?;
    if payload.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(WireGuardError::InvalidVpnEncoding);
    }
    let compressed = decode_urlsafe_payload(payload)?;
    if compressed.len() < 6 {
        return Err(WireGuardError::InvalidCompressedPayload);
    }
    let declared = u32::from_be_bytes(
        compressed[..4]
            .try_into()
            .map_err(|_| WireGuardError::InvalidCompressedPayload)?,
    ) as usize;
    if declared > MAX_WIREGUARD_CONFIG_BYTES {
        return Err(WireGuardError::DecompressedTooLarge);
    }
    let mut decoder = ZlibDecoder::new(&compressed[4..]);
    let mut decoded = Vec::with_capacity(declared.min(MAX_WIREGUARD_CONFIG_BYTES));
    decoder
        .by_ref()
        .take((MAX_WIREGUARD_CONFIG_BYTES + 1) as u64)
        .read_to_end(&mut decoded)
        .map_err(|_| WireGuardError::InvalidCompressedPayload)?;
    if decoded.len() > MAX_WIREGUARD_CONFIG_BYTES {
        return Err(WireGuardError::DecompressedTooLarge);
    }
    if decoded.len() != declared || !decoder.into_inner().is_empty() {
        return Err(WireGuardError::InvalidCompressedPayload);
    }
    String::from_utf8(decoded).map_err(|_| WireGuardError::InvalidCompressedPayload)
}

pub fn parse_amnezia_vpn_link(input: &str) -> Result<WireGuardProfile, WireGuardError> {
    let decoded = decode_amnezia_vpn_link(input)?;
    if decoded.trim_start().starts_with('{') {
        return Err(WireGuardError::UnsupportedVpnContainer);
    }
    parse_wireguard_config(&decoded)
}

fn yaml_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing an in-memory string cannot fail")
}

fn yaml_list(values: impl IntoIterator<Item = String>) -> String {
    let joined = values
        .into_iter()
        .map(|value| yaml_string(&value))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{joined}]")
}

fn mihomo_awg_name(name: &str) -> &'static str {
    match name {
        "jc" => "jc",
        "jmin" => "jmin",
        "jmax" => "jmax",
        "s1" => "s1",
        "s2" => "s2",
        "s3" => "s3",
        "s4" => "s4",
        "h1" => "h1",
        "h2" => "h2",
        "h3" => "h3",
        "h4" => "h4",
        "i1" => "i1",
        "i2" => "i2",
        "i3" => "i3",
        "i4" => "i4",
        "i5" => "i5",
        "headerprotectionkey" => "header-protection-key",
        "contentpaddingaddition" => "content-padding-addition",
        "rekeyaftertime" => "rekey-after-time",
        "rekeytimeout" => "rekey-timeout",
        "rejectaftertime" => "reject-after-time",
        "keepalivetimeout" => "keepalive-timeout",
        "maxhandshakeattempts" => "max-handshake-attempts",
        "randomtrailers" => "random-trailers",
        "disablecookies" => "disable-cookies",
        _ => unreachable!("only validated AWG fields reach the renderer"),
    }
}

impl WireGuardProfile {
    #[must_use]
    pub fn facts(&self) -> WireGuardFacts {
        WireGuardFacts {
            flavor: self
                .awg
                .as_ref()
                .map_or(WireGuardFlavor::Standard, |options| {
                    WireGuardFlavor::Amnezia(options.generation)
                }),
            endpoint_kind: self.endpoint.kind,
            endpoint_port: self.endpoint.port,
            has_ipv4: self.addresses.iter().any(|item| item.address.is_ipv4()),
            has_ipv6: self.addresses.iter().any(|item| item.address.is_ipv6()),
            dns_count: self.dns.len(),
            allowed_ip_count: self.allowed_ips.len(),
            has_preshared_key: !self.preshared_key.is_empty(),
            has_custom_mtu: self.mtu.is_some(),
            keepalive_range_normalized: matches!(
                self.persistent_keepalive.as_ref(),
                Some(Keepalive::Range { .. })
            ),
        }
    }

    #[must_use]
    pub fn subscription_identity(&self) -> String {
        let mut values = vec![
            self.private_key.clone(),
            self.public_key.clone(),
            self.preshared_key.clone(),
            self.endpoint.host.clone(),
            self.endpoint.port.to_string(),
        ];
        values.extend(self.addresses.iter().map(IpPrefix::canonical));
        values.extend(self.allowed_ips.iter().map(IpPrefix::canonical));
        values.extend(self.dns.iter().cloned());
        if let Some(mtu) = self.mtu {
            values.push(mtu.to_string());
        }
        if let Some(keepalive) = &self.persistent_keepalive {
            values.push(keepalive.canonical());
        }
        if let Some(awg) = &self.awg {
            values.push(awg.generation.as_str().to_owned());
            values.extend(
                awg.values
                    .iter()
                    .map(|(name, value)| format!("{name}={value}")),
            );
        }
        sha256_hex(values.join("\0").as_bytes())
    }

    #[must_use]
    pub fn render_mihomo_proxy(&self, name: &str, server_override: Option<&str>) -> String {
        let mut lines = vec![
            format!("- name: {}", yaml_string(name)),
            "  type: wireguard".to_owned(),
            format!(
                "  server: {}",
                yaml_string(server_override.unwrap_or(&self.endpoint.host))
            ),
            format!("  port: {}", self.endpoint.port),
            format!("  private-key: {}", yaml_string(&self.private_key)),
            format!("  public-key: {}", yaml_string(&self.public_key)),
            "  udp: true".to_owned(),
        ];
        if let Some(ipv4) = self.addresses.iter().find(|item| item.address.is_ipv4()) {
            lines.push(format!("  ip: {}", yaml_string(&ipv4.canonical())));
        }
        if let Some(ipv6) = self.addresses.iter().find(|item| item.address.is_ipv6()) {
            lines.push(format!("  ipv6: {}", yaml_string(&ipv6.canonical())));
        }
        if !self.preshared_key.is_empty() {
            lines.push(format!(
                "  pre-shared-key: {}",
                yaml_string(&self.preshared_key)
            ));
        }
        lines.push(format!(
            "  allowed-ips: {}",
            yaml_list(self.allowed_ips.iter().map(IpPrefix::canonical))
        ));
        if !self.dns.is_empty() {
            lines.push(format!("  dns: {}", yaml_list(self.dns.iter().cloned())));
            lines.push("  remote-dns-resolve: true".to_owned());
        }
        if let Some(mtu) = self.mtu {
            lines.push(format!("  mtu: {mtu}"));
        }
        if let Some(keepalive) = &self.persistent_keepalive {
            lines.push(format!("  persistent-keepalive: {}", keepalive.rendered()));
        }
        if let Some(awg) = &self.awg {
            lines.push("  amnezia-wg-option:".to_owned());
            if awg.generation.mihomo_version() != 0 {
                lines.push(format!("    version: {}", awg.generation.mihomo_version()));
            }
            for (field, value) in &awg.values {
                let output = mihomo_awg_name(field);
                let rendered = if AWG_V3_1_FIELDS.contains(&field.as_str()) {
                    boolean(value)
                        .expect("AWG booleans were validated")
                        .to_string()
                } else if ["jc", "jmin", "jmax", "s1", "s2", "s3", "s4"].contains(&field.as_str()) {
                    value.clone()
                } else {
                    yaml_string(value)
                };
                lines.push(format!("    {output}: {rendered}"));
            }
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write as _;

    const PRIVATE_KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";
    const PUBLIC_KEY: &str = "ICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj8=";
    const PSK: &str = "QEFCQ0RFRkdISUpLTE1OT1BRUlNUVVZXWFlaW1xdXl8=";
    const HEADER_KEY: &str = "YGFiY2RlZmdoaWprbG1ub3BxcnN0dXZ3eHl6e3x9fn8=";

    fn standard() -> String {
        format!(
            "[Interface]\nPrivateKey = {PRIVATE_KEY}\nAddress = 10.0.0.2/32, fd00::2/128\nDNS = 1.1.1.1, 2606:4700:4700::1111\nMTU = 1420\n\n[Peer]\nPublicKey = {PUBLIC_KEY}\nPresharedKey = {PSK}\nAllowedIPs = 0.0.0.0/0, ::/0\nEndpoint = [2001:db8::1]:51820\nPersistentKeepalive = 25\n"
        )
    }

    fn awg3() -> String {
        format!(
            "[Interface]\nPrivateKey = {PRIVATE_KEY}\nAddress = 10.8.0.2/32\nJc = 4\nJmin = 10\nJmax = 30\nS1 = 20\nS2 = 25\nS3 = 30\nS4 = 35\nH1 = 100\nH2 = 200\nH3 = 300\nH4 = 400\nI1 = <r 2><b 0x0102>\nHeaderProtectionKey = {HEADER_KEY}\nContentPaddingAddition = 10-100\nRekeyAfterTime = 100-120\nRekeyTimeout = 3-7\nRejectAfterTime = 150-180\nKeepaliveTimeout = 5-15\nMaxHandshakeAttempts = 15-20\n\n[Peer]\nPublicKey = {PUBLIC_KEY}\nAllowedIPs = 0.0.0.0/0\nEndpoint = awg.example.invalid:443\nPersistentKeepalive = 25-35\n"
        )
    }

    fn awg_base(extra: &str) -> String {
        format!(
            "[Interface]\nPrivateKey = {PRIVATE_KEY}\nAddress = 10.8.0.2/32\nJc = 4\nJmin = 10\nJmax = 30\nS1 = 20\nS2 = 25\nH1 = 100\nH2 = 200\nH3 = 300\nH4 = 400\n{extra}[Peer]\nPublicKey = {PUBLIC_KEY}\nAllowedIPs = 0.0.0.0/0\nEndpoint = awg.example.invalid:443\n"
        )
    }

    fn vpn_link(config: &str) -> String {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(8));
        encoder.write_all(config.as_bytes()).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut qcompressed = (config.len() as u32).to_be_bytes().to_vec();
        qcompressed.extend(compressed);
        format!("vpn://{}", URL_SAFE_NO_PAD.encode(qcompressed))
    }

    #[test]
    fn parses_standard_one_peer_config_and_renders_mihomo() {
        let profile = parse_wireguard_config(&standard()).unwrap();
        assert_eq!(profile.facts().flavor, WireGuardFlavor::Standard);
        assert_eq!(profile.facts().endpoint_kind, HostKind::Ipv6);
        assert!(profile.facts().has_ipv4);
        assert!(profile.facts().has_ipv6);
        assert_eq!(profile.facts().dns_count, 2);
        let yaml = profile.render_mihomo_proxy("WG node", None);
        assert!(yaml.contains("type: wireguard"));
        assert!(yaml.contains("allowed-ips: [\"0.0.0.0/0\", \"::/0\"]"));
        assert!(yaml.contains("remote-dns-resolve: true"));
        assert!(!yaml.contains("amnezia-wg-option"));
        assert_eq!(profile.subscription_identity().len(), 64);
    }

    #[test]
    fn detects_awg3_and_maps_verified_mihomo_fields() {
        let profile = parse_wireguard_config(&awg3()).unwrap();
        assert_eq!(
            profile.facts().flavor,
            WireGuardFlavor::Amnezia(AwgGeneration::V3)
        );
        assert!(profile.facts().keepalive_range_normalized);
        let yaml = profile.render_mihomo_proxy("AWG node", None);
        for expected in [
            "amnezia-wg-option:",
            "version: 3",
            "header-protection-key:",
            "content-padding-addition: \"10-100\"",
            "max-handshake-attempts: \"15-20\"",
            "persistent-keepalive: 25",
        ] {
            assert!(yaml.contains(expected));
        }
    }

    #[test]
    fn distinguishes_supported_awg_generations_and_rejects_unverified_v1_5() {
        let cases = [
            (awg_base(""), AwgGeneration::V1),
            (awg_base("S3 = 30\nS4 = 35\n"), AwgGeneration::V2),
            (awg3(), AwgGeneration::V3),
            (
                awg3().replace(
                    "[Peer]",
                    "RandomTrailers = on\nDisableCookies = off\n[Peer]",
                ),
                AwgGeneration::V3_1,
            ),
        ];
        for (config, expected) in cases {
            assert_eq!(
                parse_wireguard_config(&config).unwrap().facts().flavor,
                WireGuardFlavor::Amnezia(expected)
            );
        }
        assert_eq!(
            parse_wireguard_config(&awg_base("I1 = <b 0x01>\n")).unwrap_err(),
            WireGuardError::UnsupportedAmneziaGeneration
        );
    }

    #[test]
    fn qcompress_guest_link_round_trips_into_the_same_strict_parser() {
        let config = awg3();
        let link = vpn_link(&config);
        assert_eq!(decode_amnezia_vpn_link(&link).unwrap(), config);
        assert_eq!(
            parse_amnezia_vpn_link(&link).unwrap().facts().flavor,
            WireGuardFlavor::Amnezia(AwgGeneration::V3)
        );
    }

    #[test]
    fn structured_guest_container_is_explicitly_deferred() {
        let link = vpn_link("{\"containers\":[]}");
        assert_eq!(
            parse_amnezia_vpn_link(&link).unwrap_err(),
            WireGuardError::UnsupportedVpnContainer
        );
    }

    #[test]
    fn dangerous_directives_duplicates_and_extra_peers_fail_closed() {
        for injected in [
            "PostUp = private-command --secret\n",
            "PrivateKey = duplicate-private-key\n",
        ] {
            let input = standard().replace("[Interface]\n", &format!("[Interface]\n{injected}"));
            let error = parse_wireguard_config(&input).unwrap_err();
            assert!(matches!(
                error,
                WireGuardError::DangerousDirective | WireGuardError::DuplicateField
            ));
            assert!(!error.to_string().contains("private-command"));
        }
        let extra = format!("{}\n[Peer]\nPublicKey = {PUBLIC_KEY}\n", standard());
        assert_eq!(
            parse_wireguard_config(&extra).unwrap_err(),
            WireGuardError::ExtraPeer
        );
    }

    #[test]
    fn large_split_tunnel_lists_are_bounded_without_becoming_host_commands() {
        let prefixes = (0..425)
            .map(|index| format!("10.{}.{}.0/24", index / 256, index % 256))
            .collect::<Vec<_>>()
            .join(", ");
        let input = standard().replace("0.0.0.0/0, ::/0", &prefixes);
        let profile = parse_wireguard_config(&input).unwrap();
        assert_eq!(profile.facts().allowed_ip_count, 425);

        let excessive = (0..=MAX_LIST_ITEMS)
            .map(|_| "0.0.0.0/0")
            .collect::<Vec<_>>()
            .join(",");
        let input = standard().replace("0.0.0.0/0, ::/0", &excessive);
        assert!(matches!(
            parse_wireguard_config(&input).unwrap_err(),
            WireGuardError::InputTooLarge | WireGuardError::InvalidAllowedIp
        ));
    }

    #[test]
    fn yaml_control_values_are_quoted_and_dns_search_domains_reject() {
        let profile = parse_wireguard_config(&standard()).unwrap();
        let yaml = profile.render_mihomo_proxy("node:\n- injected", Some("override.example"));
        assert!(yaml.starts_with("- name: \"node:\\n- injected\""));
        assert!(yaml.contains("server: \"override.example\""));

        let search_domain = standard().replace(
            "1.1.1.1, 2606:4700:4700::1111",
            "1.1.1.1, search.example.invalid",
        );
        assert_eq!(
            parse_wireguard_config(&search_domain).unwrap_err(),
            WireGuardError::InvalidDns
        );
    }

    #[test]
    fn incomplete_or_mixed_awg_generations_fail_closed() {
        let incomplete = awg3().replace("HeaderProtectionKey = ", "# HeaderProtectionKey = ");
        assert_eq!(
            parse_wireguard_config(&incomplete).unwrap_err(),
            WireGuardError::IncompleteAmnezia
        );
        let mixed = awg3().replace("[Peer]", "J1 = <b 0x01>\n[Peer]");
        assert_eq!(
            parse_wireguard_config(&mixed).unwrap_err(),
            WireGuardError::MixedAmneziaGeneration
        );
    }

    #[test]
    fn debug_and_errors_do_not_disclose_private_values() {
        let profile = parse_wireguard_config(&standard()).unwrap();
        let debug = format!("{profile:?}");
        for secret in [PRIVATE_KEY, PUBLIC_KEY, PSK, "2001:db8::1"] {
            assert!(!debug.contains(secret));
        }
        let error = parse_wireguard_config("private-secret").unwrap_err();
        assert!(!error.to_string().contains("private-secret"));
    }

    #[test]
    fn guest_decoder_enforces_declared_and_actual_size_bounds() {
        let mut malformed = vec![0, 1, 0, 1, 0x78, 0x9c];
        let link = format!("vpn://{}", URL_SAFE_NO_PAD.encode(&malformed));
        assert_eq!(
            decode_amnezia_vpn_link(&link).unwrap_err(),
            WireGuardError::DecompressedTooLarge
        );
        malformed[..4].copy_from_slice(&1_u32.to_be_bytes());
        let link = format!("vpn://{}", URL_SAFE_NO_PAD.encode(malformed));
        assert_eq!(
            decode_amnezia_vpn_link(&link).unwrap_err(),
            WireGuardError::InvalidCompressedPayload
        );

        let original = vpn_link(&standard());
        let mut bytes = URL_SAFE_NO_PAD.decode(&original[6..]).unwrap();
        bytes.push(0);
        let trailing = format!("vpn://{}", URL_SAFE_NO_PAD.encode(bytes));
        assert_eq!(
            decode_amnezia_vpn_link(&trailing).unwrap_err(),
            WireGuardError::InvalidCompressedPayload
        );
    }
}
