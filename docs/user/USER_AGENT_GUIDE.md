# Help a user operate OmaVLESS

This is a guide for an agent helping an end user, not instructions for
developing the repository. Read the [matching user guide](NATIVE_INSTALL.md)
first. Match the guide to the user's installed release; an older panel may
offer different actions. Identify its components before choosing a repair.

## Preserve the user's intent

- Installing components, opening a panel, refreshing a subscription or reading
  diagnostics does not authorize connecting or disconnecting a VPN.
- Explain the next operation and its effects before acting. Updates keep
  profiles/settings; full removal deletes them without a backup.
  A desktop restart requires the user's agreement and saved work.
- Recommend turning off other VPN connections before setup or an OmaVLESS
  connection. Do not stop, uninstall or change another VPN client yourself.
- Passwords belong only in the operating system's authentication prompt, never
  chat, scripts or command arguments. Never approve an unidentified prior prompt.
  Terminal words such as `INSTALL` and `DNS` are confirmations, not passwords.
- Keep profile links, UUIDs, keys, subscription URLs, configuration, backups and
  raw private logs out of Git, shared output and command arguments. Use the
  bounded **Support report** rather than dumping a user's files.

## Identify the components before choosing a repair

The widget checkout, running shell, native application and DNS package can have
different identities. Updating or reinstalling the widget does not update
native packages. A `0.9.8-1` package label alone does not distinguish maintenance
deliveries.

For a standard installation these are read-only metadata checks. Missing files
or packages are useful observations, not permission to create or remove them:

```bash
git -C "$HOME/.config/omarchy/plugins/kdk.omavless" rev-parse HEAD
git -C "$HOME/.config/omarchy/plugins/kdk.omavless" status --short --untracked-files=no
pacman -Q omavless omavless-dns
grep -E '^(sourceCommit|productVersion|architecture|binarySha256)=' \
  /usr/share/doc/omavless/build-identity.txt
```

The build-identity file is declared provenance, not proof of currently executing
bytes. If a build mismatch matters, locate files using
`pacman -Ql omavless omavless-dns`, then compare their SHA-256 with the selected
immutable release's expected executable identities. Do not assume the first
`omavless` or `mihomo` on PATH is the package-owned executable. Archive digests
and installed executable digests are different. Do not invent a missing
expected identity from an unrelated development installation.

The application package is `omavless`; `omavless-dns` contains both the matching
Mihomo VPN core and DNS helper. The latter manages website-address lookup for
OmaVLESS connections, not selection of a public DNS provider. A separately
installed Mihomo is not its substitute and must be preserved.

## Choose the supported user path

| Goal or observed state | Action |
| --- | --- |
| First installation | Use the [ordinary plugin add and guided installation](NATIVE_INSTALL.md#install); setup does not connect a VPN. |
| Partial installation | Finish or cancel the current terminal and its OS prompts, then **Check again** and follow the offered remaining action. |
| Helper already authorized | Use **Start DNS helper**, not first-time authorization because a prior start failed. |
| Helper setup incomplete | Follow [Set up DNS helper](NATIVE_INSTALL.md#continue-dns-setup): `FIRST` only for a genuinely first authorization; `START` uses the existing one. |
| Access removed by a completed clean reinstall | Use the offered **Restore helper access** path. This is not profile Restore or a universal retry. |
| Update while keeping data | Update the widget, then follow [the offered component update](NATIVE_INSTALL.md#updates), including the ordinary shutdown/preparation prerequisites. |
| User wants complete removal | Use the [checked full-removal helper](NATIVE_INSTALL.md#remove), including its restart and same-command continuation. No manual cleanup recipe is needed. |
| Widget already removed | The same guide downloads and verifies the helper; no developer cache or working panel is required. |
| Unknown or modified build / other account's setup | Retain the specific refusal and establish a separate explicit plan. Do not disguise it as a supported public installation. |

`INSTALL`, `DNS`, `FIRST`, `START`, `SELECT`, `PREPARE`, `UPGRADE`, `REMOVE`
and `REBOOT` are terminal confirmations, not commands to type in a normal
shell. Follow the current prompt; pressing Enter may cancel that step. A later
failure does not undo every earlier step, and typed consent is not completion.

For component updates, **Shut down OmaVLESS**, re-enable only the widget, then
choose **Update components…** without starting the application first. Existing
DNS authorization stays in place. The public 0.8.2 **Update OmaVLESS** route
adds the matching DNS package and has separate DNS setup afterward. The original
0.9.8 broker cannot use automatic preparation; report that restriction rather
than bypassing package guards.

## Keep actions and targets distinct

- Clicking a profile name selects its management target, not its connection.
  Use **Connect beside the intended server** to connect or switch servers.
- The top toggle disconnects an existing connection; it does not switch to a
  newly selected profile. **Direct** retains the TUN and is not Disconnect.
- Subscription refresh updates server lists, not latency or VPN health. A
  failed refresh does not prove the list changed.
- **Open app** opens/focuses the terminal client of the same runtime. Closing
  it or pressing `q` leaves a running VPN and accepted background work alone.
- **Shut down OmaVLESS** stops/disables the application and plugin but preserves
  data/packages. **Start OmaVLESS** is separate from connecting the VPN.
- Backup creation is offered in the terminal application. **Restore is not
  available in 0.9.8**; code in a development branch is not a supported control.

## Observe completion, not just a closed window

1. Finish or cancel the current terminal and all related OS prompts. A generic
   failure is not proof of a pending sudo dialog.
2. Acknowledge **Terminal and system prompts are closed** only when true, then
   choose **Check again**. Use the newly offered action, not an earlier one.
3. Setup completion should reach the usable panel. **Core ready** and
   **Connected** have different meanings. An authorized connection test also
   needs the requested real network checks, not only traffic counters.
4. Report what was actually measured: a synthetic/loopback connection is not
   public Internet access; an HTTPS delay check is not a DNS-leak test or proof
   of the complete routing policy. Evidence from x86_64 is not an ARM64 install.
5. Removal completes at **OmaVLESS and its saved profiles and settings have been
   removed**, with the selected components actually absent. Preparation,
   reboot or a safe refusal alone is not completed removal.

The helper removes guarded packages before asking Omarchy to remove the widget.
If widget removal is cancelled, packages can already be absent while private
data and progress remain. Resume with the **same cached helper**. Do not delete
its records, unmask services or remove sockets/DNS/network rules to force a
successful-looking result. If shutdown or authorization has an unknown outcome,
preserve that context and report it before retrying.

## Help without turning support into development

For stale controls only, after all prompts are settled and with the user's
agreement, `omarchy restart shell` reloads the bar, panels and other plugins.
It does not repair native packages or a real runtime failure. Repeatedly
removing/re-adding the widget is not a native recovery procedure.

Use **Settings → Support report → Copy report / Save file** when available.
Give the attempted action, component identities, short stage/error and the
available next step. Review even a bounded report before publishing it. Ask for
only the minimum additional facts, not a private configuration in a public issue.

Do not change permissions, root services, firewall policy, removal progress or
package contents to bypass a supported refusal. Never force package replacement
or substitute an experimental build without a separately explained decision.

Source contributions belong in a separate development checkout following its
own instructions. Do not turn the user's installed widget into a worktree,
switch it to an experimental branch, push changes, release or submit to a
marketplace as an incidental support action.

[User guide](NATIVE_INSTALL.md) · [Short removal guide](CLEAN_REINSTALL_098.md) ·
[Troubleshooting](TROUBLESHOOTING.md) · [Privacy and security](SECURITY.md)
