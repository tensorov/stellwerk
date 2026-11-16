---
id: a01b
milestone: V1
status: GREEN
depends_on: [a01]
files_allowed: [crates/stellwerk-task/src/m5/gate_channel.rs, crates/stellwerk-task/tests/gate_channel.rs]
acceptance: cargo test -p stellwerk-task --test gate_channel
max_diff_lines: 200
overlap_waiver: C9 — N2/00i1, N2/00j2 — общие файлы gate_channel, последовательное исполнение в одном worktree
---

# a01b — таблица рисков не выдаёт пишущий `Allow` без digest

## Цель
Ветка `AllowBasis::RiskTable` в `answer_for` возвращает `GateAnswer::Allow { tool }`
безусловно (`crates/stellwerk-task/src/m5/gate_channel.rs:264`). Для пишущего
инструмента с путём это решение по паре `(tool, path)` без содержимого: файл можно
подменить после авторизации, а `AuthorizedCall` не выдаётся вовсе. A01 закрыл эту
дыру только для `AllowBasis::ApprovedPlan`. a01b закрывает её для таблицы рисков.

## Что измерено (до правки)
- `crates/stellwerk-task/src/m5/gate_channel.rs:264` — `AllowBasis::RiskTable => GateAnswer::Allow { tool }`, без проверки `requested` и без `authorize_write`.
- `crates/stellwerk-task/src/m5/gate_channel.rs:268-287` — соседняя ветка `ApprovedPlan` уже требует digest для пишущего вызова: `Some(requested)` → `authorize_write`, `None` при пишущем вызове с путём → `Deny` с названной причиной.
- `crates/stellwerk-gateway/src/gate.rs:38` — `writes_content(tool)` определяет, какие инструменты пишут содержимое.
- `crates/stellwerk-task/src/m5/gate_channel.rs:312-314` — legacy `edit` без аргумента и digest даёт `Ok(None)`: «typed-запроса не было», а не разрешение.

## Шаг 1 — падающий тест
`crates/stellwerk-task/tests/gate_channel.rs`, тест
`risk_table_allow_of_write_requires_content_digest`:

1. `edit` с путём, аргументом и верным `content_digest`, политика отвечает
   `Evaluated { basis: AllowBasis::RiskTable, .. }` и `Verdict::Allow` →
   ожидается `GateAnswer::Authorized`, чей `scope` связан с digest.
   Сегодня приходит `GateAnswer::Allow` — тест падает.
2. `edit` с путём и **без** аргумента/digest при том же вердикте → ожидается
   `GateAnswer::Deny` с названной причиной (`write_without_content_digest`).
   Сегодня приходит `GateAnswer::Allow` — тест падает.

Оба утверждения проверяют одно: `Allow` таблицы рисков не является разрешением
на запись; запись разрешает только `AuthorizedCall`, связанный с digest.

## Шаг 2 — минимальная правка
В `AllowBasis::RiskTable` повторить дисциплину ветки `ApprovedPlan`:
- `writes_content(&request.tool) && request.path.is_some()`:
  - `requested` = `Some(..)` → `authorize_write` → `Authorized` / `Deny`;
  - `requested` = `None` → `Deny` с причиной `write_without_content_digest`.
- иначе (не пишущий вызов или нет пути) → `Allow { tool }` как раньше.

## Приёмка
cargo test -p stellwerk-task --test gate_channel
Ожидается PASS.

## Запрещено
- нельзя выдавать `Allow` на пишущий вызов с путём без digest;
- нельзя выводить digest из текста модели или из пути;
- нельзя менять `AllowBasis`/`PolicyVerdict`/`GateAnswer` ради обхода теста;
- нельзя трогать `crates/stellwerk-gateway/**` — эта дыра уже закрыта там, правка локальна каналу.
