use super::*;
const PORT: u32 = 17;
const SEQ: u32 = 3;
const GEN: u32 = 9;
fn new(kind: Kind) -> ObjectDump {
    ObjectDump::new(kind, SEQ, PORT, GEN).unwrap()
}
fn done() -> Vec<u8> {
    message(3, 2, SEQ, PORT, &[0; 4])
}
fn object(kind: Kind) -> Vec<u8> {
    message(
        kind.operation() - 1,
        kind.flags(),
        SEQ,
        PORT,
        &[
            vec![1, 0, 0, GEN as u8],
            attribute(1, TABLE),
            attribute(2, b"extra\0"),
            attribute(kind.handle() as u16, &7_u64.to_be_bytes()),
        ]
        .concat(),
    )
}
fn receive(dump: &mut ObjectDump, bytes: &[u8]) -> Result<()> {
    dump.receive(bytes, Some(NetlinkAddr::new(0, 0)), MsgFlags::empty())
}

#[test]
fn all_three_kinds_require_complete_empty_dump_and_any_object_prevents_empty() {
    for kind in [Kind::Set, Kind::Object, Kind::Flowtable] {
        let mut dump = new(kind);
        assert!(dump.empty().is_err());
        receive(&mut dump, &done()).unwrap();
        assert_eq!(dump.empty(), Ok(true));
        assert!(receive(&mut dump, &done()).is_err());
        let mut dump = new(kind);
        receive(&mut dump, &object(kind)).unwrap();
        assert!(dump.empty().is_err());
        receive(&mut dump, &done()).unwrap();
        assert_eq!(dump.empty(), Ok(false));
        let mut bytes = object(kind);
        bytes.extend(done());
        let mut dump = new(kind);
        receive(&mut dump, &bytes).unwrap();
        assert_eq!(dump.empty(), Ok(false));
    }
}

#[test]
fn incomplete_interrupted_duplicate_foreign_and_malformed_frames_refuse() {
    for kind in [Kind::Set, Kind::Object, Kind::Flowtable] {
        let original = object(kind);
        for end in 0..original.len() {
            assert!(receive(&mut new(kind), &original[..end]).is_err());
        }
        for offset in [0, 4, 6, 8, 12, 16, 17, 19, 24] {
            let mut bad = original.clone();
            bad[offset] ^= 1;
            assert!(receive(&mut new(kind), &bad).is_err());
        }
        let mut dump = new(kind);
        receive(&mut dump, &original).unwrap();
        assert!(receive(&mut dump, &original).is_err());
        for bad in [
            message(3, 0x12, SEQ, PORT, &[0; 4]),
            message(3, 2, SEQ, PORT, &[1; 4]),
            message(2, 0, SEQ, PORT, &[0; 20]),
            [done(), done()].concat(),
        ] {
            assert!(receive(&mut new(kind), &bad).is_err());
        }
        assert!(
            new(kind)
                .receive(&done(), Some(NetlinkAddr::new(1, 0)), MsgFlags::empty())
                .is_err()
        );
        assert!(
            new(kind)
                .receive(&done(), Some(NetlinkAddr::new(0, 0)), MsgFlags::MSG_TRUNC)
                .is_err()
        );
        for mut bounded in [
            ObjectDump {
                bytes: 64 * 1024,
                ..new(kind)
            },
            ObjectDump {
                datagrams: 32,
                ..new(kind)
            },
            ObjectDump {
                messages: 64,
                ..new(kind)
            },
        ] {
            assert!(receive(&mut bounded, &done()).is_err());
        }
    }
}

#[test]
fn object_attributes_cannot_hide_presence_or_duplicate_identity() {
    for kind in [Kind::Set, Kind::Object, Kind::Flowtable] {
        for extra in [
            attribute(1, TABLE),
            attribute(0x4001, TABLE),
            attribute(0x8002, b"extra\0"),
            attribute(31, &[0; 4]),
            attribute(kind.padding() as u16, &[1]),
        ] {
            let mut body = object(kind)[16..].to_vec();
            body.extend(extra);
            assert!(
                receive(
                    &mut new(kind),
                    &message(kind.operation() - 1, kind.flags(), SEQ, PORT, &body)
                )
                .is_err()
            );
        }
    }
}

#[test]
fn exact_shape_requires_every_table_chain_rule_and_extra_fact() {
    let table = || TableMetadata {
        flags: 6,
        uses: 1,
        handle: 7,
        owner: Some(PORT),
        userdata: None,
    };
    let chain = LocalChainInventory::ExpectedOutputChainUntrusted;
    let rules = LocalRuleInventory::ExactRulesUntrusted(Policy::FullVpn);
    assert_eq!(
        classify(&table(), PORT, chain, rules, true),
        LocalPolicyInventory::ExactUntrusted(Policy::FullVpn)
    );
    for changed in [
        TableMetadata {
            flags: 4,
            ..table()
        },
        TableMetadata {
            owner: Some(PORT + 1),
            ..table()
        },
        TableMetadata { uses: 2, ..table() },
        TableMetadata {
            userdata: Some(vec![]),
            ..table()
        },
    ] {
        assert_eq!(
            classify(&changed, PORT, chain, rules, true),
            LocalPolicyInventory::OtherUntrusted
        );
    }
    assert_eq!(
        classify(&table(), PORT, chain, rules, false),
        LocalPolicyInventory::OtherUntrusted
    );
    assert_eq!(
        classify(
            &table(),
            PORT,
            LocalChainInventory::OtherUntrusted,
            rules,
            true
        ),
        LocalPolicyInventory::OtherUntrusted
    );
    assert_eq!(
        classify(
            &table(),
            PORT,
            chain,
            LocalRuleInventory::OtherUntrusted,
            true
        ),
        LocalPolicyInventory::OtherUntrusted
    );
}
