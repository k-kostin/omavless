# Disposable-user startup inventory prerequisite — source only

This separately versioned capture precedes any creation of UID/GID 48044,
account `ov-t4-abort-v1`, login session or user manager. It does not authorize
those effects. The native scaffold remains the independently reviewed
`2bfedf3ce203ad639c766ca5dcd8c4d35f5f2c38`; no ignored native entry has been
invoked by these source tests.

`startup_inventory.py` is trusted source delivered through stdin to isolated
Python (`-I -B`), with the one literal `--capture-t4-user-startup-v1` argument.
It requires all real/effective/saved IDs to be root. It has no subprocess,
generator execution, manager query, account creation or network operation.
Its only writes are exclusive files in the create-only root-owned 0700
`/run/ov-t4-user-startup-inventory-v1`. Existing output is a refusal, not a
resume request. Partial artifacts are retained after any failure.

The fixed roots include global user units/search paths, user and environment
generators, environment configuration, XDG autostart entries, user-manager
configuration, real `user@`/runtime-dir templates and instance/type drop-ins,
PAM inputs and the two manager/runtime executables. Regular source bytes,
xattrs, metadata, absent entries and symlink text/normalized targets are
private evidence. Only the fixed outcome marker and aggregate SHA are printed.
No raw source content, private path list or exception text belongs in Git or
shareable execution logs.

All original regular-file and ancestor directory descriptors remain held
through capture and publication rechecks. Re-entering a known directory uses
its original descriptor, not a replacement. Files require root ownership,
single link, non-writable group/other mode, stable metadata and repeated
original-FD SHA; FIFOs and other unexpected types refuse before opening.
Symlinks are recorded and followed only into finite declared configuration or
package namespaces (or the typed `/dev/null` mask). Unsupported symlinked
ancestors refuse. Missing roots are pinned to their first absent component.
Live directory enumeration is bounded; no unbounded completed list is used.

Bounds are 4096 records/entries, depth 32, 8 MiB per file, 32 MiB total source
bytes, 48 MiB serialized output and a 45-second checked capture budget. Xattrs
are limited to 32 names and 4096 bytes each. These are local operation bounds,
not a kernel-I/O cancellation or power-loss durability guarantee. First
uncertainty seals the inventory. No retry, cleanup, fallback or later process
observation is introduced.

## What the receipt does not prove

The schema always says `semantic_admission: false`. This is a source snapshot,
not a computed effective systemd graph, package authenticity proof or proof
that starting a manager cannot activate an application. Generator code may
consult additional inputs; unit `EnvironmentFile` and executable references
may require separately reviewed bounded follow-up captures. All actual bytes,
links, default targets, generator code and references must be independently
reviewed before proposing a manager-start gate. Do not disable generators,
install inert replacement units, mask unknown entries or infer absence from
an incomplete capture.

Actual delivery, frozen-source hashes, full parent/peer review and an explicit
exclusive VM lease remain required. No VM capture, account activation,
canonical UID1000 change or whole T4 acceptance is claimed here. Every earlier
NONPASS and exact process-loss receipt remains unchanged.

The ordinary pure tests use only synthetic HOME-private files. They exercise
retained descriptor reuse, byte-identical replacement/in-place edits,
FIFO-before-open refusal, link escape/cycle, absent-to-present changes,
deadline/count/size/read bounds, permanent refusal and exclusive publication.
