//! Inactive, read-only full-policy shape observation. The fixed `nft` child
//! reads only our scoped table; neither it nor the parser authenticates the
//! creator. No caller supplies a program, argument, namespace or policy.

use super::*;
use crate::nft::{self, UntrustedPolicyShape};
use std::{
    io::Read,
    process::{Command, Stdio},
};

const NFT_PATH: &str = "/usr/bin/nft";
const NFT_ARGS: &[&str] = &[
    "--json",
    "--handle",
    "--numeric",
    "--numeric-priority",
    "list",
    "table",
    "inet",
    "omavless_netguard",
];

fn fixed_scoped_readback(deadline: Instant) -> Result<Vec<u8>> {
    require(Instant::now() < deadline)?;
    let mut child = Command::new(NFT_PATH)
        .args(NFT_ARGS)
        .env_clear()
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| REFUSE)?;
    let stdout = child.stdout.take().ok_or(REFUSE)?;
    // Reading in parallel avoids pipe back-pressure without `Command::output`'s
    // unbounded allocation. An oversized or hung child is killed, never parsed.
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take((nft::MAX_READBACK_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let status = loop {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(REFUSE);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(REFUSE);
            }
        }
    };
    let bytes = reader.join().map_err(|_| REFUSE)?.map_err(|_| REFUSE)?;
    require(status.success() && !bytes.is_empty() && bytes.len() <= nft::MAX_READBACK_BYTES)?;
    Ok(bytes)
}

impl LocalReadSession {
    fn rule_sequences(&mut self) -> Result<[u32; 2]> {
        let first = self.next_sequence;
        require(first != 0)?;
        let second = first.checked_add(1).ok_or(REFUSE)?;
        self.next_sequence = second.checked_add(1).ok_or(REFUSE)?;
        Ok([first, second])
    }

    /// A complete fixed-table `nft` JSON read under a GETGEN sandwich and a
    /// retained, repeatedly checked local namespace/socket pair. A successful
    /// `Exact` result is still untrusted: this API never returns `Table` or an
    /// ownership token and is not wired to any installed service or EffectPort.
    pub fn inspect_policy_shape(&mut self) -> Result<UntrustedPolicyShape> {
        let result = self.inspect_policy_shape_once();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn inspect_policy_shape_once(&mut self) -> Result<UntrustedPolicyShape> {
        let deadline = Instant::now() + Duration::from_secs(1);
        self.check(deadline)?;
        let [before_seq, after_seq] = self.rule_sequences()?;
        let before = self.exchange(GET_GEN, before_seq, deadline)?.generation()?;
        self.check(deadline)?;
        let bytes = fixed_scoped_readback(deadline)?;
        self.check(deadline)?;
        let after = self.exchange(GET_GEN, after_seq, deadline)?.generation()?;
        require(before == after)?;
        self.check(deadline)?;
        let shape = nft::classify_untrusted_shape(&bytes);
        require(shape != UntrustedPolicyShape::Unreadable)?;
        Ok(shape)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_command_is_fixed_and_scoped() {
        assert_eq!(NFT_PATH, "/usr/bin/nft");
        assert_eq!(
            NFT_ARGS[4..],
            ["list", "table", "inet", "omavless_netguard"]
        );
        assert!(NFT_ARGS.contains(&"--numeric-priority"));
    }
}
