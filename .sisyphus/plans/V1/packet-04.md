---
id: 04
milestone: V1
status: GREEN
depends_on: [r01, r04, a01, a01b, a02, a03, v02, o02]
files_allowed: [crates/stellwerk-cli/tests/run.rs, crates/stellwerk-gateway/src/gate.rs, crates/stellwerk-gateway/tests/gate.rs, crates/stellwerk-task/src/m5/gate_channel.rs, crates/stellwerk-task/tests/authorization.rs]
limit_waiver: 5 файлов — механическая правка fmt после влитых R01/A01/A03, путь исполнения не меняется, но путей больше трёх
overlap_waiver: C9 — N1/packet-01, N1/packet-09, N2/packet-00e, N2/packet-00i1, TUI/packet-02 — общие файлы с более ранними milestone, последовательное исполнение в одном worktree
acceptance: cargo test -p stellwerk-task --test authorization
---

# 04 — остаточный fmt после R01/A01/A03, закреплённый тестом

## Цель
Дерево снова проходит `cargo fmt --all -- --check`, и это закреплено тестом, а не только прогоном руками: расхождения остались в файлах уже влитых пакетов, поэтому без стража они вернутся при следующей правке любого из них.

## Почему приёмка — `cargo test`, а не сам `cargo fmt`
`scripts/integrator/accept.sh:166-169` требует строку `cargo test` в `## Приёмка` и непустой `test result: ok.` с числом прошедших; fmt-only приёмка даёт `NO_TEST_COMMAND` и отклоняется. `polyforge` не пропускает `cargo fmt` в allowlist (`MCP error -32602`, проверено пробой, claim 312), поэтому fmt-only пакет не получает и `ToolAttestation`. Оба ограничения — часть закона репозитория, а не помеха, которую можно обойти. Поэтому fmt-проверка становится тестом, а приёмка — обычным `cargo test`.

Цель выбрана так, чтобы не зависеть от `stellwerk-cli`: на `882e3b5` этот бинарь не собирается (TUI-02 влит с `use stellwerk_task::cli::TuiArgs`, а `struct TuiArgs` нет ни в одном коммите), и приёмка через `--test run` была бы красной не из-за формата. `cargo test -p stellwerk-task --test authorization` на том же коммите собирается и даёт `7 passed`, поэтому приёмка меряет ровно формат.

## Обоснование (измерено)
`cargo fmt --all -- --check` на `882e3b5` печатает 16 hunks в 7 файлах. Два из них — незакоммиченная работа владельца (`gateway/tests/session_failure.rs`, `gateway/tests/shaping.rs`) и в область этого пакета не входят. Оставшиеся пять принадлежат уже влитым пакетам и не покрыты ни одним действующим `files_allowed`:

- `crates/stellwerk-cli/tests/run.rs` — R01
- `crates/stellwerk-gateway/src/gate.rs` — A01
- `crates/stellwerk-gateway/tests/gate.rs` — A01
- `crates/stellwerk-task/src/m5/gate_channel.rs` — A01
- `crates/stellwerk-task/tests/authorization.rs` — A03

Пакет 01 трогать их не вправе: его `files_allowed` — шесть других путей, и расширение до одиннадцати отклонено владельцем 2026-11-02. Поэтому закрытие приёмки 01 вынесено в отдельный пакет, а не спрятано в чужой waiver.

## Шаг 1 — падающий тест
В `crates/stellwerk-task/tests/authorization.rs` добавить `#[test] fn workspace_is_rustfmt_clean()`, который запускает `cargo fmt --all -- --check` из корня workspace и утверждает успех, печатая stdout+stderr при провале. `Path` и `PathBuf` там уже импортированы, корень считается от `env!("CARGO_MANIFEST_DIR")` вверх на два уровня; добавляется только `std::process::Command` и сам тест — логики не появляется.

RED: тест падает, и в сообщении перечислены ровно пять файлов выше. Падение обязано быть падением утверждения, а не компиляции.

Worktree: пакет исполняется в отдельном worktree от `882e3b5` (`.worktrees/p01-fmt-04`), потому что в главном дереве лежит незакоммиченная работа владельца. В worktree этих двух файлов нет, поэтому `cargo fmt --all -- --check` там называет ровно пять путей из области пакета, и результат атрибутируется однозначно. Главный worktree не трогать.

## Приёмка
cargo test -p stellwerk-task --test authorization
Ожидается `test result: ok.` и `8 passed` (7 существующих тестов + новый). Дополнительно в worktree: `cargo fmt --all -- --check` exit 0 и пустой вывод; `cargo clippy -p stellwerk-task --all-targets -- -D warnings` exit 0. Workspace-wide clippy на `882e3b5` красный по чужой причине (`cmd_add.rs:102`, `unnecessary_map_or`; чинится незакоммиченной работой владельца), поэтому в приёмку этого пакета он не входит.
Дифф по пяти путям: только переносы строк, отступы и разбиение цепочек; в `authorization.rs` единственное добавление — сам тест.

## Запрещено
- нельзя менять логику, имена, импорты по смыслу, утверждения существующих тестов или порядок аргументов; только то, что требует rustfmt, плюс новый тест в `authorization.rs`;
- нельзя трогать `crates/stellwerk-core/src/contract.rs`, lint-политику, корневой `Cargo.toml` и `docs/p00-baseline.md` (это историческая база P00: `exit 1` и `0.87.0` там остаются);
- нельзя добавлять `#[allow]`, `#[ignore]`, `rustfmt.toml` или `#[rustfmt::skip]` — подавление вместо форматирования запрещено;
- нельзя работать в главном worktree, трогать незакоммиченные файлы владельца и гонять `git checkout --` по ним.
