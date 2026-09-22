# Contributing to OmaVLESS

Start with [current delivery status](docs/roadmap/CURRENT_STATUS.md) and the
[delivery roadmap](DEVELOPMENT_ROADMAP.md). Check open issues and PRs before
starting overlapping work. Agents additionally follow [AGENTS.md](AGENTS.md)
and its mandatory detailed guide.

## Source layout

| Area | Responsibility |
| --- | --- |
| `plugin/` | QML frontend and presentation modules |
| `crates/` | Rust application, runtime and domain logic |
| `packaging/` | Host packaging and offline release assembly |
| `templates/` | Runtime configuration templates |
| `tests/`, `tools/` | Isolated checks, recorded fixtures and developer tooling |
| `docs/user/` | Installation, controls, troubleshooting and security |
| `docs/development/` | Agent guidance and documentation/retention policy |
| `docs/roadmap/` | Architecture, feature contracts and delivery status |
| `docs/testing/` | Durable acceptance evidence and bounded handoffs |
| `skills/` | Reusable project review/localization procedures |

Root manifest/launchers/install entry points remain where Omarchy expects them.
The source repository is not the installed payload: the release assembler ships
an allowlisted frontend, not agent guidance, tests or developer tooling.

## Workflow

Use a narrow `dev/<topic>` branch from current main and an early meaningful
Draft PR. Optional kind prefixes include `dev/fix/<topic>` and
`dev/docs/<topic>`. A temporary `rc/<version>` freezes a named integration
candidate, not a second permanent product or a separate set of docs.
Follow [the canonical workflow](docs/roadmap/DEVELOPMENT_WORKFLOW.md) for ownership,
exact-head gates, affected rechecks, merge approval and cleanup. Existing open
branches need not be renamed for cosmetics. Published releases use immutable
version tags and an exact reviewed marketplace snapshot.

Main is the stable release snapshot, documentation included. Docs-only PRs do
not have automatic merge permission. Keep daily status in issues/PRs, include
the next roadmap/docs revision in the named RC, and complete the workflow's
release reconciliation checklist before an explicitly owner-authorized main update.

## Checks

```sh
./tests/run.sh
./tests/run-rust.sh
git diff --check
```

These are local development checks, not proof of live networking or installed
UI behavior. Read the [testing index](docs/testing/README.md) and
[acceptance policy](docs/roadmap/ACCEPTANCE_ENVIRONMENTS.md) to select additional
gates. Host/lifecycle scripts are not an unattended bulk test suite; preserve
the user's VPN and follow their authorization requirements.

For docs-only work, verify local links, discovery paths and any affected tooling.
Never repeat unchanged R6/live acceptance merely to update prose. CI may still
run its normal full suite. Runtime changes need owning regression coverage and
the applicable exact-head host gate.

## Documentation and privacy

Keep README focused on installing and using the product. Put architecture,
test counts, source identities and engineering decisions in the relevant docs
or PR, following [the retention policy](docs/development/README.md).
Preserve useful historical evidence; do not delete reports simply to shorten a
directory listing. Private fixtures, exported configurations, backups, raw logs,
unreviewed screenshots and build outputs never belong in Git.

To discuss a problem, provide the bounded support report, reproduction and
versions; do not post private profiles, endpoints, subscription URLs or secrets.
