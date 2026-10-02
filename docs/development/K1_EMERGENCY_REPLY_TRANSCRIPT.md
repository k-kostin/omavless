# K1 fixed Emergency reply transcript candidate

Status: inactive pure Rust parsing candidate, 2026-10-02. It does not open a
socket, send a batch, authenticate a namespace, prove a kernel effect or own an
nftables table. It is not wired to the installed application or NetGuard.

The encoder in `omavless-netguard::emergency_wire` emits exactly one Emergency
batch with four acknowledged operations and one separate acknowledged GETGEN
barrier. `emergency_wire::reply::EmergencyTranscript` derives the five expected
request headers from that same fixed encoder. It consumes only caller-supplied
raw response datagrams and receive metadata, with a 32-KiB aggregate and
16-datagram bound. The future transport must supply authentic `recvmsg` sender
and flags; fabricating these values can fabricate a complete transcript.

The decoder accepts only one ACK for each of the four operation sequences,
one GETGEN ACK and one well-formed NEWGEN response. Success ACKs must contain
exactly the echoed 16-byte original request header. A nonzero error, duplicate,
unexpected sequence/type/flag/pid, malformed length/alignment/padding,
non-kernel sender metadata, truncated receive, invalid generation attribute or
missing reply permanently poisons the instance. `finish` closes it and returns
only `CompleteUntrustedTranscript { observed_generation }`, never a created or
owned-table token. It does not infer generation freshness by comparing the
request generation with the barrier response.

This closes the raw-byte parsing gap in the earlier Python fixture's
`checked_reply`, which saw already-decoded dictionaries. The kernel exchange,
deadline, ACK-loss/restart behavior, complete policy readback, retained creator,
canonical host namespace, reviewed safe syscall bindings and durable receipt
ordering remain separate gates. The complete K1 product matrix, including
installed boot and physical NIC/suspend cases, remains open.

Unit tests use only synthetic transcripts. An additional opt-in exact-head
developer gate feeds the raw successful ACK/GETGEN datagrams from the existing
disposable-network-namespace VM fixture into the Rust decoder; it refuses
kernel errors such as a stale-generation batch before that success-only check.
On 2026-10-02 the gate passed in Omarchy Dev VM using exact test binary SHA-256
`be96c754212422c084b2c7e39e3ff69a59228939a76065734aea69493f331a00`.
The temporary VM binary and its dedicated directory were removed afterward.
No real VPN profile or host firewall is used. This demonstrates compatibility
with that VM kernel's success replies, not complete transport, ownership,
physical-host or startup acceptance.
