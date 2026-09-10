# Native traffic counters and graph

The Python-era reference is `Service.qml`'s sysfs RX/TX collection,
`applyTraffic`, and the checked-in six-case `traffic-cases.json` corpus. Rust
owns bounded counter acquisition; QML retains presentation-only rates/history
and reuses the existing Sparkline and DetailPair components.

`runtime.traffic` / fixed `runtime traffic` accepts no client parameters. A
fresh coherent connected owner, desired profile, mode/selector readiness and
one visible core/TUN are required. The PID-authenticated private Unix controller
must explicitly report an enabled TUN and a bounded device name. The reported
device is checked again after reading that exact sysfs device's TUN flags,
ifindex and counters. Missing/changed/ambiguous observations return unavailable,
never fabricated zero throughput. No arbitrary interface or host path is accepted.

The response scope is `controller_attributed_tun_counters`: this is attribution
through our authenticated core, not a claim that Linux exposed the core's TUN
file descriptor. Try Omarchy's capability-bearing Mihomo correctly denies the
user runtime access to its fdinfo. The initial fdinfo-only implementation was
therefore unusable on that host; no dumpability/capability/OS security change was
made. Synthetic fdinfo cases remain a reference, not the production admission.

Total acquisition has a 500 ms budget, individual counter reads 100 ms, strings
and file sizes are bounded, and counters remain within exact JavaScript integer
range. Responses expose opaque session identity and daemon-monotonic sample time,
not device names, PIDs, paths, controller secrets or profile metadata. Rates use
monotonic sample deltas; first samples, identity changes and counter resets show
unknown rates until a valid second sample. History is at most 30 points and is
cleared on stale/closed/disconnected/changed-instance context.

Sampling is every two seconds while the main panel is visible, or when the
existing bar-throughput preference is explicitly enabled. No graph is presented
for unavailable observations. TUN traffic can include traffic later sent direct
under Routing; these counters are activity, not confidentiality/route proof.
No durable store, VPN desired state, controller transport or privilege changes.

Validation includes Rust attribution/bounds/reset tests, shared actual-QML
counter parity, eight JS parser/rate/staleness/graph tests, and real Quickshell
component compilation. Installed exact-head counter movement and lifecycle
regression remain required before merge. Python remains oracle/rollback; this
does not complete ICMP monitoring, subscription probes, R5 or R6.
