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

The five-run series completed on the x86_64 Omarchy Dev VM (Hyprland
0.56.2, 1890×2080 virtual output at scale 1.6667). The direct source was
the `dev/g1-direct-a11y-scope` candidate from PR #469; its release binary
SHA-256 was `03534d5bb2ebb5bf8a2e5caefa846e5579ffea4fa6e30be60dcaf44b240c158a`.
The Shell app was the `dev/g1-shell-narrow-keyboard-scroll` candidate from
PR #472; its `main.js` SHA-256 was
`d209db2905023e85dc3bc9beed6589a43de9d1c8c3ce445934a7bcb45c49e611`.
Its separately pinned `gpui-shell` host binary SHA-256 was
`1ece32dceeb8ffbdb3131f5b73a6083d89c929f1407494382ecc34b1ef9b332b`.
Both candidates used the same invented fixture and default synthetic
connected scene. Runs were sequential and no trial windows remained afterward.

| Candidate | Run | Window appearance, ms | Idle PSS, KiB | Idle CPU, % of one core |
| --- | ---: | ---: | ---: | ---: |
| Direct Rust | 1 | 109 | 51,481 | 0.50 |
| Direct Rust | 2 | 110 | 50,898 | 0.40 |
| Direct Rust | 3 | 110 | 51,363 | 0.50 |
| Direct Rust | 4 | 113 | 51,358 | 0.40 |
| Direct Rust | 5 | 110 | 51,324 | 0.50 |
| GPUI Shell | 1 | 109 | 77,887 | 0.10 |
| GPUI Shell | 2 | 109 | 77,386 | 0.10 |
| GPUI Shell | 3 | 109 | 77,658 | 0.10 |
| GPUI Shell | 4 | 109 | 77,059 | 0.20 |
| GPUI Shell | 5 | 109 | 77,820 | 0.20 |

Median sampled PSS was 51,358 KiB (direct) and 77,658 KiB (Shell).
The measured window-appearance median was 110 ms and 109 ms respectively.
All *preregistered VM research bounds* above were met. These values do not
establish first-frame time, total application memory, or a performance winner:
PSS accounts for shared mappings, and the two prototypes and hosts are not
identical implementations. The 50 ms compositor polling interval also limits
the precision of the appearance result; millisecond-looking values are not
millisecond-resolution evidence.

The large-list check was a separate fresh process for each candidate. After
20 seconds of idle, opening the **10,006-row synthetic list** and waiting two
seconds produced these within-process samples:

| Candidate | Before, KiB PSS | After, KiB PSS | Delta, KiB |
| --- | ---: | ---: | ---: |
| Direct Rust | 51,123 | 54,171 | +3,048 |
| GPUI Shell | 77,735 | 85,791 | +8,056 |

The controls visibly switched to `5 samples`, and both windows stayed
responsive. These one-shot differences meet the provisional <20 MiB bound,
but are not a steady-state subscription memory budget. No screen-reader,
real provider, VPN, backend IPC, cold install, frame pacing, ARM64 or
production integration was tested. Neither prototype is ready to replace
the QML frontend on the strength of this sampling alone.

## Follow-up: ordinary launch without network

Each exact candidate above was also launched separately under the VM's
`bubblewrap --unshare-net` network namespace, with a fresh temporary `HOME`
and `XDG_CACHE_HOME`. The trial process had a different network namespace
inode from the VM session; inside the namespace only loopback existed and
there was no IPv4 route. Both candidates rendered their normal synthetic
connected screen from their local binary/app files. The two trial windows
were stopped, and only their isolated temporary cache directories (about
2.2 MiB of Mesa shader cache each) were removed.

This checks a *warm binary's offline launch*, not a clean installation or
proof that the Shell host never tries network access in every code path.
The whole-filesystem bind left system and pre-existing non-HOME caches
available, and neither candidate had external app dependencies. A clean
machine package/update/remove trial is still NOT RUN.
