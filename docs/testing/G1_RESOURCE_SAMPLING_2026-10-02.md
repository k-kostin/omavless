# G1a bounded synthetic resource sampling — 2026-10-02

This is a preregistered **research** check for the two independent G1a
synthetic windows, not a product performance or GUI-adoption gate. The
fixture contains only invented `.example` rows and simulated VPN states.
The installed OmaVLESS runtime, private profiles and network state are not
part of this check.

## Protocol fixed before the follow-up run

- Run on the same x86_64 Omarchy Dev VM, Wayland/Hyprland session and scale
  used for the initial G1a comparison. Use exact copied release binaries for
  both candidates and the pinned, bundled Shell app; record their hashes.
- For each candidate take **five sequential warm-cache starts**, never two
  simultaneous trial windows. Poll the compositor's client list every 50 ms
  until the launched PID owns a window. This measures *window appearance*,
  not first completed frame or perceived readiness.
- At 20 seconds after appearance sample process PSS from `smaps_rollup`.
  Measure process CPU jiffies over the next 10 seconds, reported as percent
  of one CPU core. Close only the trial PID before the next run.
- Exclude the first cold compile, offline dependency resolution and unrelated
  package installation. Stop on a failed render or PID mismatch; do not
  substitute a different test host or release binary mid-series.
- Separately inspect the 10,006-row synthetic list once per candidate and
  record PSS with the same process still open. This check is descriptive,
  not comparable to a sustained real subscription/update workload.

Provisional bounds for this VM screen, chosen **before** these five-run
samples: every window appears within 500 ms; idle PSS stays below 100 MiB;
idle CPU stays below 2% of one core during the 10-second window; expanding
the list adds less than 20 MiB PSS. A failure would call for diagnosis, not
automatic rejection of a toolkit. The current small fixture, warm cache and
virtual display make these weaker than a product budget. Frame latency,
interactive p95, cold user install, animation jank and ARM64 remain NOT RUN.

## Observations

Pending. Record exact source/binary identity, five raw rows per candidate,
sampling limitations and result before treating any of the bounds as checked.
