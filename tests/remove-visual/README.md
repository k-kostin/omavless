# Bounded removal-review rendering

This opt-in fixture renders the actual RemoveReview.qml and I18n.js, with their
local PlainText.qml and OmaScrollBar.qml dependencies. It is not installed Panel
navigation, native-state or remover acceptance. Stage only those four production
files and this shell.qml in a private owned guest directory
created with `mktemp -d "$HOME/.cache/ovt/remove.XXXXXXXX"`. Do **not** stage
remove-entry.sh or tools/uninstall.sh; this fixture must not execute removal.

Link Commons, Ui and services from the **measured running shell root**. Set
OMAVLESS_REMOVE_REVIEW_DIR to that directory and OMAVLESS_REMOVE_REVIEW_ENTRY to
its file://RemoveReview.qml, then start the existing Quickshell on that exact
directory. Address only that instance with `qs ipc -p DIRECTORY call removeReview`.

`scenario en|ru 360|600 ready|native|setup|setup-terminal|terminal|failed` sets
explicit synthetic states. `inspect` returns public focus, scroll and blocker
facts; `capture SLUG` starts a PNG and `result` must return ready before viewing.
Invalid scenarios refuse capture instead of reusing an older frame. Review both
locales at both heights, all text by scrolling, Tab/Shift+Tab and Escape. Do not
activate removal in this fixture. End only this instance with `finish`.

The separate installed journey must navigate actual Panel entry points, launch
the original pinned local helper and cancel at its initial REMOVE confirmation.
It must preserve user profiles, native/network state and existing maintenance
custody; no destructive wipe or reboot merely for screenshots.
