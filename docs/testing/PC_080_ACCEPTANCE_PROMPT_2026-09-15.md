# Omarchy PC: финальный пакет 0.8.0 и новый путь установки

Продолжи `k-kostin/omavless` на физическом x86-64 Omarchy PC: свежий Git →
согласованный кандидат → сборка/установка → приёмка → сохранение результатов.
Выполни все доступные безопасные шаги, не останавливайся после аудита или CI.
Этот prompt заменяет передачу от 2026-09-14.

## Полномочия и границы

Не создавай тег/релиз, не загружай release assets и не меняй marketplace без
отдельного прямого разрешения владельца. Передача не даёт нового разрешения на
merge. Локальная интеграция, тестирование и сохранение полезной работы в PR
разрешены. Пароль вводит владелец только в OS/sudo, никогда в чат.

**Не повторяй полную R6-приёмку на неизменном коде:** R6 закрыт в согласованной
области. Проверяй новые установочные и x86-64 gates, после fix — затронутые
сценарии. Не начинай TUI, P4/WG/AWG, kill switch или новую миграцию. Не меняй
реализации Draft #30/#135. AUTO-1, V0 missing-family и записанные DNS/provider
findings не становятся PASS автоматически. Не использовать новые protocol keys.

## 1. Обнови контекст и выбери кандидат

В checkout: `git fetch origin --prune`, затем сравни HEAD/remote/main, dirty state,
открытые PR и недавние ветки. Не стирай чужую работу. Контрольная точка:

| Объект | Точка / состояние |
| --- | --- |
| main | `56800b0a1f05eb83f8a761a20772153883c914f1` |
| Принятый ARM runtime source | `b7fd0a99b8b169f0933e5f43ea4389642015193a` |
| #249 `fix/marketplace-first-run-setup` | Код/UI: `4e2f17f88b230e4d43e05b738c6cf6f4c6caea43`; docs: `329b77a660a46ef142eee5a907f8824cfb63fc17`; Draft, package pins пусты |
| #250 `docs/080-marketplace-preparation` | `cf55fcacc56e6fc46598961298f0c9df08c1242e`; ready, CI PASS; текст и безопасные снимки со странами/городами |
| #251 `fix/release-frontend-pairing` | Код: `789238f82869531a680fc5e5241b66eb7f8e97bc`; отчёт: `cc1470f588671be094bc61bb3196c506ac0c5aea`; ready, CI PASS; этот prompt добавлен следующим docs-only коммитом |
| Frozen archive | `archive/python-legacy` → `aa5873783c019edc303a732e55ea8c85f1f0b090`; сохранить без изменений |

Свежие remote HEAD авторитетнее таблицы. VM-агент после передачи прекращает
запись в этих ветках; перед записью проверь, не продолжил ли их другой агент.
Изменённый SHA требует изучить diff, не автоматически повторять все тесты.
Это один продукт: общий QML frontend и нативные ARM64/x86-64 пакеты, не разные
плагины. Версию 0.8.0 повторно повышать не нужно.

Прочитай полностью актуальные AGENTS, DEVELOPMENT_ROADMAP, CURRENT_STATUS,
DEVELOPMENT_WORKFLOW, ACCEPTANCE_ENVIRONMENTS, RUST_MIGRATION и:

- `docs/testing/NATIVE_080_FINAL_CANDIDATE_2026-09-14.md` до конца: начальная
  pending-таблица историческая, фактические результаты ниже;
- `docs/testing/NATIVE_080_RELEASE_HANDOFF_2026-09-14.md`;
- `docs/testing/MARKETPLACE_FIRST_RUN_2026-09-15.md` из #249;
- `docs/testing/NATIVE_FRONTEND_PAIR_2026-09-15.md` из #251;
- `docs/testing/HOST_AUTHORIZATION_ACCEPTANCE.md`;
- `packaging/release/README.md`, `packaging/release/FRONTEND_README.md`,
  `docs/user/NATIVE_INSTALL.md`;
- `docs/marketing/MARKETPLACE_080.md` из #250, если трогаешь описание/снимки.

Merged PR повторно не накладывай. Если они ещё open — чистый временный
integration checkout от свежего main с уникальными изменениями #249 → #250 →
#251 после проверки диапазонов. Это не разрешение merge или постоянный стек.
Запиши frontend SHA. Конфликт/fix исправляй в owning PR, не только в scratch.

## 2. Сохрани принятую VM-приёмку

Rust-only runtime, scoped R6, реальные initialize/activate/onboarding с уже
установленным пакетом, stable ARM64 package и отдельный Full VPN HTTPS/TUN/
disconnect/restore gate приняты. **Это не clean marketplace provisioning**.
Startup Off по умолчанию; Last/pinned fresh-login не принят.

ARM package `omavless-0.8.0-1-aarch64.pkg.tar.zst`:
`454662a76f106af5b2f4ee8ab3ef4626a1641981fd9e6baa0a2ca8d91dc7983d`.
Embedded ELF: `12afa0a6ae279d23f1b89426d47fdd478e0ef1987f171a92924b9c925f6f0458`.
Это не x86-64 хеши. Оговорка package-runner: владелец опечатался в `settled`,
его финальный marker отсутствовал; итоговые ELF/units/private fingerprints
проверены независимо, отдельный live gate завершился PASS. Историю не переписывать.

Combined scratch `67491a71765cfb88c715e3aa688dd226fc3139be`: код #249, начальный
#250 и #251; **275 Python total / 273 PASS / 2 SKIP**, JS/QML PASS; настоящий
ARM-пакет использован повторно без изменения байтов. Последующий #250 меняет
только подписи/снимки, повторные parser/refusal/QML/visual и CI PASS. Scratch
не опубликован для fetch: воспроизводи из составляющих PR. Не называй его
новейшим финальным кандидатом или скачанным релизом.

## 3. x86-64 пакет и frontend-пара

Без вывода секретов определи состояние PC: архитектура, package version,
native/legacy ownership, connection/startup, процессы/TUN, существующий рабочий
VLESS fixture. Сохрани приватный backup и recovery archive до изменения
установки. Состояние VM не является состоянием PC.

Выбери чистый reviewed runtime source A. Если runtime/package inputs не менялись,
можно строить тот же `b7fd0a...`, что принят на ARM, в отдельном checkout. Если
remote принёс runtime fixes, выбери актуальный A и затронутые gates; не игнорируй
их и не приписывай старому ELF новый SHA.

Выполни применимые локальные `./tests/run.sh`, `./tests/run-rust.sh`, compile,
shell/JSON/diff/plugin validation. Не заменяй их лишними GitHub workflows.
Собери настоящий бинарник и архив, сохранив toolchain/build log:

```sh
cargo build --release --locked -p omavless-runtime --bin omavless
python3 packaging/release/build-candidate.py /absolute/empty-output /absolute/built/omavless FULL_RUNTIME_SOURCE_SHA --stable
```

Подставь проверенные пути вне Git. `--offline` для Cargo — при полном кеше.
Проверь ELF/package SHA-256, architecture, schema-3 identity, payload, units,
dependencies, отсутствие hooks/private data. Не использовать тестовый ELF.

Для frontend B новее A используй #251 в чистом checkout B:

```sh
python3 packaging/release/pair-frontend.py /absolute/empty-pair-output /absolute/reviewed/omavless-0.8.0-1-x86_64.pkg.tar.zst FULL_FRONTEND_SOURCE_SHA REVIEWED_X86_PACKAGE_SHA256
```

Требуются ancestry и неизменность runtime/build/package inputs. Не ослабляй
allowlist ради успеха. Сохрани A/B, `frontend-pair.json`, `SHA256SUMS` и build
evidence. Пины можно добавить в B после получения хеша A; не требуй рекурсивный
«один SHA для всего». Пустые pins — NOT READY для публичной установки.
Две архитектуры должны доставлять один выбранный общий frontend; разные bytes
нельзя публиковать под одним именем frontend release asset.

## 4. Установленный PC-кандидат

Следуй реальному starting state и `NATIVE_INSTALL.md`:

- Новый пользователь: reviewed package → initialize → activate → matching frontend.
- Уже native: без повторной initialize/activate; disconnected update и frontend
  с сохранением данных. Frontend-only update не заменяет runtime package.
- Legacy: startup Off, disconnect, штатные preflight/activation preconditions;
  без ручных markers и второго owner.
- Уже та же 0.8.0: сравни bytes/историю. `--upgrade-only` требует строгое повышение
  и архив реально установленного старого пакета. Не обходи проверки и не меняй
  версию ради теста; используй применимый документированный путь.

Авторизующие эффекты — в настоящем терминале, `ready` до и `settled` после всех
OS-диалогов согласно host procedure. Не auto-ack, не лавина запросов, не сброс
PAM/polkit, не короткий дедлайн. Для удаления данных/сброса ради clean test нужно
отдельное согласие и проверяемое восстановление. Desktop actions — по Omarchy skill.

Короткая фактическая матрица:

1. Running ELF/units/frontend identity, native owner, enabled, данные сохранены.
2. EN/RU main/Settings/import; clipboard/file profile и subscription URL доходят
   до правильной confirmation; без лишних постоянных подписок-дубликатов.
3. Subscriptions, refresh, Copy report с безопасным version 0.8.0.
4. Рабочий VLESS connect/disconnect, actual profile (не выделенная строка), Full
   VPN/обычный Routing; bounded HTTPS/DNS, один runtime/core/TUN, private Unix
   controller, отсутствие attributable TCP controller.
5. Startup Off, shell restart neutrality, подтверждённый Quit/reopen/cleanup
   по затронутым gates. Last/pinned и NIC/suspend не имитировать.

Ошибка сначала классифицируется: harness/runtime/UI/provider/network/fixture/
restoration. Воспроизводимая поломка обычного соединения — release blocker;
scoped R6 её не оправдывает. Fix — в owning PR с регрессией и повтором только
затронутых проверок.

## 5. Guided first run — отдельный gate

В #249 pins пока пусты. До реальных утверждённых assets скачивание → pacman →
activation из панели: **NOT RUN — RELEASE ASSETS/PINS UNAVAILABLE**. Не заполняй
production pins заглушками/несуществующими ссылками. Выполни доступные PC/package
gates и верни этот конкретный остаток; ручная установка не заменяет guided E2E.

Только после отдельного разрешения на публикацию пакетов и доступности точных
immutable assets: закрепить architecture/source/SHA pins, проверить реальные
bytes (не только HTTP 200), собрать B и пройти
`plugin add → panel → Required components → terminal setup → Check again → onboarding`.

Отдельная матрица публикуемого пути для ARM64 и x86-64:

- app/core отсутствуют; совместимый core уже есть; app есть, activation нет;
- уже native — нет повторной initialize/activation/reinstall;
- Set up later → reopen сохраняет missing-component blocks;
- Finish later в мастере завершает wizard без профиля, не скрывая недостающие компоненты;
- cancel до INSTALL, отказ после частичных эффектов, ошибка скачивания;
- один setup terminal, ни одной повторной попытки до закрытия всех auth prompts;
- успешная установка/activation, startup Off, честные optional-helper/TUN
  readiness и рабочее обычное VPN-подключение.

После отмены позднего шага предыдущие эффекты могут сохраниться; автоматического
отката нет. Inspect → Check again → оставшийся документированный шаг. Stale
lock/interrupted ownership не удалять принудительно. Терминал и исчезнувшие
component cards не доказывают readiness/успех.

Standard installation / Manual setup определяет marketplace maintainer по
продемонстрированному пути, не агент. Marketplace отдельно одобряет владелец;
не обновляй его автоматически вместе с release assets.

## 6. Что сохранить и вернуть

Безопасный подробный отчёт в `docs/testing/` через commit/push/PR, точные current
main/A/B/local-remote identity и CI, фактический delivery status без переписывания
истории. Не печатай/коммить приватные names/IDs/URI/endpoints/UUID/keys/passwords/
subscription URLs, raw private errors или QR. Снимки — с безопасными названиями;
не подменять Connected и не выдавать fixture rendering за живую приёмку.

Краткий русский отчёт владельцу:

1. Git/PR/A/B; что merged, что нет; кто продолжает запись в ветках.
2. Реальные x86-64 artifacts/hashes/пути, installed identity, данные сохранены.
3. Test counts и PC matrix PASS/FAIL/NOT RUN + причины.
4. Отдельная guided matrix по архитектурам, не смешанная с ручной установкой.
5. Дефекты/fixes; реальные блокеры отдельно от AUTO-1/V0 follow-ups.
6. Финальные mode/connection/startup/runtime/core/TUN/manual recovery/plugin/repo state.
7. Следующее необходимое действие владельца и что НЕ опубликовано.

Честный вердикт:

- **PC PACKAGE ACCEPTANCE PASS — GUIDED PUBLIC INSTALL PENDING**;
- **0.8.0 ACCEPTANCE INCOMPLETE — REAL GATES REMAIN**;
- **0.8.0 RELEASE BLOCKED**;
- **0.8.0 READY FOR OWNER MARKETPLACE APPROVAL** — только после реальных package,
  host и guided gates на заявленных архитектурах, когда остаётся действительно
  лишь отдельное согласование marketplace.

Не запрашивай церемониальную повторную R6-приёмку и не публикуй сам.
