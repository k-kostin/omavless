# Installed QML compilation gate

Run `bash tests/test-qml-load.sh` inside Try Omarchy, or pass an absolute
candidate `plugin/Panel.qml` path. Requires installed Quickshell, Omarchy
imports and a working Wayland session (exit 77 if absent).

The isolated shell compiles the component graph without creating the plugin.
It does not instantiate Service, access profile state, or change the tunnel.
Wayland is required because Omarchy's PanelWindow type needs its backend even
for compilation; offscreen cannot validate this graph. Each runner has a
private temporary configuration and a five-second lifetime bound. Raw engine
errors are not published. A positive compile marker is mandatory.

Self-check: `valid.qml` must pass and `invalid.qml` must fail. They cover the
Process/default-property error that textual QML contracts previously missed.
This is not visual acceptance or proof that bindings and actions execute.
No production package files or runtime ownership change.
