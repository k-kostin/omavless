# Protected Rule-policy fixture publication

This development leaf changes only the explicit ignored peer renderer's fixed
publication directory to `/run/omavless-k1-rule-rendered`. It preserves the old
Global-policy fixture and its original evidence rather than overwriting them.
The same production renderer creates the new bytes; no fixture reconstructs
configuration JSON. Ordinary runtime paths are unchanged, and the protected
coverage issuer still returns `Unsupported`.

Required separate VM observations are:

1. Fresh generated Rule policy with original validation and real DNS/TCP success.
2. The same successful baseline, then one unmarked non-DNS UDP datagram delivered
   into the TUN. Require positive inner packet evidence, no corresponding new
   outer connection/packet, and complete strict socket-generation retirement.
3. The same baseline, then one ICMP echo request delivered into the TUN. Require
   positive inner packet evidence and no external ICMP/core socket. A locally
   synthesized echo reply is not remote reachability.

Keep original-process completion, collector framing, original reaps and
same-case fixture administration distinct from product rollback. No missing
capture, timeout or incomplete generation census is a passing rejection test.
These observations cannot issue coverage or authorize native Arm by themselves.
The normal native interval, ownership/recovery faults and repeated-cycle gates
remain separate. No VM acceptance is claimed by this source-only checkpoint.
