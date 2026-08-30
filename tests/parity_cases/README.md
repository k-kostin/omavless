# Migration parity result fixtures

Status: R0 language-neutral result contract.

Parity adapters write a sanitized JSON report for the existing Python/reference
implementation and the Rust candidate. The `omavless-parity` tool compares
those reports; it does not execute either implementation.

The v1 report is bounded to 1 MiB and 4096 cases. Each case contains only:

- a public stable case ID;
- a stable classification slug;
- an optional lowercase SHA-256 fingerprint of canonical private semantics;
- at most 64 public scalar facts. Text facts are restricted to identifier-like
  tokens; free-form, nested and credential-bearing values are rejected.

Adapters own canonicalization and redaction. A fingerprint can prove equality
without copying the canonical private value into the report. Checked-in reports
must use credential-free fixtures only.

Never include profile names, endpoints, URIs, UUIDs, passwords, keys,
subscription URLs, controller secrets or provider identities. Private local
inputs stay outside Git in regular user-owned mode-`0600` files and never travel
through process arguments. The comparator reports only bounded public case IDs;
it never prints classifications, facts, fingerprints, input paths or parser
fragments.

The R0 smoke pair proves the comparison boundary itself. R1 and later stages
will add implementation-specific adapters and real language-neutral parity
corpora without widening this public result envelope.
