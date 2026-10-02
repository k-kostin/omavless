# Upstream API compile-only consumer

These templates are not a workspace crate, adopted dependency or runtime
authenticator. These checks compile, never execute, the consumer. They verify borrowed-descriptor API types
and the x86_64/aarch64 Linux ioctl/socket constants under compilation with
unsafe code forbidden in the consumer. The proposed upstream library itself
still contains the separately reviewed unsafe boundary.

Supply a private exported `nix/` sibling directory from exact upstream
`e35c00891f52468979f92b795de2dc1f3dd58a87`, with exactly the owning patch
SHA-256 `520b6f8d4ef72f122c9970c5be436823a2fcf0414faf7f000467e7a63c326d62`.
Copy the three templates into private `consumer/Cargo.toml`,
`consumer/Cargo.lock` and `consumer/src/lib.rs`. Do not build them in the
application workspace or use an arbitrary source checkout as attestation.

Using checksum-verified official compiler components and already cached,
lock-pinned dependencies, the tested consumer compiles offline with
Rust/Cargo 1.69.0. Old Cargo's registry-cache format differs; an isolated
offline vendor directory avoids network access and global configuration edits.
It also cross-checks with official Rust 1.98.1 for
`aarch64-unknown-linux-gnu`, without linking or executing an ARM64 binary.
Keep Cargo targets and compiler components in private HOME scratch. Do not
change the user's installed compiler, shell profile or system package state.

This does not run all upstream features/tests, establish canonical namespace
authority, exercise unsupported kernels or fulfill maintainer/adoption review.
Exact compiler component/dependency hashes and limits are recorded in the
[owning review](../../docs/development/K1_NAMESPACE_PATCH_REVIEW.md).
