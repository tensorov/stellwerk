---
id: a02b
milestone: V1
status: GREEN
depends_on: [a02, c01, c02]
files_allowed: [crates/stellwerk-task/pi/stellwerk_transport.ts, crates/stellwerk-cli/src/build_runner.rs, crates/stellwerk-cli/tests/host_tools.rs]
overlap_waiver: C9 — N2/00i1, N2/00j6, N2/00h3b, N2/00h3c, N2/00h3d, N2/00j1 — общие файлы transport.ts/build_runner.rs, последовательное исполнение в одном worktree
acceptance: cargo test -p stellwerk-cli --test host_tools
---

# a02b — хост исполняет авторизованный edit, а не Pi

## Цель
`run_build` применяет к workspace только `AuthorizedCall`, полученный от гейта, а extension больше не пишет файл сам: успех, собранный из ответа Pi, не отличает исполнение хостом от исполнения внутри Pi. Это отдельный пакет от `v01`, потому что `v01` остаётся тестом, а перенос эффекта в хост требует правки `transport.ts` и `build_runner.rs`, которых нет в `files_allowed` пакета `a02`.

## Обоснование (измерено)
- `crates/stellwerk-cli/src/build_runner.rs:326-345` — `serve_gate_until_stopped_logged` вызывает `decision.into_answer()` и теряет `GateDecision`; `apply_gate_decision` на строке 422 уже существует, но из `run_build` не вызывается.
- `crates/stellwerk-cli/src/build_runner.rs:858` — поток гейта запускается без корня workspace, поэтому применить авторизованный вызов в нём нечем.
- `crates/stellwerk-task/pi/stellwerk_transport.ts:128-140` — `writeHost`/`editHost` пишут файл через `writeFileSync` уже после `authorized`; хост байты не получает.
- `crates/stellwerk-task/pi/stellwerk_transport.ts:111-125` — `hostCall` отправляет `JSON.stringify(params)` как argument; для `edit` это описание правок, а не итоговые байты файла. Хост не может записать то, что авторизовал.
- `crates/stellwerk-task/pi/stellwerk_transport.ts:339-375` — `edit` и `write` вызывают `hostCall`, затем сами исполняют эффект.

## Шаг 1 — падающий тест
`crates/stellwerk-cli/tests/host_tools.rs`, тест `authorized_gate_decision_is_applied_by_the_host`: поднимает настоящий `GateChannel` и `serve_gate_until_stopped_logged` с корнем workspace, отправляет в сокет `write`/`edit` с digest итоговых байт и требует, чтобы файл изменился хостом, а digest после записи совпал с авторизованным. До связывания `GateDecision` с `apply_gate_decision` файл не меняется, и тест падает на содержимом.

## Шаг 2 — минимальная правка
1. `build_runner.rs`: `serve_gate_until_stopped_logged` получает корень workspace и для каждого `GateDecision` с `authorized()` вызывает `apply_gate_decision`; ошибка `HostRefusal` не глотается, а возвращается как ошибка потока гейта. `serve_gate_until_stopped` остаётся обёрткой.
2. `run_build`: передаёт `&work` в поток гейта; после `join` ошибка применения авторизованного вызова делает прогон неуспешным.
3. `stellwerk_transport.ts`: для `write` отправляет `params.content` как argument, для `edit` — итоговый текст после применения `oldText -> newText`; digest считается по этим байтам. После `authorized` extension возвращает результат, но **не** вызывает `writeFileSync`/`editHost`. `read` остаётся как есть.
4. Удалить `writeFileSync` из импорта, если он больше не используется; `editHost`/`writeHost` не оставлять как второй путь записи.

## Приёмка
cargo test -p stellwerk-cli --test host_tools
Ожидается PASS.

## Запрещено
- нельзя оставлять второй путь записи в extension: после `authorized` файл пишет только хост.
- нельзя считать `Allow`/`Deny`/`Ask` разрешением на запись; писать можно только по `AuthorizedCall`.
- нельзя ослаблять digest или сверять его только до записи.
- нельзя трогать `v01`-тест в этом пакете; `v01` остаётся отдельным test-only пакетом.
- нельзя добавлять `#[allow]`/`#[ignore]` и менять lint-политику.
