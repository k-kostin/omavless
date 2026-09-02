# Third-party notices

## Omarchy VPN

Parts of the OmaVLESS interface and interaction model are adapted from
[Omarchy VPN](https://github.com/jkoestinger/omarchy-vpn) by Justin Köstinger.
The project is also listed in the
[Omarchy Plugins directory](https://omarchyplugins.com/plugin.html?id=jkoestinger.vpn).
The adapted source files are `plugin/Panel.qml`, `plugin/Service.qml`,
`plugin/NamePrompt.qml`, `plugin/RenameWindow.qml`, and
`plugin/QrWindow.qml`.

Omarchy VPN is distributed under the MIT License:

> MIT License
>
> Copyright (c) 2026 Justin Köstinger
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all
> copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

## RoscomVPN routing data

The bundled OmaVLESS routing template implements the publicly documented
[RoscomVPN DEFAULT](https://github.com/hydraponique/roscomvpn-routing) policy
model. It does not bundle RoscomVPN rule databases. At runtime Mihomo retrieves
the referenced `.mrs` files from
[roscomvpn-geosite](https://github.com/hydraponique/roscomvpn-geosite) and
[roscomvpn-geoip](https://github.com/hydraponique/roscomvpn-geoip), which are
separate MIT-licensed projects maintained by hydraponique. Those remote files
remain subject to their own licenses and update lifecycle.

## China routing data

The China preset references Mihomo-native `.mrs` files published by
[MetaCubeX/meta-rules-dat](https://github.com/MetaCubeX/meta-rules-dat). The
rule databases are not bundled with OmaVLESS; Mihomo downloads them from the
project's `meta` branch at runtime. The remote project and data are distributed
under GPL-3.0 and remain subject to their own license and update lifecycle.

## Iran routing data

The Iran preset references Mihomo-native `.mrs` files published by
[Chocolate4U/Iran-clash-rules](https://github.com/Chocolate4U/Iran-clash-rules).
The rule databases are not bundled with OmaVLESS; Mihomo downloads them from
the project's `release` branch at runtime. The remote project and data are
distributed under GPL-3.0 and remain subject to their own license and update
lifecycle.

## Design references

[Mihoro](https://github.com/spencerwooo/mihoro) and
[omarchy-mihoro](https://github.com/huacnlee/omarchy-mihoro) were consulted for
Mihomo service and API behavior. The separate subscription-management page,
private URL handling and explicit refresh interaction were also informed by
omarchy-mihoro's MIT-licensed multi-subscription work (PR #7). No source file
from either project is bundled verbatim; their code remains subject to the
respective project licenses.

The Rust-first WireGuard/AmneziaWG import foundation was designed against
[Amnezia Client](https://github.com/amnezia-vpn/amnezia-client) for native
configuration fields, generation markers and Qt `qCompress` guest-link
encoding, and against [Mihomo](https://github.com/MetaCubeX/mihomo) for the
`wireguard` / `amnezia-wg-option` mapping. No source file from either project is
bundled verbatim; the checked upstream revisions are recorded in
`PROTOCOL_ROADMAP.md`.
