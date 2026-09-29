# Managed DNS RC triple: offline x86_64 assembly — 2026-09-28

Scope: agent-run, offline release-preparation diagnostic. This is not a
published GitHub release, marketplace installation, owner-attended acceptance
or proof that the final RC frontend can download immutable assets. No private
profiles, endpoints, logs, screenshots or credentials were used as inputs.
The physical PC and its VPN were untouched.

- Reviewed source of **both** CI packages:
  `6b672f9c6e6fd5ff53f4df37568932e8d151d697`, x86_64,
  `0.9.0rc1-1`. Application SHA-256:
  `477309dddf18b405258b664ff1a0998e8dbe934b73cc70dc9d5a93de58f12dca`.
  Production-name DNS companion SHA-256:
  `5d3d5b4d1204ab233c22ab3f5f9f1e02eaef8d63e2dc168faa9939917cdbb8e8`.
- Frontend commit:
  `779be43a9faeecfa2317c6a73fcb3990c4e14b7a`, descendant of the
  package source. The offline assembler verified that the runtime/build input
  tree was equivalent, while inspecting the exact application dependency and
  DNS package members, modes, metadata, receipt, ELF architecture, payload
  digests and committed privileged scripts/unit.
- The output contains the original two archives and the frontend archive.
  `sha256sum --check SHA256SUMS` passed for all four recorded files. The
  frontend archive SHA-256 is
  `cf450ec8d5cdb118758062e92d754015e2e7dfa56b890e29a655063f9403351b`.
  Its allowlisted contents include the first-use script and both release pin
  maps, not a private profile or developer Python tool.
- `managed-dns-pair.json` records `bootstrapPins: empty`,
  `publishedDownloadVerified: false` and
  `publication: unpublished-candidate`. Thus the output is **not** a usable
  public first-use download. The caller-supplied hashes are reviewed inputs,
  not authenticity signatures.
- The whole `./tests/run.sh` gate passed, including ten new offline triple
  tests for matching/partial pins, runtime drift, hash failure, extra DNS
  members, duplicate metadata and dirty/occupied source/output refusal.

Remaining: the final release source and both architecture artifacts must be
reconciled; immutable public asset identity and pins, real CDN transfer,
installed migration/upgrade/removal, default-deny firewall guidance,
end-to-end working-server DNS/TUN/HTTPS and owner-attended acceptance remain
separate gates. No release or marketplace action was performed.
