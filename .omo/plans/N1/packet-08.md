# N1-08 — EvidenceLedger поверх SQLite

**label:** — · **mutation_gate:** yes (≥75%)
**depends_on:** [N1-07] · **ADR:** не требуется

## Цель

Минимальная реализация `EvidenceLedger` (`evidence.rs:48`). Разрыв п.5: трейт
объявлен, но **реализаций нет нигде** — ни в `src/`, ни в тестах. Граница
записи, обещанная докстрингом, не существует.

## production_call

- `EvidenceLedger::append` ← `orchestrator::execute`
- `EvidenceLedger::entries` ← `orchestrator::execute` (перед выводом статуса)

## files_allowed

- `crates/stellwerk-task/Cargo.toml`
- `crates/stellwerk-task/src/ledger_sqlite.rs`
- `crates/stellwerk-task/src/lib.rs`
- `crates/stellwerk-task/tests/ledger_sqlite.rs`

**limit_waiver:** `files: 4` — зависимость и её использование неразделимы:
без строки в `Cargo.toml` модуль не собирается.

## Что делать

0. **Сначала зависимость.** Проверено: `rusqlite` есть в `[workspace.dependencies]`
   и подключён в `stellwerk-store`, но **в `stellwerk-task` её нет**. Трейт
   объявлен в `task/evidence.rs:48`, а зависимость односторонняя (`task → store`),
   поэтому реализовать трейт в `store` без цикла невозможно. Добавить в
   `crates/stellwerk-task/Cargo.toml`:
   `rusqlite = { workspace = true }`.
   Версию из workspace — не изобретать.

1. `SqliteEvidenceLedger` реализует `EvidenceLedger`. Ориентир — уже
   работающий `impl EventStore for SqliteStore` (`sqlite.rs:318`).

2. `append`:
   - запись сериализуется целиком; `EvidenceEntry` выводит `Serialize`
     (`contract.rs:447`);
   - отказ **возвращается** через `LedgerError::Unavailable` (`evidence.rs:39`),
     а не теряется: запись, о которой никто не знает, и отвергнутая выглядят
     одинаково.

3. `entries(task_id)` — записи **в порядке записи**. Порядок обязателен:
   `chain_tail` не зависит от порядка и потому не защищает от перестановки
   доказательств (докстринг `evidence.rs:53`).

4. Таблица — **отдельная** от `tasks/events/modules` (`sqlite.rs`), чтобы отказ
   ledger не ломал event-лог. Описывать её здесь, а не правкой `sqlite.rs`.

## Почему `mutation_gate: yes`

Хранилище доказательств — то, что делает «проверенную запись» проверяемой.
Мутанты, теряющие порядок или роняющие отказ, обязаны быть пойманы.

## Запрещено трогать

`crates/stellwerk-task/src/evidence.rs` (трейт и `evaluate` — как есть),
`crates/stellwerk-store/src/sqlite.rs` (существующие таблицы не трогать),
`crates/stellwerk-gateway/**`, `runner.rs`, `justfile`, `.github/`,
`scripts/plan-lint.py`, `xtask/`.

## Приёмка

```sh
cargo build --workspace --all-targets; echo "BUILD_EXIT=$?"
cargo test -p stellwerk-task --test ledger_sqlite; echo "TEST_EXIT=$?"
```

**Оба 0.** Обязательные тесты:

- две записи одного `task_id` возвращаются **в порядке добавления**;
- `entries` для чужого `task_id` — пусто, а не «все записи»;
- отказ хранилища **возвращается** через `LedgerError`, а не теряется;
- запись без `Validation` читается как «ждёт человека» (решение 5).

Проверка, что трейт перестал быть пустым:

```sh
grep -rn "impl EvidenceLedger" crates/*/src/
```

Ровно одна строка.