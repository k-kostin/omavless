# K1 inactive durable root-state foundation

This stacks on the isolated packet test foundation. It does **not** install or
start NetGuard, enable kill switch, change any firewall, provision directories,
or connect a production runtime. Only private temporary fixtures are exercised.
The optional capability remains unavailable to users.

## Fixed state and trust

The only public opener targets `/var/lib/omavless-netguard/armed-v1.json`.
It walks `/`, `var`, `lib` with fd-relative no-follow directory opens and checks
root ownership and no group/other write access. The final directory must be
root:root `0700`; the leaf root:root `0600`, regular, single-link, at most 4096
bytes. Nonblocking/no-follow opens reject FIFOs and symlinks without hanging.
A nonblocking directory `flock` is held for the store lifetime. All future
helper instances must cooperate with this lock, including across the complete
kernel observation/transaction, not only writes. It is advisory, not protection
against a malicious root administrator.

The final directory's name is rebound and compared with the held descriptor
before operations. Leaf identity is checked again after bounded reading. No
caller-supplied path, mode, rule, shell command or endpoint exists in this API.
Tests have a private constructor for their own temporary directory and UID/GID;
there is no public alternate-path or alternate-owner API.

The flat JSON object has exactly these fields:

```json
{"version":1,"policy_version":1,"enrolled_uid":1001,"generation":7,"armed":true,"flags":0}
```

The UID above is synthetic. Actual trusted enrollment must come from a future
root-owned enrollment mechanism, not from an untrusted IPC field. UID zero,
mismatched UID, unknown versions/flags/fields, duplicates, arrays, floats,
negative/overflowed generations, trailing documents and malformed data refuse.
No profile name, provider address, key, credential or private log is stored.

## Durable ordering and uncertain writes

The caller passes an expected observed marker and a permitted successor.
Missing can become Armed; Armed(N) can retry itself or become Closed(N);
Closed(N) can retry itself or arm a strictly greater generation. Invalid cannot
be overwritten through this API. Exhaustion does not wrap.

Persistence performs, in order:

1. Validate expected state under the held lock.
2. Exclusively create fixed `.armed-v1.json.next`, mode `0600`.
3. Write bounded bytes and fsync that file.
4. Atomically rename within the same directory.
5. Fsync the directory and read back the expected marker.
6. Only then allow a successful persist-effect acknowledgement.

Any uncertain write poisons the store instance. Any leftover staging entry,
even an empty file or dangling symlink, projects to Invalid on reopening.
Nothing automatically deletes, adopts or repairs staging files. A failure after
rename can leave the new record visible but does not acknowledge the old
transaction: a new reconciler must inspect all facts. The tests inject failure
at all six boundaries; these are deterministic process/error simulations, not
a claim about physical power-loss behavior on all filesystems.

The planner orders Arm as install → verify → durable Armed → success. Disarm is
durable Closed → remove proven-owned rules → verify absence → success. Failure
to persist Closed prevents deletion. The closed high-water fence survives reopen.

## Reconciliation is not a storage-only decision

Missing in a verified private directory is only a storage observation. Missing
directory, unsafe/unreadable file, unsupported version or staging artifact is
Invalid, never Missing. The future adapter must turn opener errors into a
manual-recovery/protection decision, not fresh-install success.

- Missing/Closed plus verified absent table can be DisarmedVerified.
- Missing/Closed plus independently proven-owned stale table requires removal
  and verified absence before disarmed status.
- Invalid plus absent/proven-owned table requests the fixed Emergency policy;
  only its successful verification may claim EmergencyProtected, always with
  manual recovery required.
- Foreign/unreadable table refuses mutation and reports manual recovery without
  claiming protection, even if the marker is missing or invalid.

This file is intent, **not a kernel table receipt**. Table ownership cannot be
inferred from its existence, UID or generation. Existing independent
boot/netns/table identity admission and the future root enrollment/receipt
lifecycle remain mandatory. No fixed-name foreign table may be adopted.

## Remaining gates

Root provisioning/enrollment and service lifecycle, production authenticated IPC,
kernel receipt durability and boot reconciliation, crash/power-loss installed
tests, upgrade/removal recovery and runtime coordination are not implemented.
Neither root-state tests nor the earlier isolated packet tests authorize feature
activation. No installed root path is opened during this slice's test suite.

Relevant syscall contracts: [open](https://man7.org/linux/man-pages/man2/open.2.html),
[rename](https://man7.org/linux/man-pages/man2/rename.2.html),
[fsync](https://man7.org/linux/man-pages/man2/fsync.2.html) (file sync does not
replace directory sync), and [flock](https://man7.org/linux/man-pages/man2/flock.2.html).
