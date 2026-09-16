# Documentation map

## Using OmaVLESS

Start with the [project README](../README.md). User instructions live in
[`user/`](user/INSTALL.md): [installation](user/INSTALL.md),
[native candidate installation](user/NATIVE_INSTALL.md),
[native everyday use](user/NATIVE_USAGE.md), [legacy usage](user/USAGE.md),
[troubleshooting](user/TROUBLESHOOTING.md) and [security](user/SECURITY.md).
The published marketplace version and the locally accepted native candidate
are different installation paths; do not mix their recovery commands.

## Developing and reviewing

Start with [CONTRIBUTING.md](../CONTRIBUTING.md) and the
[development documentation map](development/README.md). Agents read
[AGENTS.md](../AGENTS.md) and its required [detailed guide](development/AGENT_GUIDE.md).
The [delivery roadmap](../DEVELOPMENT_ROADMAP.md) and
[architecture/roadmap index](roadmap/README.md) retain their established paths.
The [current delivery status](roadmap/CURRENT_STATUS.md) separates the accepted
native application, published marketplace snapshot and remaining follow-ups.
Native release assembly and owner-controlled publication gates are described
in [packaging/release](../packaging/release/README.md).
Reusable project workflows live in [`skills/`](../skills/omavless-ui-review/SKILL.md),
not in the installed frontend payload.

## Current native integration evidence

Use the [testing index](testing/README.md), beginning with the local R6 closure
and publication candidate. Dated reports preserve observations at their actual
tested heads, including failures. They are not independent current task lists.

Repository layout: `plugin/` contains QML and presentation modules; `crates/`
contains Rust; `packaging/` contains host packaging; `tests/` contains isolated
tests and synthetic corpora. Root launchers/manifest remain where the supported
plugin installer expects them. Generated builds, private fixtures, captures and
live-result files do not belong in this documentation tree or in Git.
