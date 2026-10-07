# Review-only prepared LOCAL reply composition

These are inert textual research fixtures, not product source, dependencies,
an authenticated manager transport or an instruction to run an ignored test.
No build/test script applies this patch or instantiates the template. Ordinary
runtime source, Cargo manifests/lock, backend contract and CLI refusal stay
unchanged. The [owning contract](../../../docs/development/T4_RETAINED_MANAGER_PARENT_PROTOTYPE.md)
and [external shim export/licenses](../rustix-owned-ancillary/README.md) remain
separate prerequisites, not evidence of normal adoption.

## Exact exported bytes and dependency boundary

`source-only-68f618c.patch` exports only two Rust files from public base
`9ac3b6758d304c3983a50518439144b1a092b348` to private experiment
`68f618c6c601d3fb9145cb90e585af1779f64eb9`. It adds the review-only module and
four-line private-feature seam inside the existing cfg(test)-only prototype;
it does not modify these ordinary source files in this checkout. Result hashes:

| Resulting file | SHA256 |
| --- | --- |
| restore_abort_prepared_local_reply.rs | 8599de705f67f7f071546e095e9d2981dd6cf69eb0b042b7806067803db96dcf |
| restore_abort_retained_parent_prototype.rs | 76229092efa2e60171a2cad47aa803667af2a08181aded91be3310374e4040a8 |

The original private driver used external prepared-rustix shim
`0a0c958cd823b5be70abd38cf8def36b1554a089`, not the ordinary registry API.
Its eleven-file source export and licenses are retained in the sibling fixture.
`private-driver.toml.in` replaces only that original absolute private path with
`@PREPARED_RUSTIX_0A0C958_HARNESS@` and adds a review warning. It is deliberately
not named Cargo.toml. This relocated template is UNCOMPILED and cannot inherit
the private driver's build/test evidence; no shim is silently downloaded,
applied or substituted. The template's other declarations preserve the private
driver's unchanged ordinary runtime dependency declarations.

`resolver-Cargo.lock.txt` is the exact immutable private resolver output,
not the ordinary application lock. The private graph has 212 package identities;
196 are shared with the ordinary lock. It selects 14 changed registry versions
and two additional local packages; shared identities keep their checksums.
Removed ordinary-only dependencies and altered shared dependency edges mean
this is NOT ordinary-lock equivalence or production resolver acceptance.
The text does not encode the replaced private path and is not auto-consumed.

| Fixture | Bytes | SHA256 |
| --- | ---: | --- |
| source-only-68f618c.patch | 14036 | 348e6c3ef5da4a4b7e0c272a0054cd2c2d81b0287f5603a01e06789e78642e13 |
| private-driver.toml.in | 1616 | cb3b94c96ba5687cbd1f53e0c5ba21e3c462e37c477f7013ad660cb54f1e4b46 |
| resolver-Cargo.lock.txt | 49718 | 1b132245a15e024b3f23b5a018ec01e9891058f05eea81ccb09cefe4d6d7b71e |

The literal patch preserves one context-marker-only space at line 262. Its
exact bytes pass `git apply --check`; source and documentation whitespace checks
exclude this intentional patch-format context line.

## Source regression and exact private HOST evidence

The original pure negative test constructed an unanchored heterogeneous array
from five- and six-byte arrays. Explicit `.as_slice()` operands fix this
intrinsic source type mismatch without changing its wrong-prefix rejection;
`heterogeneous_prefix_operands_are_byte_slices` adds an annotated slice-array
regression. A prior stopped compile's separate bounded diagnostic observer
reported E0308 and Cargo-could-not-compile categories only. That projection does
NOT demonstrate this mismatch was that invocation's cause or locate its error.
All stopped compile/mock/child scopes remain NONPASS and untouched.

After separate FULL ROOT and independent source/closed-recipe review, ROOT's
NEW compile-only invocation `bcba8e` ended `4201ac` exit 0. No test was selected
by that build. Its sole produced private harness is 291217624 bytes, SHA256
`d635cebad556a0252d308536276b3e9c9edea033943df84d4d1cfe688e75c304`.
ROOT independently admitted its ELF64LE/x64 PIE data: 12 program headers,
interpreter `/lib64/ld-linux-x86-64.so.2`, NEEDED libgcc_s.so.1/libc.so.6/
ld-linux-x86-64.so.2 and RW GNU_STACK. Installed loader/shared-library
administrative custody is accepted for these developer tests, not production
dynamic provenance or retained-FD execution.

Following separate mocked-selector gates, ROOT selected eight exact pure names
in separate original harness children (`7e3155` exit 0): payload/name, typed
slice regression, record shape, credentials/order, counts/boundary/flags,
unknown/malformed/extra records, late/second-entry sealing and empty preparation
slots. Each original wait 0 preceded exact 1 pass/0 fail/0 ignored whole stdout and
empty stderr; the fixed aggregate was 41 bytes. This did not invoke receive.

After distinct FULL ROOT and independent own-pair source/recipe review and ten
mocked controls passing, ROOT selected exactly one ignored entry (`9ad1c6` exit 0):
`restore_abort_cli::stopped_owner::retained_parent_prototype::prepared_local_reply::fresh_local_parent_prepared_reply_once`.
Its original wait 0 preceded the exact selected 1 pass/0 fail/0 ignored receipt and
empty stderr. Safe aggregate check `bc2242` recorded 43 bytes, SHA256
`8ceec1e14c7d05db6e9163cd7357bcef412137ad620e9baf8b2493c40fd971f6`.
Selector source SHA256:
`074562a6dbc99fc6c23b3ab0c9b25c4ec8cba6c96dfada1c732730b1879e08f8`.
No retry, second backend, guest, child application, root acquisition, service/
network mutation or product binary occurred. Author selected no native entry.

The unchanged ignored body composes actual current-process LocalParent capture/
consult/Bundle with one fresh nonblocking AF_UNIX SEQPACKET own pair. All outer
slots and receiver storage reserve before acquisition. One known sender sends
exact executable/pid-namespace/user-namespace originals and nonce/name bytes;
one prepared receive retains the WHOLE frame before semantic/FD checks. Exact
credentials-first/three-RIGHTS shape, no PIDFD/unknown/malformed/truncation,
borrowed-original fstat/CLOEXEC comparisons, fresh current Process recheck and
namespace comparisons, vector pointers/capacities and sealed second entry are
checked. No descriptor content read, RawFd adoption or received-authority constructor.

This is one bounded successful LOCAL known-producer composition only. Its
eight-second sampled deadline is not hard syscall cancellation. Internal
Process/LocalParent capture/clone partial acquisition and unwind, backend
installed-but-unreported FD errors, future unknown descriptor classes,
malformed tails, broader OOM/panic/process death, canonical root/child launch
authentication and normal manager adoption remain unsupported. First nonzero/
UNKNOWN permanently stops its scope without query/retry/cleanup. Source,
ordinary loader and Popen administrative trust remain explicit. Neither this
fixture nor its exact private evidence closes T4 or grants merge/release rights.

Public base 9ac CI run 37270633779 remains nongreen: DNS-broker
`simultaneous_journal_owner_is_refused` failed its isolated metadata assertion,
51 passed/1 failed/2 ignored. It is separate from the exact private68f evidence;
no public/source checkpoint inherits a whole green Rust suite from these tests.
