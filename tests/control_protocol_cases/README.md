# Control protocol v1 corpus

`cases.json` is the credential-free, language-neutral semantic corpus shared by
the temporary Python reference and canonical future Rust implementation.

R1 additionally generates a deterministic 57-case boundary matrix in
`crates/omavless-control-protocol/tests/differential.rs`. It covers valid
maxima, malformed/duplicate JSON, UTF-8, frame/string/depth bounds, exact
envelopes, version/ID/revision/operation validation, stable errors and raw-input
non-disclosure. The test-only Python adapter publishes only an acceptance
boolean and stable classification; neither implementation returns raw frames.

Never add real profile/subscription content, endpoints, credentials, provider
names or controller secrets. New contract cases must use synthetic public data,
remain within the accepted v1 bounds and pass through both implementations.
