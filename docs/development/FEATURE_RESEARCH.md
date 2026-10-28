# Feature research and external report

Use when the [operating workflow](AGENT_OPERATING_WORKFLOW.md#research-before-feature-implementation)
requires research before feature implementation or a substantial new approach.
This is the request and report procedure, not a second execution policy.

## Primary's preparation and completion

1. Read the owning feature contract and current evidence. Separate missing code,
   missing acceptance and a broken test method. Identify the concrete decisions
   research must answer, the actual stack and the existing authority boundaries.
2. Do a bounded initial review of the nearest implementations. Give the external
   researcher useful leads without requiring them to confirm the current design.
   Research and review spend the same cumulative feature budget; previous failed
   work remains recorded. Set a finite reading scope and stop once the decisions
   have supporting evidence.
3. Fill every field in the template below with shareable context. Remove secrets,
   profiles, provider URLs, raw logs and machine/process identifiers. Explain local
   evidence in words; a web researcher cannot read private worktrees or memory.
   Use full-source repository links and known revisions. State inaccessible inputs
   explicitly. Never invent source access or infer that a missing page has no code.
4. Post the **complete filled prompt in this chat**, ready for the owner to paste
   into ChatGPT Pro Deep Research. A local file/link alone is insufficient. Record
   `research_pending`; posting a prompt does not mean the research was launched.
   The owner-selected handoff does not authorize automatically sending the prompt
   to another service, using paid API processing or messaging a different chat.
5. When the report returns, treat it as evidence to verify. Check the material
   recommendations, code/tests, dates/versions and applicability at primary sources.
   In the existing checkpoint record accepted/rejected choices, reasons, remaining
   unknowns and the smallest implementation/validation plan. Preserve the report's
   provenance and read time. Mark `research_reviewed` only after this reconciliation.
   An unsupported recommendation needs a targeted follow-up, not blind adoption or
   an automatically repeated full research assignment.

Reuse an accepted report for the same feature and guarantees. A new dependency,
platform/version change or failed approach reopens only the affected question.
Closed product gates remain closed. Source integration, installed acceptance and
publication retain their existing requirements. The next phase starts when the
report is reviewed and the concrete plan is ready; another owner approval is
needed only for genuinely new scope or an explicit owner fence.

## Self-contained research prompt template

Fill all `{{...}}` fields. Keep the report request in the owner's language. The
priority order below guides search and ranking; matching behavior and required
guarantees take precedence over language popularity or recency alone.

<!-- RESEARCH_PROMPT_BEGIN -->
Ты — исследователь ChatGPT Pro Deep Research. Помоги инженеру выбрать подход
к реализации и проверке фичи на основе существующего открытого кода, тестов
и реального опыта проектов. Нужен отчёт для инженерного решения с проверяемыми
источниками. Начни с фактической даты исследования.

## Задача и контекст

{{FEATURE_AND_GOAL}}

Стек и среда:
{{STACK_AND_ENVIRONMENT}}

Требуемое поведение и границы:
{{CONTRACT_AND_BOUNDARIES}}

Что уже есть, что не доказано и какие подходы не сработали:
{{CURRENT_EVIDENCE_AND_FAILED_APPROACHES}}

Исходные материалы:
{{SOURCE_LINKS}}

Вопросы, по которым нужно принять решение:
{{QUESTIONS_TO_DECIDE}}

## Приоритет поиска

1. Самые близкие по задаче и гарантиям поддерживаемые решения на Rust для Linux;
   проекты Omarchy/Arch, systemd, Wayland/Hyprland — там, где это относится
   к исследуемому слою. Предпочитай актуальные поддерживаемые реализации.
2. Другие Rust/Linux проекты и библиотеки с полезными архитектурными решениями,
   обработкой ошибок или методами проверки для нашей задачи.
3. Прямые функциональные аналоги на Go, C, C++, Python и других языках для Linux.
   Они могут оказаться лучшим ориентиром, чем слабо связанный Rust-проект.
4. Другие платформы, исследования и более старые зрелые реализации, если они
   дают полезный метод. Явно объясни переносимость и отличия от Linux.

Это приоритет, а не фильтр исключения. Ищи достаточно широко, чтобы увидеть
свежие альтернативы и разные подходы. Не ограничивай поиск предложенными
названиями, популярными репозиториями или одним языком. Свежий прототип и зрелое
поддерживаемое решение оценивай раздельно; ни новизна, ни возраст сами по себе
не доказывают качество. Если Omarchy не имеет прямого аналога, явно перейди
к подходящему Linux-слою. Переписывать существующий компонент ради языка не нужно.

## Как проверять источники

- Проверяй свежесть на дату исследования: последние содержательные изменения,
  релизы, поддержку, версии библиотек/ОС и состояние существенных issues.
  Различай стабильный релиз, unreleased-код и эксперимент. Недавняя публикация
  статьи или правка README не доказывает актуальность реализации. Не отбрасывай
  зрелую практику только из-за старой даты, если её применимость подтверждается.
- Основные доказательства — исходный код, тесты, официальная документация,
  issues/PR с разбором сбоя и его исправлением. Обзоры используй для поиска
  первоисточников. Для важного вывода дай ссылку на конкретный файл/функцию/тест
  или обсуждение; фиксируй commit/tag/version, когда это доступно.
- Читай реализацию критического пути и её тесты, а не только README. Ищи грабли:
  гонки, устаревшие идентификаторы, отмену, тайм-ауты, частичные результаты,
  потерянные ответы и ошибки восстановления — в пределах нашей задачи.
- Разделяй подтверждённый факт, вывод и гипотезу. Не выдумывай замеры,
  выполненные тесты, доступ к репозиторию или отсутствующие реализации.
  Укажи ограничения доступа и непокрытые направления поиска.
- Сравни гарантии аналога с нашим контрактом. Укажи, что можно перенять,
  что потребует адаптации и что неприменимо. Исследование не отменяет наши
  требования безопасности, сроки операций и обязательные проверки.
- Для предлагаемой зависимости или переноса кода укажи лицензию, поддержку,
  стоимость интеграции и существенные ограничения. Заимствование идеи,
  добавление зависимости и перенос кода — разные решения.

## Ожидаемый отчёт

1. Короткий вывод: рекомендуемый путь, его основания и главная неопределённость.
2. Карта найденных подходов. Для сильнейших кандидатов — задача/стек,
   актуальность и версия, ссылки на конкретный код и тесты, полезная практика,
   расхождение с нашими гарантиями. Кратко отметь релевантные альтернативы
   и причины, по которым они не вошли в основной разбор. Не заполняй квоту
   названий слабыми совпадениями.
3. Разбор нескольких лучших реализаций и известных отказов: как устроен
   критический путь, как обрабатываются неоднозначность и завершение работы,
   что проверяется автоматически и на реальной системе.
4. Таблица решений для нашей фичи: использовать готовое / адаптировать практику /
   реализовать самим; основание, ограничения и оставшиеся доказательства.
5. Минимальный план следующего полного результата и его проверки. Раздели
   продуктовый код, инструменты тестирования и приёмку; для каждого шага
   назови критерий завершения. Не подменяй фичу бесконечным строительством стенда.
6. Неотвеченные вопросы, опровергающий тест для ключевой гипотезы и какие данные
   ещё нужны. Оценки времени допустимы с явными основаниями; неизвестное
   не превращай в обещание нескольких дней разработки.

Не запускай операции на нашей машине, не меняй репозитории и не запрашивай
приватные профили, credentials или сырые логи. Результат — исследовательский
отчёт. Дай ему возможность изменить исходное предположение инженера;
не подгоняй источники под заранее выбранный вариант.
<!-- RESEARCH_PROMPT_END -->
