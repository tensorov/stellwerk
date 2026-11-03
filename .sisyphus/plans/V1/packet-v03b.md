---
id: v03b
milestone: V1
status: GREEN
depends_on: [o01, v02, o03, o04]
files_allowed: [crates/stellwerk-task/src/evidence.rs, crates/stellwerk-task/tests/evidence.rs, crates/stellwerk-task/tests/vertical.rs, crates/stellwerk-cli/tests/vertical_v3.rs]
limit_waiver: C2 — 4 files; src and tests are split because the binding check lives in evidence.rs and the existing vertical gate fixture must carry the same bindings
overlap_waiver: C9 — N1/07 — `evidence.rs`/`tests/evidence.rs`, последовательное исполнение в одном worktree
acceptance: cargo test -p stellwerk-task --test evidence && cargo test -p stellwerk-task --test vertical && cargo test -p stellwerk-cli --test vertical_v3
---

# v03b — evidence bindings must be visible to the chain check

## Цель
`recipe_digest`, `policy_digest` и `commit_sha` входят в проверяемый хвост цепочки и
обязательны для `ToolAttestation`; подмена любого из них меняет хвост, а отсутствие —
названный отказ, а не молчаливое «привязка не нужна».

## Обоснование (измерено)
`crates/stellwerk-task/src/evidence.rs` `chain_tail` хеширует только `id`, `task_id`,
`kind`, `command`, `result_hash`; все binding-поля игнорируются. `V1/packet-v03.md`
разрешает править только `tests/vertical_v3.rs`, поэтому проверка подмены не может
позеленеть без production-изменения. `o01`/`v02` уже смёржены, схема PolyForge
принимает `ModelClaim` (entry 309), так что блокер — только область пакета.

## Шаг 1 — падающий тест
1. В `tests/evidence.rs` добавить `tool_attestation_with_bindings` (или помощник,
   заполняющий три поля) и тест: хвост полной цепочки с bindings отличается от хвоста
   с подменённым `recipe_digest`, `policy_digest` и `commit_sha`; `ToolAttestation`
   без любого binding не проходит `evaluate` и называет отсутствующее поле.
2. В `tests/vertical_v3.rs` собрать цепочку `ModelClaim → ToolAttestation → Validation`
   с ненулевыми bindings на аттестации, посчитать хвост, подменить каждое поле по
   очереди и потребовать `assert_ne!`; отдельно потребовать отказ `evaluate` при
   отсутствии binding.
3. До правки `evidence.rs` тест обязан упасть на равенстве хвостов или на отсутствии
   причины отказа — это RED по делу, а не ошибка компиляции.

## Шаг 2 — минимальная реализация
- `chain_tail` включает `recipe_digest`, `policy_digest`, `model_fingerprint` и
  `commit_sha` с маркером наличия и длиной значения; `None` остаётся явным
  отсутствием и не превращается в `Some("")`.
- `evaluate` для `ToolAttestation` добавляет отдельную причину на каждый отсутствующий
  `recipe_digest`/`policy_digest`/`commit_sha`; для остальных видов `None` остаётся
  допустимым явным «неприменимо» (закон 6).
- Обновить существующие фикстуры `tests/evidence.rs` и `tests/vertical.rs`, чтобы
  `ToolAttestation` в закрывающей цепочке несла все три bindings; не ослаблять
  существующие проверки порядка и digest.

## Приёмка
cargo test -p stellwerk-task --test evidence && cargo test -p stellwerk-task --test vertical && cargo test -p stellwerk-cli --test vertical_v3

## Запрещено
- Не принимать цепочку с `ToolAttestation` без `recipe_digest`, `policy_digest` или
  `commit_sha`; отсутствие обязано быть названным отказом.
- Не подменять проверку локальной функцией только в тесте и не объявлять привязку
  доказанной без реального `evaluate`/хвоста.
- Не трогать файлы вне `files_allowed`; не добавлять зависимости; не использовать
  `#[allow]`/`#[ignore]`.
