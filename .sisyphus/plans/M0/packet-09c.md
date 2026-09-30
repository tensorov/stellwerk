# packet-09c
plan_id: M0
goal: audit GREEN, часть 3 — dispatch-arm для ModuleCommand::Audit в cli.rs с кодом возврата 0/1 и человекочитаемым отчётом, плюс сквозные тесты на sh-фикстуре «MCP server» в tests/audit_live.rs. Транспорт и audit_module kind=mcp уже сделаны в packet-09b.
files_allowed: [crates/stellwerk-registry/src/cli.rs, crates/stellwerk-registry/tests/audit_live.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-registry
acceptance: `cargo test -p stellwerk-registry` → 0 failed; собранный `target/debug/stellwerk module audit` на sh-фикстуре с точным набором туллов → exit 0 и пустые списки расхождений; та же фикстура с лишним тулом → exit 1 и `extra_in_server`; недоступный сервер → exit 1 без паники; `kind=skill` → exit 0 с сообщением о пропуске; `cargo fmt --all -- --check` → чисто; `cargo clippy --all-targets -- -D warnings` → чисто.
forbidden: [cargo add, правка корневого Cargo.toml, правка Cargo.toml крейта, правка modules.lock, правка contract.rs, правка lib.rs/model.rs/cmd_audit.rs, правка modules.toml, async, tokio-рантайм, #[ignore], #[allow], ослабление тестов пакетов 08, 09 и 09b, изменение соседних arms в match, перенаправление аргументов пользователя в команду]
depends_on: [packet-05, packet-07a, packet-09b, packet-11]

## GREEN-пакет, часть 3 из 3

Пакет выделен из `M0-09b` после измерения: транспорт в `cmd_measure.rs` стоит **119** логических
строк (измерено: `write_message` 6, `read_response` 40, `parse_tools_list_response` 28,
`run_session` 45), `tools/call` в крейте отсутствует вовсе, и вместе с `StdioProbe`, веткой
`kind = mcp` и сквозными тестами исходный `09b` уходил примерно в 234–239 логических строки при
пределе 200. Здесь остаётся только то, что видит пользователь: команда и её приёмка.

`depends_on` объявлен на четыре пакета, и это порядок, а не формальность. `packet-09c` пишет в
`crates/stellwerk-registry/src/cli.rs`, и раньше его писали `packet-05`, `packet-07a` и
`packet-11`. Применить `09c` до них нельзя: он правит код, которого ещё нет. Проверка C1 требует,
чтобы пересечение по файлам было упорядочено хотя бы одной стороной.

## Границы этого пакета

Транспорт, `StdioProbe` и `audit_module` для `kind = mcp` — работа `packet-09b`. Здесь они только
**вызываются**. Если при проверке окажется, что им не хватает поверхности API, это FINDING, а не
правка `cmd_audit.rs` в этом пакете: `files_allowed` его не содержит.

## Что сделать

1. **Dispatch-arm** для уже разобранного `ModuleCommand::Audit(_)`, который сейчас печатает
   «not implemented (packet-09)». Arm — **в конец существующего `match`**, соседние arms не
   трогать. Имя модуля обязательно, как у `measure`; `cwd` — корень репозитория.
2. **Отчёт человекочитаемый:** списки `missing_in_server` / `extra_in_server` / `not_invocable`
   по одному на строку, либо «OK», когда все три пусты. `skill_skipped` печатается отдельной
   строкой о пропуске, а не растворяется в «OK» — иначе «аудит не выполнялся» и «аудит прошёл»
   выглядели бы одинаково.
3. **Код возврата:** 0 при пустых векторах расхождений, 1 иначе. Ошибка загрузки `modules.toml` и
   ошибка спавна — тоже 1, не паника.
4. **Недоступный сервер** (`command` указывает на несуществующий файл) → exit 1 с внятным
   сообщением, без паники и без пустого «OK».
5. **`lib.rs` не трогать.**

## Тесты — `tests/audit_live.rs`

Сквозные тесты поднимают настоящий процесс: sh-фикстура «MCP server», которая говорит на stdin /
читает stdout. Фикстура — это файл в `tempdir`, его `command` указывает на путь к скрипту.

- `tools/list` отдаёт `{"tools":[{"name":"t1","inputSchema":{"type":"object"}},{"name":"t2",…}]}`,
  на `tools/call` с любым id — `{"jsonrpc":"2.0","id":<id из запроса>,"result":{"content":[]}}`;
  id вытягивается из строки запроса. `modules.toml` с этим `command` и `tools=["t1","t2"]` → отчёт
  пустой, exit 0.
- Фикстура, чей `tools/list` отдаёт лишний тул → `extra_in_server`, exit 1.
- Фикстура, которая не отвечает → таймаут (50 мс в тесте) → `not_invocable`, exit 1.
- `command` указывает на несуществующий файл → exit 1 без паники.

Всё это уже покрыто на уровне транспорта в `tests/audit_transport.rs` (пакет 09b) **в памяти**;
здесь проверяется ровно то, чего там нет: настоящий spawn, настоящий кадр stdio, код возврата
CLI.

## БЮДЖЕТ: измерь и верни SPLIT_REQUIRED, а не подгоняй

Лимит — 200 логических строк **добавленных коммитом** для `src/cli.rs` и 250 для
`tests/audit_live.rs` (ADR-0003: `tests/` — отдельный бюджет по расположению, считается
полностью). Измерение обязательно: `git diff --numstat`, затем исключение пустых строк и строк
с `//`, с разбивкой по файлам.

**Если сумма по `src/cli.rs` больше 200 — верни `SPLIT_REQUIRED`.** **Нельзя** сжимать тесты или
выкашивать комментарии ради прохода.

## Ручная приёмка (в отчёте, **не** в тестах)

`target/debug/stellwerk module audit` против настоящего `polyforge-mcp` (в `PATH`, версия из
`modules.toml`): дословный вывод и код возврата. Если бинаря нет — `STATUS: BLOCKED` в отчёте, а
не ослабление теста. Это единственное место, где проверяется `rmcp`-живой путь на реальном сервере,
поэтому отсутствие бинаря — блокер приёмки, а не повод её сузить.

## Evidence

- GREEN вывод `cargo test -p stellwerk-registry`.
- Ручной прогон `polyforge-mcp` — дословно, с кодом возврата.
- PROOF: `evidence_append(ModelClaim)` + `evidence_verify(cmd='cargo test -p stellwerk-registry')`
  + `gate_evaluate`.
- Отдельный коммит `chore(evidence): attest M0-09c after merge` с chain_tail.
