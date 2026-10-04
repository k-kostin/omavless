# T3 initial-mapping observation gap

Source-only successor to immutable #619
`5b95b1ad64321a0d584c0d0be25ff44c1978bc24`. No successor guest execution is
authorized by this document. The original invocation remains NONPASS; its
retained namespace, files and source must not be queried, retried or cleaned up
without a separate reviewed scope.

## Recorded evidence

The separately reviewed file-only observation completed after the first #619
wrapper NONPASS. Its private receipt SHA-256 is
`785e5c026864d530dae4897838e9b5f77099bb8d6eb4065debd3cbcf76019701`.
All nine source pins matched and original file FD stability checks passed.
Child stdout was empty; result and five after-network records were absent in
that stable-directory observation. Before-network files were hashed only: no
network preservation claim follows.

Finite child labels included seventeen before-copy labels, thirty-four
before-bind labels, both daemon-start labels and ended at `before_initial_maps`.
The recorded outer exception label was `Refused` / `owned_deadline`. These
labels are before-effect observations, not completion receipts. They do not
prove which daemon or mapping operation failed, a particular unknown ELF,
loaded object identity, current process state, namespace protection or cleanup.
The source's silent failure park and an unfinished operation cannot be
distinguished from this file evidence alone.

## Executable counterexample and corrective scope

`tests/test_t3_mapping_boundary_gap.py` injects unrelated synthetic read,
mapped-identity and unknown-public-path failures into the exact old observer.
They produce identical finite before-label transcripts. An initial bus failure
and an initial resolved failure also produce identical labels. In every case
the session remains sealed and no shutdown, reap, signal or later inventory
operation occurs. No daemon, process, namespace or host query is executed by
these controls.

The evidence-backed correction is observability **before** admission decisions,
not a relaxed manifest, map predicate, timeout or failure lifecycle. A successor
can identify fixed daemon/pass and separate live-anchor, binding, map-read,
strict-parse, membership/device-inode and target-verification boundaries.
Logging an exception after the first failure would contradict the retained
failure contract and is not proposed.

Any stronger pre-admission public map-candidate record needs its own explicit
reviewed grammar, count/byte limits and false-proof flags before implementation.
It must not read an unadmitted object or silently extend the fixed table.
The original seventeen-object manifest remains unchanged. Copy/lifecycle
changes require additional evidence; none is justified by the current labels.
