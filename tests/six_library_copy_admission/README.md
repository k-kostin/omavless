# Six measured library copy proposal

Developer-only successor to #644 tested code
`45feb7e13ea71485d032b8d6a1e2319edcedacc7`. ROOT's sole fresh static
wrapper completed known zero (`d56c91`), including strict canonical/private/
service/core/TUN/resolver/non-timer-network baseline checks. The separately
reviewed nine-file observer completed known zero (`fa6d82`); filtered report
SHA-256 is `a82939ae183af783a2c17eb0c4a9df24a33bddb1e6fd1e2ff17047f613d74a73`.
Its canonical typed static receipt is
`ff4df3e27690833b25386223a7b77cf1d0da4f45fc71cb41af3136aa89a60c38`.
The collector did not independently prove the wrapper's exit or preservation.

The explicit immutable source proposal adds only six canonical original ELFs:
libcrypto.so.3, libidn2.so.0.4.0, libssl.so.3, libunistring.so.5.2.1,
libz.so.1.3.2 and libzstd.so.1.5.7. The full measured 12-object static closure,
15 observed aliases and five selected package NAME/VERSION/desc/files pins
are retained as public evidence. Catalog evidence contains only count/hash/
metadata, not installed package inventory or directory names. Package records
do not prove signatures or global ownership uniqueness; static DT_NEEDED
does not establish arbitrary dlopen closure or loaded-object identity.

All predecessor 20 logical paths/19 original objects and decoder/encoder/libm
evidence remain exactly unchanged. The proposal has 26 logical paths/25
original objects. SONAME aliases are evidence only, never additional copy
targets or fallback search paths. The original admission implementation changes
only its immutable hash and exact counts; all retained FD/ancestor identity,
mode, ownership, link-count, bounds and permanent refusal rules remain intact.
The manifest stays below its unchanged 64-KiB cap, and a canonical-result hash
control preserves exact u64 timestamps rather than floating-point projections.

This is source proposal only, not automatic observation-driven adoption,
installed package approval, compatibility acceptance or a production change.
Next the separately reviewed fresh live graph must prove original-to-copy-to-
actual mapping identity twice, retain unreaped direct-child/proc/ns anchors,
complete positive zero-only daemon shutdown and strict baseline gates. Prior
failed stages remain stopped and unchanged; no retries, guest query or cleanup
are authorized here. Full source/Rust gates and ROOT plus independent review
are required before ROOT's exclusive fresh invocation.
