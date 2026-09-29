//! Offline nft JSON candidate. Nothing in this module executes a command.
//! Constants are reserved for K1, not assertions about today's installed core.
use crate::{
    policy::{Policy, TABLE_FAMILY, TABLE_NAME},
    transaction::Table,
};
use serde::{
    Deserialize,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Value, json};
use std::fmt;

pub const CHAIN: &str = "output_guard";
pub const TUN: &str = "omavless0";
pub const CORE_MARK: u32 = 0x4f4d4101;
pub const PRIORITY: i32 = 300;
pub const MAX_READBACK_BYTES: usize = 32 * 1024;

fn eq(left: Value, right: Value) -> Value {
    json!({"match":{"op":"==","left":left,"right":right}})
}
fn meta(key: &str) -> Value {
    json!({"meta":{"key":key}})
}
fn payload(protocol: &str, field: &str) -> Value {
    json!({"payload":{"protocol":protocol,"field":field}})
}
fn prefix(addr: &str, len: u8) -> Value {
    json!({"prefix":{"addr":addr,"len":len}})
}
fn rule(mut expr: Vec<Value>) -> Value {
    expr.push(json!({"accept":null}));
    json!({"rule":{"family":TABLE_FAMILY,"table":TABLE_NAME,"chain":CHAIN,"expr":expr}})
}

// All inputs below are compiled literals. No caller-selected endpoint, LAN,
// expression, interface, mark or command is accepted by the renderer.
fn objects(policy: Policy) -> Vec<Value> {
    let mut result = vec![
        json!({"table":{"family":TABLE_FAMILY,"name":TABLE_NAME}}),
        json!({"chain":{"family":TABLE_FAMILY,"table":TABLE_NAME,"name":CHAIN,
            "type":"filter","hook":"output","prio":PRIORITY,"policy":"drop"}}),
        rule(vec![eq(meta("oifname"), json!("lo"))]),
    ];
    if policy == Policy::FullVpn {
        result.push(rule(vec![eq(meta("oifname"), json!(TUN))]));
        result.push(rule(vec![eq(meta("mark"), json!(CORE_MARK))]));
        // Initial/rebinding DHCP broadcast only. Unicast renewals are purposely
        // not widened to every server address; availability is an open host gate.
        result.push(rule(vec![
            eq(meta("nfproto"), json!("ipv4")),
            eq(payload("ip", "daddr"), json!("255.255.255.255")),
            eq(payload("udp", "sport"), json!(68)),
            eq(payload("udp", "dport"), json!(67)),
        ]));
        result.push(rule(vec![
            eq(meta("nfproto"), json!("ipv6")),
            eq(payload("ip6", "saddr"), prefix("fe80::", 10)),
            eq(payload("ip6", "daddr"), json!("ff02::1:2")),
            eq(payload("udp", "sport"), json!(546)),
            eq(payload("udp", "dport"), json!(547)),
        ]));
        // Host router solicitation, including the unspecified initial source.
        for source in [json!("::"), prefix("fe80::", 10)] {
            result.push(rule(vec![
                eq(meta("nfproto"), json!("ipv6")),
                eq(payload("ip6", "saddr"), source),
                eq(payload("ip6", "daddr"), json!("ff02::2")),
                eq(payload("ip6", "hoplimit"), json!(255)),
                eq(payload("icmpv6", "type"), json!(133)),
                eq(payload("icmpv6", "code"), json!(0)),
            ]));
        }
        // Neighbor solicitation/advertisement, with hop-limit/code constraints.
        // Unicast NUD and global-source NA need connected-link facts: omit rather
        // than silently permitting ICMPv6 to every destination.
        result.push(rule(vec![
            eq(meta("nfproto"), json!("ipv6")),
            eq(payload("ip6", "daddr"), prefix("ff02::1:ff00:0", 104)),
            eq(payload("ip6", "hoplimit"), json!(255)),
            eq(payload("icmpv6", "type"), json!(135)),
            eq(payload("icmpv6", "code"), json!(0)),
        ]));
        result.push(rule(vec![
            eq(meta("nfproto"), json!("ipv6")),
            eq(payload("ip6", "saddr"), prefix("fe80::", 10)),
            eq(payload("ip6", "daddr"), prefix("fe80::", 10)),
            eq(payload("ip6", "hoplimit"), json!(255)),
            eq(payload("icmpv6", "type"), json!(136)),
            eq(payload("icmpv6", "code"), json!(0)),
        ]));
    }
    result.push(
        json!({"rule":{"family":TABLE_FAMILY,"table":TABLE_NAME,"chain":CHAIN,
        "expr":[{"drop":null}]}}),
    );
    result
}

// nft 1.1.7's installed numeric readback omits the redundant leading nfproto
// matches in our six IP-payload-specific rules. Build a SECOND fixed expected
// template, not a normalizer over untrusted input: every address, protocol,
// constraint, action and rule position must still match in full. The retained
// ip/ip6 payloads imply exactly the omitted IPv4/IPv6 family dependency.
fn kernel_objects(policy: Policy) -> Vec<Value> {
    let mut result = objects(policy);
    if policy == Policy::FullVpn {
        for rule in &mut result[5..11] {
            rule["rule"]["expr"].as_array_mut().unwrap().remove(0);
        }
    }
    result
}

/// One create-if-absent transaction. Replace/delete require a future locked,
/// ownership-verifying executor and deliberately have no renderer here.
pub fn render_create(policy: Policy) -> Vec<u8> {
    let commands: Vec<Value> = objects(policy)
        .into_iter()
        .enumerate()
        .map(|(index, object)| {
            if index == 0 {
                json!({"create":object})
            } else {
                json!({"add":object})
            }
        })
        .collect();
    serde_json::to_vec(&json!({"nftables":commands})).expect("fixed JSON")
}

/// Identity supplied ONLY by the future trusted root adapter after exclusive
/// creation and verified root-receipt persistence. Parsing a ruleset, a comment
/// or a fixed name cannot establish this evidence. No IPC/Serde implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrustedTableIdentity {
    pub boot: [u8; 16],
    pub netns_inode: u64,
    pub table_handle: u64,
}

struct Strict(Value);
impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Strict;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("bounded nft JSON")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Strict, A::Error> {
                let mut out = serde_json::Map::new();
                while let Some(k) = map.next_key::<String>()? {
                    if out.contains_key(&k) {
                        return Err(de::Error::custom("duplicate"));
                    }
                    out.insert(k, map.next_value::<Strict>()?.0);
                }
                Ok(Strict(Value::Object(out)))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Strict, A::Error> {
                let mut out = Vec::new();
                while let Some(v) = seq.next_element::<Strict>()? {
                    out.push(v.0);
                }
                Ok(Strict(Value::Array(out)))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Strict, E> {
                Ok(Strict(json!(v)))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Strict, E> {
                Ok(Strict(json!(v)))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Strict, E> {
                Ok(Strict(json!(v)))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Strict, E> {
                Ok(Strict(json!(v)))
            }
        }
        d.deserialize_any(V)
    }
}

/// A successful, complete numeric `list table inet omavless_netguard` response
/// only. Failed commands, missing/truncated output and table absence must be
/// handled by the adapter; an empty JSON array is never proof of absence here.
/// `boot`/`netns_inode` come from the trusted observation, not peer input.
pub fn classify_readback(
    bytes: &[u8],
    boot: [u8; 16],
    netns_inode: u64,
    receipt: Option<TrustedTableIdentity>,
) -> Table {
    if bytes.len() > MAX_READBACK_BYTES {
        return Table::Unreadable;
    }
    let Ok(Strict(mut document)) = serde_json::from_slice(bytes) else {
        return Table::Unreadable;
    };
    let Some(root) = document.as_object_mut() else {
        return Table::Unreadable;
    };
    if root.len() != 1 {
        return Table::Unreadable;
    }
    let Some(entries) = root.get_mut("nftables").and_then(Value::as_array_mut) else {
        return Table::Unreadable;
    };
    if entries.first().and_then(|v| v.get("metainfo")).is_some() {
        let first = entries.remove(0);
        let Some(info) = first.get("metainfo").and_then(Value::as_object) else {
            return Table::Unreadable;
        };
        if first.as_object().map(|o| o.len()) != Some(1)
            || info.len() != 3
            || info.get("json_schema_version") != Some(&json!(1))
            || !info.get("version").is_some_and(Value::is_string)
            || !info.get("release_name").is_some_and(Value::is_string)
        {
            return Table::Unreadable;
        }
    }
    let Some(table) = entries.first().and_then(|v| v.get("table")) else {
        return Table::Unreadable;
    };
    if table.get("family") != Some(&json!(TABLE_FAMILY))
        || table.get("name") != Some(&json!(TABLE_NAME))
    {
        return Table::Foreign;
    }
    let Some(handle) = table
        .get("handle")
        .and_then(Value::as_u64)
        .filter(|v| *v > 0)
    else {
        return Table::Unreadable;
    };
    if netns_inode == 0
        || receipt
            != Some(TrustedTableIdentity {
                boot,
                netns_inode,
                table_handle: handle,
            })
    {
        return Table::Foreign;
    }
    let mut handles = std::collections::BTreeSet::new();
    for (index, entry) in entries.iter_mut().enumerate() {
        let Some(object) = entry.as_object_mut() else {
            return Table::Unreadable;
        };
        if object.len() != 1 {
            return Table::Unreadable;
        }
        let kind = if index == 0 {
            "table"
        } else if index == 1 {
            "chain"
        } else {
            "rule"
        };
        let Some(value) = object.get_mut(kind).and_then(Value::as_object_mut) else {
            return Table::OwnedUnrecognized;
        };
        let Some(h) = value
            .remove("handle")
            .and_then(|v| v.as_u64())
            .filter(|v| *v > 0)
        else {
            return Table::Unreadable;
        };
        // Table handles use a different namespace from chains and rules.
        if index > 0 && !handles.insert(h) {
            return Table::Unreadable;
        }
    }
    for policy in [Policy::FullVpn, Policy::Emergency] {
        if *entries == objects(policy) || *entries == kernel_objects(policy) {
            return Table::OwnedVerified(policy);
        }
    }
    Table::OwnedUnrecognized
}
