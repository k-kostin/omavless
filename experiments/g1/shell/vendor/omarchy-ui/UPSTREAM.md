# Bundled Omarchy UI for the isolated G1 experiment

The files in `src/` and `LICENSE` are an unmodified copy of
[Jason Lee's `omarchy-ui`](https://github.com/huacnlee/omarchy-ui) at commit
`8def54298bda03b8d481436e62b5885af4eed674` (MIT). The bundled copy is
limited to this synthetic GUI comparison, not the OmaVLESS runtime or plugin.

GPUI Shell's Git dependency resolver fetches even a pinned reference at launch.
Bundling the audited source lets the experiment check startup without network
access and makes the distribution cost explicit. Updating it requires a new
source review, license check and exact-commit comparison.
