//! VM-only synthetic packet gate; never a runtime service or host firewall tool.
use super::*;
use serde_json::json;

#[path = "packet_raw.rs"]
mod raw;

const CHILD: &str = "packet::nft_packet_child";
const PASS: &str = "K1_PACKET_CHILD_PASS";
const OUT: &str = "k1out0";
const PEER: &str = "k1peer0";
const FOREIGN: &str = "omavless_k1_foreign_fixture";
const FOREIGN_DROP: &str = "omavless_k1_foreign_drop_fixture";
const FIXTURE: &str = include_str!("packet_fixture.py");

// Two other base chains accept before and after the candidate's output hook.
// An earlier accept must not bypass its later drop; a later accept must not
// revive an already dropped packet. This is only a child-netns test fixture,
// not an approximation of any particular installed host firewall.
fn foreign_accept_commands() -> Vec<u8> {
    let commands = vec![
        json!({"create":{"table":{"family":"inet","name":FOREIGN}}}),
        json!({"add":{"chain":{"family":"inet","table":FOREIGN,"name":"early",
            "type":"filter","hook":"output","prio":0,"policy":"accept"}}}),
        json!({"add":{"rule":{"family":"inet","table":FOREIGN,"chain":"early",
            "expr":[{"accept":null}]}}}),
        json!({"add":{"chain":{"family":"inet","table":FOREIGN,"name":"late",
            "type":"filter","hook":"output","prio":400,"policy":"accept"}}}),
        json!({"add":{"rule":{"family":"inet","table":FOREIGN,"chain":"late",
            "expr":[{"accept":null}]}}}),
    ];
    serde_json::to_vec(&json!({"nftables": commands})).unwrap()
}

// A different firewall may block egress before NetGuard. This fixed child-only
// table deliberately drops non-loopback output, including K1-marked traffic.
// It tests availability interference, not a leak or compatibility promise.
fn foreign_drop_commands() -> Vec<u8> {
    let commands = vec![
        json!({"create":{"table":{"family":"inet","name":FOREIGN_DROP}}}),
        json!({"add":{"chain":{"family":"inet","table":FOREIGN_DROP,"name":"early_drop",
            "type":"filter","hook":"output","prio":100,"policy":"accept"}}}),
        json!({"add":{"rule":{"family":"inet","table":FOREIGN_DROP,"chain":"early_drop",
        "expr":[
            {"match":{"op":"!=","left":{"meta":{"key":"oifname"}},"right":"lo"}},
            {"drop":null}
        ]}}}),
    ];
    serde_json::to_vec(&json!({"nftables": commands})).unwrap()
}

fn ip(base: &NamespaceGuard, args: &[&str]) -> Output {
    base.check_identity().expect("isolated namespace required");
    let mut command = Command::new("/usr/bin/ip");
    command.env_clear().env("LC_ALL", "C").args(args);
    run(command, Stdio::null()).expect("ip unavailable")
}

fn links_valid(value: &Value, output: &str, pinned: Option<(u64, u64)>) -> Option<(u64, u64)> {
    let links = value.as_array()?;
    if links.len() != 3 {
        return None;
    }
    let find = |name| links.iter().find(|v| v["ifname"] == name);
    let lo = find("lo")?;
    let out = find(output)?;
    let peer = find(PEER)?;
    if lo["link_type"] != "loopback" || lo["ifindex"] != 1 {
        return None;
    }
    for v in [lo, out, peer] {
        if v.get("master").is_some() || v.get("link_netnsid").is_some() {
            return None;
        }
    }
    let indexes = (out["ifindex"].as_u64()?, peer["ifindex"].as_u64()?);
    // Actual same-netns iproute2 readback uses reciprocal link names. Numeric
    // peer indexes are accepted only as a complete separate representation.
    let named = out.get("link_index").is_none()
        && peer.get("link_index").is_none()
        && out["link"] == PEER
        && peer["link"] == output;
    let indexed = out.get("link").is_none()
        && peer.get("link").is_none()
        && out["link_index"].as_u64() == Some(indexes.1)
        && peer["link_index"].as_u64() == Some(indexes.0);
    if indexes.0 <= 1
        || indexes.1 <= 1
        || indexes.0 == indexes.1
        || pinned.is_some_and(|p| p != indexes)
        || out["linkinfo"]["info_kind"] != "veth"
        || peer["linkinfo"]["info_kind"] != "veth"
        || !(named || indexed)
    {
        return None;
    }
    Some(indexes)
}

fn routes_valid(value: &Value, output: &str) -> bool {
    value.as_array().is_some_and(|routes| {
        routes.len() <= 40
            && routes.iter().all(|v| {
                let dst = v.get("dst").and_then(Value::as_str).unwrap_or("");
                let dev = v.get("dev").and_then(Value::as_str).unwrap_or("");
                // No default, gateway, foreign device or arbitrary destination route.
                (matches!(dev, "lo") || dev == output || (dev == PEER && dst == "ff00::/8"))
                    && match dst {
                        "127.0.0.0/8" | "127.0.0.1" | "127.255.255.255" | "::1"
                        | "192.0.2.0/24" | "192.0.2.0" | "192.0.2.1" | "192.0.2.255"
                        | "10.0.0.2" | "255.255.255.255" | "2001:db8::/64" | "2001:db8::1"
                        | "fd00::2" | "fe80::/64" | "fe80::1" | "ff00::/8" => {
                            v.get("gateway").is_none() && v.get("multipath").is_none()
                        }
                        _ => false,
                    }
            })
    })
}

struct PacketGuard {
    base: NamespaceGuard,
    child: NamespaceIdentity,
    indexes: (u64, u64),
    output: &'static str,
}
impl PacketGuard {
    fn check(&self) {
        assert_eq!(self.base.check_identity().unwrap(), self.child);
        let links = ip(&self.base, &["-j", "-d", "link", "show"]);
        assert!(links.success);
        assert_eq!(
            links_valid(
                &serde_json::from_slice(&links.bytes).unwrap(),
                self.output,
                Some(self.indexes)
            ),
            Some(self.indexes),
            "unexpected link topology"
        );
        for family in ["-4", "-6"] {
            let routes = ip(&self.base, &["-j", family, "route", "show", "table", "all"]);
            assert!(routes.success);
            assert!(
                routes_valid(&serde_json::from_slice(&routes.bytes).unwrap(), self.output),
                "unexpected route inventory"
            );
        }
    }
    fn command(&self, executable: &str, args: &[&str]) -> Output {
        self.check();
        let mut command = Command::new(executable);
        command.env_clear().env("LC_ALL", "C").args(args);
        run(command, Stdio::null()).expect("fixture command unavailable")
    }
    fn policy(&self, policy: Policy) {
        let scratch = Scratch::new().unwrap();
        scratch
            .create("input.json")
            .unwrap()
            .write_all(&nft::render_create(policy))
            .unwrap();
        assert!(
            self.command(
                "/usr/bin/nft",
                &[
                    "--json",
                    "--file",
                    scratch.0.join("input.json").to_str().unwrap()
                ]
            )
            .success,
            "fixed policy creation refused"
        );
        let readback = self.command(
            "/usr/bin/nft",
            &[
                "--json",
                "--handle",
                "--numeric",
                "--numeric-priority",
                "list",
                "table",
                "inet",
                "omavless_netguard",
            ],
        );
        assert!(readback.success);
        let parsed: Value = serde_json::from_slice(&readback.bytes).unwrap();
        let handle = parsed["nftables"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|v| v.get("table")?.get("handle")?.as_u64())
            .unwrap();
        assert_eq!(
            nft::classify_readback(
                &readback.bytes,
                [1; 16],
                self.child.ino,
                Some(nft::TrustedTableIdentity {
                    boot: [1; 16],
                    netns_inode: self.child.ino,
                    table_handle: handle
                })
            ),
            Table::OwnedVerified(policy)
        );
    }
    fn install_foreign_accepts(&self) {
        let scratch = Scratch::new().unwrap();
        scratch
            .create("input.json")
            .unwrap()
            .write_all(&foreign_accept_commands())
            .unwrap();
        assert!(
            self.command(
                "/usr/bin/nft",
                &[
                    "--json",
                    "--file",
                    scratch.0.join("input.json").to_str().unwrap()
                ]
            )
            .success,
            "foreign fixture creation refused"
        );
    }
    fn foreign_snapshot(&self) -> Value {
        let readback = self.command(
            "/usr/bin/nft",
            &[
                "--json",
                "--handle",
                "--numeric",
                "--numeric-priority",
                "list",
                "table",
                "inet",
                FOREIGN,
            ],
        );
        assert!(readback.success, "foreign fixture disappeared");
        let value: Value = serde_json::from_slice(&readback.bytes).unwrap();
        let objects = value["nftables"].as_array().unwrap();
        assert!(
            objects
                .iter()
                .any(|entry| entry["table"]["name"] == FOREIGN)
        );
        let chains: Vec<_> = objects
            .iter()
            .filter_map(|entry| entry.get("chain"))
            .collect();
        let rules: Vec<_> = objects
            .iter()
            .filter_map(|entry| entry.get("rule"))
            .collect();
        assert_eq!(chains.len(), 2);
        assert_eq!(rules.len(), 2);
        for (name, priority) in [("early", 0), ("late", 400)] {
            assert!(chains.iter().any(|chain| {
                chain["family"] == "inet"
                    && chain["table"] == FOREIGN
                    && chain["name"] == name
                    && chain["type"] == "filter"
                    && chain["hook"] == "output"
                    && chain["prio"] == priority
                    && chain["policy"] == "accept"
            }));
            assert!(rules.iter().any(|rule| {
                rule["family"] == "inet"
                    && rule["table"] == FOREIGN
                    && rule["chain"] == name
                    && rule["expr"] == json!([{"accept":null}])
            }));
        }
        value
    }
    fn foreign_snapshot_exists(&self) -> bool {
        self.command(
            "/usr/bin/nft",
            &["--json", "list", "table", "inet", FOREIGN],
        )
        .success
    }
    fn install_foreign_drop(&self) {
        let scratch = Scratch::new().unwrap();
        scratch
            .create("input.json")
            .unwrap()
            .write_all(&foreign_drop_commands())
            .unwrap();
        assert!(
            self.command(
                "/usr/bin/nft",
                &[
                    "--json",
                    "--file",
                    scratch.0.join("input.json").to_str().unwrap()
                ]
            )
            .success,
            "foreign drop fixture creation refused"
        );
    }
    fn foreign_drop_snapshot(&self) -> Value {
        let readback = self.command(
            "/usr/bin/nft",
            &[
                "--json",
                "--handle",
                "--numeric",
                "--numeric-priority",
                "list",
                "table",
                "inet",
                FOREIGN_DROP,
            ],
        );
        assert!(readback.success, "foreign drop fixture disappeared");
        let value: Value = serde_json::from_slice(&readback.bytes).unwrap();
        let objects = value["nftables"].as_array().unwrap();
        assert!(
            objects
                .iter()
                .any(|entry| entry["table"]["name"] == FOREIGN_DROP)
        );
        assert!(objects.iter().any(|entry| {
            let chain = &entry["chain"];
            chain["family"] == "inet"
                && chain["table"] == FOREIGN_DROP
                && chain["name"] == "early_drop"
                && chain["type"] == "filter"
                && chain["hook"] == "output"
                && chain["prio"] == 100
                && chain["policy"] == "accept"
        }));
        assert_eq!(
            objects
                .iter()
                .filter(|entry| entry.get("rule").is_some())
                .count(),
            1
        );
        value
    }
    fn packets(&self, phase: &str) {
        self.check();
        let scratch = Scratch::new().unwrap();
        scratch
            .create("packet.py")
            .unwrap()
            .write_all(FIXTURE.as_bytes())
            .unwrap();
        let mut command = Command::new("/usr/bin/python3");
        command
            .env_clear()
            .env("LC_ALL", "C")
            .env("K1_PARENT_DEV", self.base.claimed_parent.dev.to_string())
            .env("K1_PARENT_INO", self.base.claimed_parent.ino.to_string())
            .env("K1_CHILD_INO", self.child.ino.to_string())
            .env("K1_OUT_INDEX", self.indexes.0.to_string())
            .env("K1_PEER_INDEX", self.indexes.1.to_string())
            .arg("-I")
            .arg(scratch.0.join("packet.py"))
            .arg(phase);
        let result = run(
            command,
            Stdio::from(self.base.parent_fd.try_clone().unwrap()),
        )
        .unwrap();
        if !result.success
            && let Some(index) = last_case(&result.bytes)
        {
            println!("K1_PACKET_CASE={index}");
        }
        assert!(
            result.success
                && std::str::from_utf8(&result.bytes).is_ok_and(|s| s
                    .lines()
                    .filter(|line| *line == format!("K1_PACKET_PASS={phase}"))
                    .count()
                    == 1),
            "packet vector gate failed: {phase}"
        );
        self.check();
    }
}

#[test]
#[ignore = "explicit VM-only child-netns packet gate"]
fn nft_packet_enforcement_in_disposable_vm() {
    assert!(
        std::env::var("OMAVLESS_K1_PACKET_VM").is_ok_and(|v| v == "1"),
        "VM opt-in required"
    );
    let parent_fd = File::open("/proc/self/ns/net").unwrap();
    let parent = fd_identity(&parent_fd).unwrap();
    let mut command = Command::new("/usr/bin/unshare");
    command
        .env_clear()
        .env("LC_ALL", "C")
        .env("OMAVLESS_K1_NFT_PARENT_DEV", parent.dev.to_string())
        .env("OMAVLESS_K1_NFT_PARENT_INO", parent.ino.to_string())
        .env("OMAVLESS_K1_PACKET_CHILD", "1")
        .args(["--user", "--map-root-user", "--net", "--"])
        .arg(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", CHILD, "--nocapture"]);
    // Six bounded 53-vector phases include negative capture windows; keep
    // each tool call at 15 seconds but allow their isolated outer process to
    // finish the whole fixed matrix.
    let result = run_bounded(
        command,
        Stdio::from(parent_fd.try_clone().unwrap()),
        Duration::from_secs(45),
    );
    assert_eq!(fd_identity(&parent_fd).unwrap(), parent);
    assert_eq!(
        fd_identity(&File::open("/proc/self/ns/net").unwrap()).unwrap(),
        parent
    );
    let result = result.expect("packet child unavailable");
    // Allowlisted stage only, never raw nft/ip stderr or observed packets.
    let output = std::str::from_utf8(&result.bytes).unwrap_or("");
    let stage = [
        "isolation",
        "links",
        "addresses",
        "baseline",
        "full",
        "foreign-drop",
        "interface",
        "emergency",
        "cleanup",
    ]
    .into_iter()
    .rev()
    .find(|s| output.lines().any(|l| l == format!("K1_PACKET_STAGE={s}")))
    .unwrap_or("launch");
    assert!(
        result.success && output.lines().filter(|l| *l == PASS).count() == 1,
        "isolated packet gate failed at {stage}, vector {:?}",
        last_case(&result.bytes)
    );
}

fn last_case(bytes: &[u8]) -> Option<u8> {
    std::str::from_utf8(bytes)
        .ok()?
        .lines()
        .filter_map(|line| {
            line.strip_prefix("K1_PACKET_CASE=")?
                .parse::<u8>()
                .ok()
                .filter(|v| (1..=53).contains(v))
        })
        .next_back()
}

#[test]
#[ignore = "internal packet namespace child; direct invocation refuses"]
fn nft_packet_child() {
    assert!(std::env::var("OMAVLESS_K1_PACKET_CHILD").is_ok_and(|v| v == "1"));
    let base = NamespaceGuard {
        parent_fd: File::from(std::io::stdin().as_fd().try_clone_to_owned().unwrap()),
        claimed_parent: NamespaceIdentity {
            dev: std::env::var("OMAVLESS_K1_NFT_PARENT_DEV")
                .unwrap()
                .parse()
                .unwrap(),
            ino: std::env::var("OMAVLESS_K1_NFT_PARENT_INO")
                .unwrap()
                .parse()
                .unwrap(),
        },
    };
    println!("K1_PACKET_STAGE=isolation");
    base.check().expect("fresh child must have only loopback");
    let child = base.check_identity().unwrap();
    println!("K1_PACKET_STAGE=links");
    assert!(
        ip(
            &base,
            &["link", "add", OUT, "type", "veth", "peer", "name", PEER]
        )
        .success
    );
    let links = ip(&base, &["-j", "-d", "link", "show"]);
    assert!(links.success);
    let links_value: Value = serde_json::from_slice(&links.bytes).unwrap();
    let indexes = links_valid(&links_value, OUT, None).expect("local veth pair required");
    let mut guard = PacketGuard {
        base,
        child,
        indexes,
        output: OUT,
    };
    println!("K1_PACKET_STAGE=addresses");
    for args in [
        vec!["link", "set", "lo", "up"],
        vec!["link", "set", OUT, "address", "02:00:00:00:00:01"],
        vec!["link", "set", PEER, "address", "02:00:00:00:00:02"],
        vec!["addr", "add", "192.0.2.1/24", "dev", OUT],
        vec!["-6", "addr", "add", "2001:db8::1/64", "dev", OUT, "nodad"],
        vec!["-6", "addr", "add", "fe80::1/64", "dev", OUT, "nodad"],
        vec!["link", "set", OUT, "addrgenmode", "none"],
        vec!["link", "set", PEER, "addrgenmode", "none"],
        vec!["link", "set", OUT, "up"],
        vec!["link", "set", PEER, "up"],
        vec!["route", "add", "10.0.0.2/32", "dev", OUT],
        vec!["-6", "route", "add", "fd00::2/128", "dev", OUT],
    ] {
        assert!(
            guard.command("/usr/bin/ip", &args).success,
            "fixed link setup unavailable"
        );
    }
    for address in ["192.0.2.2", "10.0.0.2", "2001:db8::2", "fd00::2", "fe80::2"] {
        assert!(
            guard
                .command(
                    "/usr/bin/ip",
                    &[
                        "neigh",
                        "replace",
                        address,
                        "lladdr",
                        "02:00:00:00:00:02",
                        "nud",
                        "permanent",
                        "dev",
                        OUT
                    ]
                )
                .success
        );
    }
    println!("K1_PACKET_STAGE=baseline");
    guard.install_foreign_accepts();
    let foreign_before = guard.foreign_snapshot();
    guard.packets("baseline");
    println!("K1_PACKET_STAGE=full");
    guard.policy(Policy::FullVpn);
    guard.packets("full");
    assert!(
        guard.foreign_snapshot() == foreign_before,
        "foreign fixture changed"
    );
    println!("K1_PACKET_STAGE=foreign-drop");
    guard.install_foreign_drop();
    let foreign_drop_before = guard.foreign_drop_snapshot();
    guard.packets("foreign-drop");
    assert!(
        guard.foreign_drop_snapshot() == foreign_drop_before,
        "foreign drop fixture changed"
    );
    assert!(
        guard
            .command("/usr/bin/nft", &["delete", "table", "inet", FOREIGN_DROP])
            .success
    );
    assert!(
        !guard
            .command(
                "/usr/bin/nft",
                &["--json", "list", "table", "inet", FOREIGN_DROP]
            )
            .success
    );
    // Removal of only that foreign blocker restores K1's marked egress rule.
    guard.packets("full");
    assert!(guard.foreign_snapshot() == foreign_before);
    println!("K1_PACKET_STAGE=interface");
    assert!(
        guard
            .command("/usr/bin/ip", &["link", "set", OUT, "name", nft::TUN])
            .success
    );
    guard.output = nft::TUN;
    guard.packets("interface");
    assert!(
        guard.foreign_snapshot() == foreign_before,
        "foreign fixture changed"
    );
    println!("K1_PACKET_STAGE=emergency");
    assert!(
        guard
            .command(
                "/usr/bin/nft",
                &["delete", "table", "inet", "omavless_netguard"]
            )
            .success
    );
    guard.policy(Policy::Emergency);
    guard.packets("emergency");
    assert!(
        guard.foreign_snapshot() == foreign_before,
        "foreign fixture changed"
    );
    println!("K1_PACKET_STAGE=cleanup");
    assert!(
        guard
            .command(
                "/usr/bin/nft",
                &["delete", "table", "inet", "omavless_netguard"]
            )
            .success
    );
    assert!(
        guard.foreign_snapshot() == foreign_before,
        "foreign fixture changed"
    );
    assert!(
        guard
            .command("/usr/bin/nft", &["delete", "table", "inet", FOREIGN])
            .success
    );
    assert!(!guard.foreign_snapshot_exists());
    // Child exit reclaims both veth endpoints; no interface is ever moved out.
    println!("{PASS}");
}

#[test]
fn topology_and_route_guards_refuse_uplinks_defaults_and_identity_changes() {
    assert_eq!(
        last_case(b"K1_PACKET_CASE=3\nK1_PACKET_CASE=private\n"),
        Some(3)
    );
    assert_eq!(last_case(b"K1_PACKET_CASE=0\nK1_PACKET_CASE=99\n"), None);
    let fixture = json!([
        {"ifname":"lo","ifindex":1,"link_type":"loopback"},
        {"ifname":OUT,"ifindex":2,"link_index":3,"linkinfo":{"info_kind":"veth"}},
        {"ifname":PEER,"ifindex":3,"link_index":2,"linkinfo":{"info_kind":"veth"}}
    ]);
    assert_eq!(links_valid(&fixture, OUT, Some((2, 3))), Some((2, 3)));
    let mut named = fixture.clone();
    named[1].as_object_mut().unwrap().remove("link_index");
    named[2].as_object_mut().unwrap().remove("link_index");
    named[1]["link"] = json!(PEER);
    named[2]["link"] = json!(OUT);
    assert_eq!(links_valid(&named, OUT, Some((2, 3))), Some((2, 3)));
    for (index, field, value) in [
        (1, "link", json!("foreign")),
        (2, "link", json!(PEER)),
        (1, "link_index", json!(3)),
        (1, "link_netnsid", json!(0)),
        (2, "master", json!(1)),
    ] {
        let mut changed = named.clone();
        changed[index][field] = value;
        assert_eq!(links_valid(&changed, OUT, Some((2, 3))), None);
    }
    for (pointer, bad) in [
        ("/1/link_index", json!(55)),
        ("/1/linkinfo/info_kind", json!("tun")),
        ("/1/ifindex", json!(4)),
    ] {
        let mut changed = fixture.clone();
        *changed.pointer_mut(pointer).unwrap() = bad;
        assert_eq!(links_valid(&changed, OUT, Some((2, 3))), None);
    }
    for field in ["master", "link_netnsid"] {
        let mut changed = fixture.clone();
        changed[1][field] = json!(0);
        assert_eq!(links_valid(&changed, OUT, None), None);
    }
    assert!(routes_valid(
        &json!([{"dst":"192.0.2.0/24","dev":OUT}]),
        OUT
    ));
    for bad in [
        json!([{"dst":"default","dev":OUT}]),
        json!([{"dst":"0.0.0.0/0","dev":OUT}]),
        json!([{"dst":"::/0","dev":OUT}]),
        json!([{"dst":"192.0.2.0/24","dev":"eth0"}]),
        json!([{"dst":"192.0.2.0/24","dev":OUT,"gateway":"192.0.2.2"}]),
        json!({}),
    ] {
        assert!(!routes_valid(&bad, OUT));
    }
}

#[test]
fn socket_fixture_self_checks_are_pure() {
    let scratch = Scratch::new().unwrap();
    scratch
        .create("packet.py")
        .unwrap()
        .write_all(FIXTURE.as_bytes())
        .unwrap();
    let mut command = Command::new("/usr/bin/python3");
    command
        .env_clear()
        .arg("-I")
        .arg(scratch.0.join("packet.py"))
        .arg("self-test");
    let result = run(command, Stdio::null()).unwrap();
    assert!(result.success && result.bytes == b"K1_PACKET_SELF_TEST_PASS\n");
}

#[test]
fn coexistence_fixture_has_only_fixed_accept_chains() {
    let value: Value = serde_json::from_slice(&foreign_accept_commands()).unwrap();
    let entries = value["nftables"].as_array().unwrap();
    assert_eq!(entries.len(), 5);
    assert_eq!(entries[0]["create"]["table"]["name"], FOREIGN);
    for (chain, priority, chain_index, rule_index) in [("early", 0, 1, 2), ("late", 400, 3, 4)] {
        let header = &entries[chain_index]["add"]["chain"];
        assert_eq!(header["family"], "inet");
        assert_eq!(header["table"], FOREIGN);
        assert_eq!(header["name"], chain);
        assert_eq!(header["hook"], "output");
        assert_eq!(header["prio"], priority);
        assert_eq!(header["policy"], "accept");
        let rule = &entries[rule_index]["add"]["rule"];
        assert_eq!(rule["table"], FOREIGN);
        assert_eq!(rule["chain"], chain);
        assert_eq!(rule["expr"], json!([{"accept":null}]));
    }
}

#[test]
fn foreign_drop_fixture_is_fixed_non_loopback_only() {
    let value: Value = serde_json::from_slice(&foreign_drop_commands()).unwrap();
    let entries = value["nftables"].as_array().unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0]["create"]["table"]["name"], FOREIGN_DROP);
    let chain = &entries[1]["add"]["chain"];
    assert_eq!(chain["family"], "inet");
    assert_eq!(chain["table"], FOREIGN_DROP);
    assert_eq!(chain["hook"], "output");
    assert_eq!(chain["prio"], 100);
    assert_eq!(chain["policy"], "accept");
    assert_eq!(
        entries[2]["add"]["rule"]["expr"],
        json!([
            {"match":{"op":"!=","left":{"meta":{"key":"oifname"}},"right":"lo"}},
            {"drop":null}
        ])
    );
}
