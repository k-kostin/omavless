# Descriptor-free synthetic actor research

Historical model checkpoint only. The separate opt-in
[Rust manager actor service](../../../docs/development/T4_MANAGER_ACTOR_SERVICE.md)
now adds executable code under its own approved availability fault boundary.
This unchanged model and its receipts do not supply the service's real gate.

This fixture is not a receiver, process implementation or production API.
The Rust model has no descriptor, PID, syscall or IPC operation. Completion
facts are fabricated public booleans; model Drop only changes a label and does
not preserve an actual descriptor table through failure or death.

The unchanged `actor_state.rs` has six pure controls. ROOT selected its exact
private source commit `054305502de47bea19997fccb9606a73146fb9e4` and SHA
`e4784dba06a3014991f9e65296a4e839133f79e1b00cc9e872afed172b148664`
through a separately reviewed developer recipe. Eight mocked recipe controls
returned original zero (`d80550`, projection `92d2d7`). The one whole
compile/six-pure selection `b9affc` returned original zero and the fixed marker
`T4_SYNTHETIC_ACTOR_COMPILE_PURE6_KNOWN_ZERO`, with produced binary SHA
`13e16b87960c559f7b641df03fd45c7e4f632de8fe9647e689278d4b2980ec63`.
These are ROOT-supplied exact-source receipts, not author capture inspections.
No actor acquisition, socket, descriptor import, namespace switch or VM was
selected by that recipe.

[Provenance](provenance.json) binds the model, selected external source recipe
and finite result. No captured raw log, local wrapper, private source path or
binary is retained here. The first two unselected external recipes remain
preserved: review found delimiter aliasing and then missing sampled final-output
checks. The selected successor uses whole literal-LF receipt parsing, sampled
pre/post full-integer write, sampled pre/post flush and a final budget tick.

`compile_recipe.py.in` and `mock_recipe.py.in` are inert textual exports, not
selected commands. They have no entrypoint or source/compiler/scope/module
binding. Relocation changes their bytes and they do not inherit external recipe
execution evidence. A future selection requires new fixed bindings, source and
tool admission, environment/loader assumptions, budget, fresh scope, both FULL
reviews and a separate mocked gate. The sampled budget does not preempt blocked
syscalls; a visible marker alone does not establish original terminal zero.

Ordinary source gates run only [text/AST export controls](../../test_actor_state_export.py).
They neither compile nor execute the Rust model or either template.
The four new text/AST controls passed. The full ordinary source suite passed
654 Python controls with two existing skips and the JS/QML contracts. The first
new text assertion refused an AST pretty-printer parenthesis difference; its
structural AST comparison was corrected before these final source gates.
The crate/workspace manifests, lock, shared backend and accepted T4 contract
are unchanged. The inherited #653 public Test gate was negative while package
gates passed; this research receipt does not relabel that head green.

[Actor-service feasibility](../../../docs/development/T4_ACTOR_SERVICE_FEASIBILITY.md)
records a possible different trust boundary and its missing prerequisites.
A nonfatal service may keep imported descriptors internal while alive. Fatal
death makes that service unavailable; an unreaped task or pidfd cannot promise
descriptor-table survival. This fixture is not a substitute for #653's local
Bundle or unconditional partial-error custody, and it grants no merge,
production adoption or native selection authority.
