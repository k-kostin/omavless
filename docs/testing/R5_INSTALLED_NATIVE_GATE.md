# Installed native acceptance tooling

`tests/installed_native_acceptance.py` preserves the installed Full VPN smoke
used for the September 9 Try Omarchy ARM64 native activation checkpoint. Python
here is a **developer test tool**, not part of the production runtime path.
This harness does not prove all of R5/R6 or other protocol families.

Default invocation reports NOT RUN. Actual execution requires both flags:

```bash
python3 tests/installed_native_acceptance.py --run --authorize-socket-inspection
```

Run only with owner authorization and no concurrent VPN/ownership work. It
uses fixed `/usr/bin/omavless`, existing committed Rust ownership and current
user installed state, never supplied profile IDs, alternate binaries or paths.
Startup must be explicitly configured disabled, the owner disconnected and
legacy service inactive. It selects only the existing last usable VLESS record.
Missing fixtures or unsupported template policy report FIXTURE UNAVAILABLE;
the script never imports credentials, rewrites a template or activates ownership.

The supported fixture class has exactly one nonzero mixed proxy port, explicit
`allow-lan: false`, IPv4 loopback bind, system TUN stack and no other proxy/TCP
controller keys. This is a tested metadata policy, not an arbitrary YAML parser
or a claim that other configurations fail. Canonical Rust/Mihomo still validate
the full configuration. Generated controller configuration must be Unix-only.

Connected acceptance requires one Mihomo in the installed service cgroup and
daemon descendant tree, one visible TUN, a private PID-authenticated controller,
and only expected new loopback proxy/TUN-forwarder listeners. Pre-existing host
listeners are excluded. Every new listener needs both inode and exact core-PID
proof from the fixed read-only `pkexec /usr/bin/ss -H -ltnpe` inspection. This
test-only privileged observation requires the separate flag; ordinary human
authorization dialogs have no short artificial deadline. It adds no privileged
runtime API. Missing proof never becomes PASS.

The generic HTTPS probe is bounded and explicitly bound to the observed TUN;
counter growth and HTTP result are separate evidence. A `finally` block requests
disconnect, checks no remaining core/TUN/controller and daemon-only cgroup,
then restores the original disconnected mode. The existing daemon stays active;
there is no service start/stop/enable, package action or ownership rollback.
Failed cleanup is manual recovery, not permission to alter host networking.

Private responses stay in memory. Public output contains fixed case labels,
booleans, timing and bounded classifications only; raw subprocess/parser errors,
private names, record IDs, endpoints and credentials are never emitted.

References: [native service acceptance](../../tests/native_service_acceptance.py),
[native action bridge](R5_NATIVE_PLUGIN_ACTIONS.md), and
[Arch capability contract](../roadmap/ARCH_SERVICE_SECURITY.md). The original successful
ad-hoc probe is replaced by this checked-in harness, not imported from a machine
specific workspace. Deterministic parser/authorization/privacy tests run through
`tests/run.sh`; executing them never starts VPN or asks for authorization.
