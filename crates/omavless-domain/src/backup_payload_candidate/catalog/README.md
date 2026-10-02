# Frozen backup template catalog

These are compatibility data, not the mutable runtime templates. V1 freezes
the exact accepted source bytes from commit
`6e103e6edbe237c7a3d81f5bb9f2fe9716e21ebe` (the proxy-selection fix):

| Preset | File | Original Git blob |
| --- | --- | --- |
| roscomvpn-default | v1/default.yaml | 58d45135aaa4b0710c578c3c4894abca4c6eee15 |
| china-cn-direct | v1/china.yaml | 26f5918967602d5a2b1851cf05f308d86dc73cfe |
| iran-ir-direct | v1/iran.yaml | 588676b125c24cae13d158f3585d93bf7541ce77 |

The only variants are replacement of the single literal LF-delimited
`mode: rule` line with `mode: rule`, `mode: global` or `mode: direct`.
Everything else must match byte for byte, including comments and line endings.
This closed nine-member set does not call the ordinary routing renderer.
Accepted input bytes are borrowed unchanged. No YAML parser, normalization,
automatic migration or network lookup is performed.

Never edit a V1 snapshot or its recognition grammar to follow a runtime template
update. Future supported snapshots require explicit review, a new catalog entry,
immutable provenance/digest tests and cross-version acceptance fixtures. Runtime
template edits alone must not enlarge or shrink the accepted backup set. Unknown
templates remain unreadable; being present in Git history is not approval.
Pre-fix historical proxy-selection variants are deliberately excluded.

The encrypted envelope already carries exact template bytes, so no template ID,
new envelope version or silent rewrite is needed for this catalog. A future
security revocation must be explicit and distinguish archival readability from
permission to restore/execute; this slice introduces no such product mechanism.
Store-schema evolution and core/template execution compatibility are separate
gates. Supporting these bytes does not activate restore or promise compatibility
with arbitrary future Mihomo versions.
