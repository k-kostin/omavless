use super::*;

const PORT: u32 = 123;
const SEQ: u32 = 7;
const GENERATION: u32 = 19;

// Independently spell the two Emergency expressions using the kernel's bare
// nesting convention. Do not build this positive oracle from the encoder.
fn loopback() -> Vec<u8> {
    let meta = [
        attribute(1, &1_u32.to_be_bytes()),
        attribute(2, &7_u32.to_be_bytes()),
    ]
    .concat();
    let cmp = [
        attribute(1, &1_u32.to_be_bytes()),
        attribute(2, &[0; 4]),
        attribute(3, &attribute(1, b"lo\0")),
    ]
    .concat();
    [
        attribute(1, &[attribute(1, b"meta\0"), attribute(2, &meta)].concat()),
        attribute(1, &[attribute(1, b"cmp\0"), attribute(2, &cmp)].concat()),
        drop_or_accept(1),
    ]
    .concat()
}
fn drop_or_accept(code: u32) -> Vec<u8> {
    attribute(
        1,
        &[
            attribute(1, b"immediate\0"),
            attribute(
                2,
                &[
                    attribute(1, &[0; 4]),
                    attribute(2, &attribute(2, &attribute(1, &code.to_be_bytes()))),
                ]
                .concat(),
            ),
        ]
        .concat(),
    )
}
fn rule(handle: u64, position: Option<u64>, expression: &[u8]) -> Vec<u8> {
    let mut body = vec![1, 0, 0, GENERATION as u8];
    body.extend(attribute(1, TABLE));
    body.extend(attribute(2, CHAIN));
    body.extend(attribute(8, &[]));
    body.extend(attribute(3, &handle.to_be_bytes()));
    if let Some(position) = position {
        body.extend(attribute(6, &position.to_be_bytes()));
    }
    body.extend(attribute(4, expression));
    message(NEW_RULE, 0x802, SEQ, PORT, &body)
}
fn receive(dump: &mut RuleDump, bytes: &[u8]) -> Result<()> {
    dump.receive(bytes, Some(NetlinkAddr::new(0, 0)), MsgFlags::empty())
}
fn finish(dump: &mut RuleDump) -> Result<LocalRuleInventory> {
    receive(dump, &message(3, 2, SEQ, PORT, &[0; 4]))?;
    dump.classify()
}
fn new() -> RuleDump {
    RuleDump::new(SEQ, PORT, GENERATION).unwrap()
}

#[test]
fn independent_emergency_oracle_requires_complete_ordered_dump() {
    let mut dump = new();
    receive(&mut dump, &rule(8, None, &loopback())).unwrap();
    assert!(dump.classify().is_err());
    receive(&mut dump, &rule(9, Some(8), &drop_or_accept(0))).unwrap();
    assert_eq!(
        finish(&mut dump),
        Ok(LocalRuleInventory::ExactRulesUntrusted(Policy::Emergency))
    );
    assert!(finish(&mut dump).is_err());
}

#[test]
fn empty_missing_reordered_extra_and_wrong_verdict_are_not_exact() {
    for expressions in [
        vec![],
        vec![loopback()],
        vec![drop_or_accept(0), loopback()],
        vec![loopback(), drop_or_accept(1)],
        vec![loopback(), drop_or_accept(0), drop_or_accept(1)],
    ] {
        let mut dump = new();
        for (index, expression) in expressions.iter().enumerate() {
            receive(
                &mut dump,
                &rule(
                    index as u64 + 1,
                    (index != 0).then_some(index as u64),
                    expression,
                ),
            )
            .unwrap();
        }
        assert_eq!(finish(&mut dump), Ok(LocalRuleInventory::OtherUntrusted));
    }
}

#[test]
fn dump_refuses_bad_identity_framing_generation_position_and_duplicates() {
    let original = rule(8, None, &loopback());
    for offset in [0, 4, 6, 8, 12, 16, 17, 19] {
        let mut bad = original.clone();
        bad[offset] ^= 1;
        assert!(receive(&mut new(), &bad).is_err());
    }
    for end in 0..original.len() {
        assert!(receive(&mut new(), &original[..end]).is_err());
    }
    for position in [None, Some(7), Some(9)] {
        let mut dump = new();
        receive(&mut dump, &original).unwrap();
        assert!(receive(&mut dump, &rule(9, position, &drop_or_accept(0))).is_err());
    }
    let mut dump = new();
    receive(&mut dump, &original).unwrap();
    assert!(receive(&mut dump, &rule(8, Some(8), &drop_or_accept(0))).is_err());
    for sender in [
        None,
        Some(NetlinkAddr::new(1, 0)),
        Some(NetlinkAddr::new(0, 1)),
    ] {
        assert!(new().receive(&original, sender, MsgFlags::empty()).is_err());
    }
    assert!(
        new()
            .receive(&original, Some(NetlinkAddr::new(0, 0)), MsgFlags::MSG_TRUNC)
            .is_err()
    );
    for flags in [0x12, 0x802, 4] {
        assert!(receive(&mut new(), &message(3, flags, SEQ, PORT, &[0; 4])).is_err());
    }
    assert!(receive(&mut new(), &message(3, 2, SEQ, PORT, &[1; 4])).is_err());
}

#[test]
fn coalesced_done_and_budgets_are_strict() {
    let mut bytes = rule(8, None, &loopback());
    bytes.extend(rule(9, Some(8), &drop_or_accept(0)));
    bytes.extend(message(3, 2, SEQ, PORT, &[0; 4]));
    let mut dump = new();
    receive(&mut dump, &bytes).unwrap();
    assert_eq!(
        dump.classify(),
        Ok(LocalRuleInventory::ExactRulesUntrusted(Policy::Emergency))
    );
    bytes.extend(message(3, 2, SEQ, PORT, &[0; 4]));
    assert!(receive(&mut new(), &bytes).is_err());
    for mut dump in [
        RuleDump {
            total: MAX_BYTES,
            ..new()
        },
        RuleDump {
            datagrams: 32,
            ..new()
        },
        RuleDump {
            messages: MAX_MESSAGES,
            ..new()
        },
    ] {
        assert!(receive(&mut dump, &message(3, 2, SEQ, PORT, &[0; 4])).is_err());
    }
}

#[test]
fn nested_data_is_typed_not_recursively_guessed_and_duplicates_refuse() {
    assert!(same_expressions(&loopback(), &full_vpn_wire::rules()[0]).unwrap());
    let mut extra = loopback();
    extra.extend(drop_or_accept(1));
    assert!(!same_expressions(&extra, &full_vpn_wire::rules()[0]).unwrap());
    let scalar = attribute(1, &[4, 0, 1, 0]);
    assert!(same_fields(&scalar, &scalar, 0).unwrap());
    assert!(same_fields(&[scalar.clone(), scalar.clone()].concat(), &scalar, 0).is_err());
    let flagged = emergency_wire::nested(1, &[4, 0, 1, 0]);
    assert!(!same_fields(&flagged, &scalar, 0).unwrap());
    assert!(same_fields(&attribute(0x4001, &[0; 4]), &scalar, 0).is_err());
}

#[test]
#[ignore = "VM-only retained creator and raw rule readback in fresh user/net namespace"]
fn raw_rules_in_disposable_vm() {
    use std::{
        os::fd::AsFd,
        process::{Command, Stdio},
    };
    assert_eq!(std::env::var("OMAVLESS_K1_RAW_RULE_VM").as_deref(), Ok("1"));
    let parent = File::open("/proc/thread-self/ns/net").unwrap();
    let identity = namespace_identity(&parent).unwrap();
    for policy in ["full", "emergency"] {
        let mut child = Command::new("/usr/bin/unshare")
            .env_clear()
            .env("OMAVLESS_K1_RAW_RULE_CHILD", policy)
            .args(["--user", "--map-root-user", "--net", "--"])
            .arg(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "kernel_observer::rule_wire::tests::raw_rules_child",
                "--nocapture",
            ])
            .stdin(Stdio::from(parent.as_fd().try_clone_to_owned().unwrap()))
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("isolated raw rule deadline");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(namespace_identity(&parent).unwrap(), identity);
        assert_eq!(
            namespace_identity(&namespace_file().unwrap()).unwrap(),
            identity
        );
    }
    println!("K1_RAW_RULE_VM_PASS");
}

#[test]
#[ignore = "internal isolated raw rule child; direct invocation refuses before socket"]
fn raw_rules_child() {
    use std::os::fd::AsFd;
    let selected = std::env::var("OMAVLESS_K1_RAW_RULE_CHILD").unwrap();
    assert!(matches!(selected.as_str(), "full" | "emergency"));
    let parent = File::from(std::io::stdin().as_fd().try_clone_to_owned().unwrap());
    let parent_id = namespace_identity(&parent).unwrap();
    let child_id = namespace_identity(&namespace_file().unwrap()).unwrap();
    assert_ne!(parent_id, child_id);
    let interfaces = std::fs::read_to_string("/proc/thread-self/net/dev").unwrap();
    let names: Vec<_> = interfaces
        .lines()
        .skip(2)
        .map(|v| v.split(':').next().unwrap().trim())
        .collect();
    assert_eq!(names, ["lo"]);
    let mut session = LocalReadSession::open().unwrap();
    assert_eq!(session.inspect_rules(), Ok(LocalRuleInventory::TableAbsent));
    let deadline = Instant::now() + Duration::from_secs(2);
    let generation = session
        .exchange(GET_GEN, 10, deadline)
        .unwrap()
        .generation()
        .unwrap();
    let first = 11;
    let full = full_vpn_wire::encode(generation, first).unwrap();
    let emergency = emergency_wire::encode(generation, first).unwrap();
    let is_full = selected == "full";
    let (batch, barrier) = if is_full {
        (full.batch(), full.barrier())
    } else {
        (emergency.batch(), emergency.barrier())
    };
    let mut full_reply =
        full_vpn_wire::reply::FullVpnTranscript::new(generation, first, session.local.pid())
            .unwrap();
    let mut emergency_reply =
        emergency_wire::reply::EmergencyTranscript::new(generation, first, session.local.pid())
            .unwrap();
    for bytes in [batch, barrier] {
        session.check(deadline).unwrap();
        assert_eq!(
            sendto(
                session.socket.as_raw_fd(),
                bytes,
                &NetlinkAddr::new(0, 0),
                MsgFlags::MSG_DONTWAIT
            )
            .unwrap(),
            bytes.len()
        );
    }
    let mut messages = 0;
    while messages < if is_full { 14 } else { 6 } {
        session.check(deadline).unwrap();
        let mut bytes = [0; LIMIT];
        let mut iov = [IoSliceMut::new(&mut bytes)];
        match recvmsg::<NetlinkAddr>(
            session.socket.as_raw_fd(),
            &mut iov,
            None,
            MsgFlags::MSG_DONTWAIT,
        ) {
            Ok(reply) => {
                let (length, sender, flags) = (reply.bytes, reply.address.unwrap(), reply.flags);
                assert!(length <= LIMIT);
                if is_full {
                    full_reply
                        .push_datagram(
                            &bytes[..length],
                            sender.pid(),
                            sender.groups(),
                            flags.bits() as u32,
                        )
                        .unwrap();
                } else {
                    emergency_reply
                        .push_datagram(
                            &bytes[..length],
                            sender.pid(),
                            sender.groups(),
                            flags.bits() as u32,
                        )
                        .unwrap();
                }
                let mut rest = &bytes[..length];
                while !rest.is_empty() {
                    let size = u32n(&rest[..4]).unwrap() as usize;
                    rest = &rest[aligned(size)..];
                    messages += 1;
                }
            }
            Err(nix::errno::Errno::EAGAIN) => std::thread::sleep(Duration::from_millis(1)),
            Err(_) => panic!("isolated raw create receive"),
        }
    }
    if is_full {
        full_reply.finish().unwrap();
    } else {
        emergency_reply.finish().unwrap();
    }
    session.next_sequence = 100;
    let policy = if is_full {
        Policy::FullVpn
    } else {
        Policy::Emergency
    };
    for _ in 0..2 {
        assert_eq!(
            session.inspect_rules(),
            Ok(LocalRuleInventory::ExactRulesUntrusted(policy))
        );
    }
    assert_eq!(
        session.inspect_policy_shape(),
        Ok(crate::nft::UntrustedPolicyShape::Exact(policy))
    );
    assert_eq!(
        namespace_identity(&namespace_file().unwrap()).unwrap(),
        child_id
    );
    // Destroy this isolated namespace at child exit; never delete a host table.
    println!("K1_RAW_RULE_CHILD_PASS");
}

#[test]
fn raw_rule_child_direct_invocation_refuses_before_effects() {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .env_clear()
        .args([
            "--ignored",
            "--exact",
            "kernel_observer::rule_wire::tests::raw_rules_child",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(!status.success());
}
