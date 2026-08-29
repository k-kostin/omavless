---
name: omavless-localization
description: Add, review, or verify OmaVLESS UI localization with source context, exact-head visual state coverage, locale formatting, and screenshot privacy.
---

# OmaVLESS localization

Read the repository `AGENTS.md` and the complete
`docs/roadmap/I18N.md` before acting. That document owns locale policy, string
ownership, the current screen/state matrix, formatter gates and privacy rules.

Treat localization as a context-and-rendering task. Inspect a string at its QML
sink and in related states before translating it. Preserve stable semantic
keys, complete English fallback and bounded named interpolation. Provider,
profile, endpoint, core-rule and diagnostic data remains untranslated and must
stay in plain-text or sanitized sinks.

Catalog, contract and syntax tests are necessary but do not prove visual
acceptance. Install the exact candidate in Try Omarchy, exercise every affected
matrix state in each changed locale, inspect scrolling surfaces completely and
review captures for meaning, fallback leakage, truncation, overlap, wrapping,
focus and mixed dynamic text. Fix the owning catalog/formatter/string boundary,
then recapture affected and adjacent states.

Screenshots containing private fixture identity or network metadata are private
evidence even if their directory is named `shareable`. Publish only a redacted
state/result matrix unless an explicit pixel-level privacy audit passes. Never
commit acceptance captures with real fixtures.

When a new locale introduces different plural, number/date/currency, casing,
spacing, Unicode, line-break or bidirectional behavior, add the relevant tested
formatter/layout boundary before claiming that surface supported. Do not infer
RTL readiness from catalog lookup alone.

Finish by restoring normal locale selection and a disconnected healthy runtime.
