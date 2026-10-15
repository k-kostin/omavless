# OmaVLESS glossary

Use these names consistently in the interface, setup terminals, user guides
and support replies. Explain a component's purpose at its first mention;
afterwards use its short name. This is a terminology reference, not an
installation procedure or evidence that a candidate has been released.

## Components and state

| English | Russian | Meaning |
| --- | --- | --- |
| Omarchy plugin / widget | Плагин / виджет Omarchy | The bar icon and panel. Installing/updating it does not install/update native packages. Closing its panel does not stop the application; persistent plugin disable/remove requests application shutdown. |
| OmaVLESS application | Приложение OmaVLESS | The native program that manages settings and VPN connections. Starting it and connecting a VPN are separate actions. |
| VPN core / Mihomo | Ядро VPN / Mihomo | The program that carries VPN traffic. OmaVLESS uses its bundled core; an unrelated existing Mihomo installation is not silently replaced or removed. |
| DNS helper | DNS-помощник | Sets up website address lookup for OmaVLESS VPN connections. Normal disconnect removes only OmaVLESS's DNS settings. It is not the VPN core or a choice of public DNS provider. |
| Helper access | Доступ к помощнику | Permission for this computer account to use the DNS helper. It is not a VPN profile or a saved server password. |
| Component update | Обновление компонентов | Update the native application and its VPN core/DNS-helper package together. Updating only the widget does not update these packages. |
| Setup | Настройка | Install missing components and prepare the application/helper. It is not permission to connect a VPN. |
| Setup assistant | Помощник настройки | The guided review of components, routing and profile import. Readiness review is not a connection test. |
| Profile | Профиль | Saved connection settings for a VPN server. Selecting a profile for management does not connect it. |
| Subscription | Подписка | A source of server profiles. Updating its server list is different from updating installed application components. |
| VPN login autoconnect | Автоподключение VPN при входе | A preference to connect at login. Starting a helper service at computer boot does not enable this preference. |
| Unconfirmed result | Неподтверждённый результат | The operation's outcome is not known. It is not proof that nothing happened or permission to repeat the operation. |

The two native packages are `omavless` (application) and `omavless-dns`
(the matching Mihomo VPN core **and** DNS helper). Name their roles separately:
do not rename the helper to “DNS core” or present a standalone Mihomo update
for this managed installation.

## Setup action names

These are the actual EN/RU catalog labels. Terminal instructions and guides
name the visible action, not an invented synonym. The `↗` suffix marks a web
link; prose may omit the suffix. The ellipsis in an update entry means that
more steps and confirmations follow, not that installation starts immediately.

| Catalog key | English | Russian |
| --- | --- | --- |
| `setup.install_all` | Install required components | Установить необходимые компоненты |
| `setup.prepare_update` | Update components… | Обновить компоненты… |
| `setup.resume_dns` | Set up DNS helper | Настроить DNS-помощник |
| `setup.start_broker` | Start DNS helper | Запустить DNS-помощник |
| `setup.restore_enrollment` | Restore helper access | Восстановить доступ к помощнику |
| `setup.start_app` | Start OmaVLESS | Запустить OmaVLESS |
| `setup.terminal_closed` | Terminal and system prompts are closed | Терминал и запросы системы закрыты |
| `setup.check` | Check again | Проверить снова |
| `setup.guide_short` | Setup help ↗ | Помощь ↗ |
| `setup.later` | Set up later | Настроить позже |
| `native.app.open` | Open app | Открыть приложение |

Restore helper access is only for access removed by a completed clean
reinstall. It is neither restoring profiles nor a fallback after a failed
helper Start. “Core ready” describes core setup, not an active tunnel or
working internet. “Open app” opens the terminal interface; closing that
window does not shut down the application or disconnect its VPN.

## Terminal confirmations

Confirmation words stay exactly as shown, in both languages. They are not
password prompts. Enter without a word cancels the offered step; cancellation
does not undo earlier completed steps.

| Word | Meaning |
| --- | --- |
| `INSTALL` | Begin the offered missing-component setup. |
| `DNS` | Separately permit the helper for the displayed account and start its service now and at computer boot. This does not enable VPN autoconnect. |
| `FIRST` | Select first-time helper setup, only if it has not already been authorized. |
| `START` | Use an already successful helper setup on this computer without repeating authorization. Failure does not justify switching to FIRST. |
| `SELECT` | Select the installed VPN core and helper with supported defaults; custom DNS settings are not silently overwritten. |
| `PREPARE` | Prepare an update and, if needed, safely stop the helper. Does not install packages. |
| `UPGRADE` | Install the offered matching application and core/helper packages. Start/helper continuation is separate. |
| `RELOAD` | Optionally reload the whole Omarchy shell. Other shell plugins reload too; this is not VPN recovery. |
| `REMOVE` | Confirm the displayed removal, including permanent deletion of OmaVLESS profiles and settings. Other VPN clients are kept. |
| `REBOOT` | Restart this computer after removal preparation. Save other work first; run the same removal helper after logging back in. |

## Editorial use

- Lead with the current state and the next reachable action. Use short
  sentences; disclose effects and cancellation at the relevant confirmation.
- Describe what this operation did, not unobserved global state. “This setup
  did not connect a VPN” does not claim that every VPN on the computer is off.
- Keep implementation terms such as broker, enrollment, ownership and matched
  pair in developer documentation or explicit diagnostics. User prose uses
  helper, helper access and component update instead.
- Do not translate command names, consent words, diagnostic codes, provider
  data or credentials. Do not hide system/pacman prompts behind simplified prose.
- Change terminology across all affected owners in one task: QML catalog,
  shell, native prompt formatter and current user guide. Existing historical
  evidence keeps its original labels and source identity.

The [UI/UX contract](docs/roadmap/UI_UX_CONTRACT.md) owns action semantics;
the [localization policy](docs/roadmap/I18N.md) owns locale and rendering rules.
Published help follows [current main](docs/development/README.md#public-guides-follow-main).
