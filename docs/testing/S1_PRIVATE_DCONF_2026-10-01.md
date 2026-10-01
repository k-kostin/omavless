# S1 private persistent dconf experiment

Test implementation `f474afc23c2b4f8b12cfbef6a9e71c5b62eefc20`, stacked on
#414 (`85421a1a0d48e77d4a98523ae99386015d7a9500`). This is installed-library
evidence from isolated synthetic databases on x86_64 Linux. It is not a live
desktop restoration test or App proxy acceptance.

The fixture starts its own `dbus-daemon` with a generated configuration that
has no standard includes, activation directories or systemd integration. It
starts one explicit owned `dconf-service` child on that bus. Every child has a
cleared environment with private `XDG_CONFIG_HOME`, `XDG_RUNTIME_DIR`, cache/data
directories and a private absolute `DCONF_PROFILE` containing only
`user-db:user`. The normal desktop bus, configuration database, manager
environment, profiles, services, TUN and routes are not read or modified.

All setting operations run in separately spawned Rust test processes. The
parent never constructs a default GSettings backend. Configuration paths and
the bus address are checked before a fixture child constructs one. The fixture
owns exclusively created 0700 directories and 0600 input/observation files;
child stdout/stderr are private/discarded. Cleanup kills/reaps only its own
children and removes only its own temporary directory. Dependencies are fixed
installed paths; nothing is downloaded or installed.

## Results

Both ignored installed tests passed with dconf 0.49.0, GLib 2.88.3, dbus 1.16.2
and gsettings-desktop-schemas 50.1:

- An initial snapshot contains absent, explicit equal-default and empty-string
  overrides across the fixed 16 fields. A child writes changed synthetic values
  to all fields and waits for pending operations. A fresh process confirms
  persistence. Another child restores saved user overrides in reverse order,
  using key reset only for original absence. A new process obtains the exact
  original complete canonical layered snapshot. The database exists only under
  the fixture's config directory.
- The fixture stops its owned dconf service and waits for the kernel child-stop
  notification before starting a writer. `set_value` succeeds and the same
  backend immediately reads the target. An independent reader still sees the
  original persisted state. Resuming the owned service lets `Settings.sync`
  return; a fresh reader then observes the committed target.
- In a second run, killing/reaping that stopped service makes `Settings.sync`
  return too, but a fresh process still observes the unchanged original state.
  There is no activation directory from which a replacement service could start.

The initial fixture configuration lacked a required listen element and timed
out; that fixture construction error was corrected before the reported passes.

```sh
cargo test --locked -p omavless-s1-observer --all-features --lib \
  gio_host::persistent_tests::installed_dconf -- --ignored --test-threads=1
```

The all-feature observer suite passed serially: 31 passed and 5 installed-only
tests ignored. Its first parallel run encountered the inherited ancillary-FD
test's `EAGAIN`; the serial rerun passed without modifying that test. Strict
all-feature/all-target observer Clippy and `tests/run.sh` passed. The full Rust
script is **not PASS**: 690 runtime tests passed, 15 inherited long Unix-socket
fixtures failed under home TMPDIR (`SUN_LEN`/`SocketUnavailable`), and 7 were
ignored. This branch does not include the independent #382 fixture correction.

## Required production invariant

`Settings.sync` means pending backend operations have settled; it is not proof
of a successful commit. Reading the same GSettings backend can observe an
optimistic queued change. A future writer must verify persistence through a
fresh independent backend/process after draining the admitted operation, then
compare the complete exact expected snapshot. Another cached read is insufficient.
Timeout, lost reply, a killed helper or a dead predecessor must retain an
unknown-outcome journal and refuse automatic transfer/retry until completion
is established. A successful comparison also does not create compare-and-swap
semantics against other desktop writers.

This matches upstream dconf: the
[GSettings backend](https://github.com/GNOME/dconf/blob/264e6598863fc1d9ad6edb4c58d86f82ea669927/gsettings/dconfsettingsbackend.c)
uses `dconf_engine_change_fast`, and the
[engine](https://github.com/GNOME/dconf/blob/264e6598863fc1d9ad6edb4c58d86f82ea669927/engine/dconf-engine.c)
handles failed asynchronous commits separately while its sync routine waits
for the in-flight queue to empty. GIO's [sync API](https://docs.gtk.org/gio/type_func.Settings.sync.html)
has no success result.

These tests establish persistent backend mechanics and two counterexamples.
They do not exercise administrator defaults/locks, real desktop conflicts,
actual crash takeover, trusted manager/broker/session/profile provenance,
owned listener readiness or new-application consumption. No production
adapter, write permit, journal migration, package or UI capability is added.
The [S1 admission chain](../development/S1_ADMISSION_CHAIN.md) and exact host
restoration gates remain open.
