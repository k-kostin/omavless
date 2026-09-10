# Native configuration-report clipboard UI

Settings reuses the existing action row and clipboard handoff to expose the
registered Rust `diagnostics.export` configuration report. This is explicitly
not a complete live support bundle: it contains counts and configured preferences,
not endpoints, profiles, URLs, controller observations or verified login state.

The frontend reconstructs a bounded allowlisted report from a strict response,
reduces unknown preset labels to `custom`, checks known instance/revision and
panel generation before clipboard handoff, and invokes the fixed native desktop
clipboard helper. Raw backend responses/errors never enter clipboard text.

No runtime ownership or network behavior changes. Python remains the legacy
reference; R5/R6 and live support-bundle parity are not complete.

Validation: full reference suite, strict parser/privacy and Service callback
tests, EN/RU catalog and QML contracts, shell syntax and plugin validation.
Required installed acceptance: explicit click copies the report, stale/closed
panel does not copy a delayed response, longer Russian labels remain legible,
and original clipboard contents are restored by automated smoke where captured.
