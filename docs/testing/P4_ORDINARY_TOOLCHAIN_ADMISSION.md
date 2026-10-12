# P4 ordinary installed-toolchain admission successor

SOURCE only: no Go command, test ELF, source export, engine selector, package
decompression, installed-tool-tree observation, network/service or VM action was
performed by this source checkpoint. The previous [runner proposal](P4_DEFAULT_RESIDUE_RUNNER_PROPOSAL.md)
and [developer export evidence](P4_GO_DEVELOPER_EXPORT_RESET.md) retain their
original identities and outcomes. This successor does not inspect those old
scopes, adopt their developer tree or change a failed receipt.

## Executable scope

The separate [ordinary entry](../../tests/fixtures/p4_awg_peer/run_default_residue_ordinary.py)
keeps the original runner frozen at SHA-256
`e58c23773429d3c2854b60f1a5b5d1c11b36ac39c5e700ae21e6237f7a907343`.
It retains the exact supervisor/helper/parser/three overlay pins, separate
BUILD/EXECUTE phases, fixed selectors, resource upper bounds, no cleanup path,
unknown-owner latch, exclusive attempt and final-receipt ordering. The engine
defaults, causal assertions and 570/600/620-second bounds are unchanged.

Only ordinary CGO0 is admitted: fixed `/usr/bin/go`, explicit
`GOROOT=/usr/lib/go`, `GOTOOLCHAIN=local`, and the existing complete offline
environment. There is no race, CC/CXX, caller environment, alternate tool path,
diagnostic command or timing override. The old race proposal remains separate.
This is developer Python build/receipt tooling; normal runtime ownership remains
Rust and normal P4 stays unavailable.

## Independent package provenance and reached input closure

ROOT independently verified the detached Arch signature of cached package
`go-2:1.27.0-1-x86_64.pkg.tar.zst`, SHA-256
`232a5228e0305cd42a2ed2517806c78ff34ec9b0e8ba879a20914a8f7a31f5a9`,
size 44,314,959 bytes, with signer fingerprint
`2E36D8620221482FC45CB7F2A91764759326B440` (Lukas Fleischer, Arch Linux).
The trusted boundary is ordinary administrator/package installation; this is
not a claim of hostile-root or atomic filesystem attestation. A local initial
verification against an armored keyring failed; ROOT's separately dearmored
private keyring copy produced the actual GOOD signature without global changes.

The [read-only checker](../../tests/fixtures/p4_awg_peer/ordinary_toolchain_admission.py)
has no CLI, subprocess, extraction or execution capability. On a separately
selected invocation, Python 3.14's standard-library zstd decoder reads the
fixed root-owned package in memory under byte/count bounds, checks its independent
digest, and builds a full path/type/mode/size/content catalogue. No archive
member is written. Installed `/usr/lib/go` must have exactly those directories
and files: no omitted compiler, stdlib, assembly, object, support or data input,
additional file, symlink, hard link, extended attribute or writable ancestry.
The only executable-selection symlink is root-owned `/usr/bin/go`, literal
target `/usr/lib/go/bin/go`; its canonical bytes must match SHA-256
`3144268876ba974458f06ab523581fabac9e967855e2a1bb9de9ba950167b542`.

The required native Go/compile/asm/link/test2json/vet paths are actual x86-64
ELF64 objects. Every executable ELF in the catalogue is inspected for program
header bounds, PT_INTERP and DT_NEEDED. Dynamic interpreter/shared dependencies
refuse rather than borrow the top-level Go executable's static result. Executable
source wrappers are hash-attested full-tree inputs, not automatically invoked
dependencies of this fixed CGO0 Linux/amd64 command. The checker does not use
`ldd`. A changed package/runtime dependency closure needs its own review.

The build/execution records bind package/signature provenance, explicit GOROOT,
Go hash, full tree catalogue digest/count, exact link, ordinary recipe identity,
all nine captured source-input hashes and existing binary/default/resource
fields. The full toolchain observation repeats before each Go command, before
execution-attempt consumption, and before final success issuance. Directory
membership/identity and the original link are rechecked within each observation.
Its 60-second budget is sampled, not a hard cancellation or atomic-tree proof.

## Remaining selections and gates

ROOT and independent review must inspect this changed source plus affected
call sites and the concrete environment/resource closure before selecting any
Go command. ROOT separately prepares and reviews an offline private module
snapshot against the pinned AWG `go.mod`/`go.sum`; no module provenance or actual
module-copy result is manufactured by this checkpoint. Existing `go mod verify`
still has to complete under the original supervisor before the zero-case
`go test -c` phase. Build output/ELF and original known-zero closure must be
inspected before ROOT separately selects exactly one ordinary reject EXECUTE.

The argument shape remains the previous proposal, using the new entry filename
and distinct ROOT-selected HOME scratch/cache/module/artifact directories.
No actual scope is selected in this document. Both cases exist in source, but
the first requested actual scenario is reject. Exhaust and race are not implied
by a reject result. First failed/uncertain execution seals that snapshot; no
resend, cleanup, old scope observation, normal P4 activation or release follows.

The [inert controls](../../tests/test_p4_ordinary_toolchain.py) build only
fabricated tar/ELF bytes and mocked runner command/export/admission paths. They
do not read a real package/tool tree, decompress a real archive, execute Go or
prove engine behavior. Full ordinary developer-suite and exact-head review
results belong on the owning Draft PR; installed/native/VM gates remain unrun.
