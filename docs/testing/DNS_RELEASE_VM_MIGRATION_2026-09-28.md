# DNS release-pair VM migration diagnostic — 2026-09-28

Scope: **agent-run diagnostic, not owner-attended acceptance or a released
installer**. This report records only the isolated x86_64 Omarchy VM. The
physical PC's VPN and network state were not changed. No profile contents,
subscription URLs, endpoint names, raw logs or screenshots are retained here.

## Exact inputs

- Source of both installed packages: `22293f1857406e587c723cc1ef9f03f310df43ae`
  (Draft #306 stacked on #305). Later workflow-only changes are not the source
  of the installed binaries.
- `omavless-0.9.0rc1-1-x86_64.pkg.tar.zst` SHA-256:
  `bae696f1a79da2a735737033a6061edd6d3ffdbc6d2a53b89ef09aacb94f3130`.
- `omavless-dns-0.9.0rc1-1-x86_64.pkg.tar.zst` SHA-256:
  `a4fc77dc5bcce0a1c929094e02acefee4c4c11b6acc24cf829394c9278a717f3`.
- Before migration, the VM had the experimental pair, was disconnected in
  Rule mode, had no TUN and retained 37 profiles. A cold, verified disk/UEFI
  checkpoint was made outside Git before package changes.

## Observations

1. The experimental broker was stopped with zero held descriptors. Its old
   enrollment was explicitly revoked at the clean boundary. A preserved
   root-owned socket initially caused the old package guard to refuse; the
   exact inspected stale socket was removed, after which the guard passed.
2. Normal `pacman -U` replaced only the two reviewed packages, including the
   expected experimental-package conflict removal. It did not enroll or start
   the new broker, connect a profile or create TUN. Installed receipt, core
   capability, package identity and unchanged private-profile count were
   checked.
3. After separate VM-only administrator enrollment and explicit broker start,
   the release template and release-only selector were prepared while
   disconnected. The user runtime started. The broker held zero descriptors
   before connection.
4. A previously screened VLESS profile connected in Rule mode. The reviewed
   TUN existed and the broker held its lease. HTTPS to a fixed IP and to a
   domain both returned HTTP 200. Switching Rule → Full VPN → Rule via the
   widget showed a neutral transition, then a settled Connected state; both
   HTTPS probes returned HTTP 200 in the settled modes.
5. The widget toggle disconnected OmaVLESS. Desired and actual state became
   Disconnected, TUN disappeared, and the user runtime stopped. The VM-only,
   narrowly scoped UFW allowance used for the HTTPS probes was removed;
   its comment was absent afterward. The new broker remained active with
   `NFileDescriptorStore=0`; the old enrollment remained absent.

## Limits and remaining gates

This is **not** a fresh plugin-first setup: packages were copied from inspected
CI artifacts and enrollment was performed as separate administrator steps.
The connected profile was screened for this diagnostic, not representative of
all subscriptions. HTTP 200 checks do not prove all routing, IPv6, UDP or
leak properties. The default-deny VM needed an explicit reversible firewall
allowance, which normal setup must not install silently. The production
download pins, two-package setup, independent authorization guidance,
fresh-install failure matrix, clean removal and owner-attended acceptance
remain open. Main/RC and public release status are unchanged.
