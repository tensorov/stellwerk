# packet-04
plan_id: M1
goal: Реализовать `DataClass::permits_model` — правило «Personal никогда не уходит провайдеру с чужим retention».
files_allowed: [crates/stellwerk-core/src/contract.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-core --test data_class_routing
acceptance: `cargo test -p stellwerk-core --test data_class_routing` → 7 passed; 0 failed.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [01, 02, 03]
parallelism: sequential-only (single file lock — contract.rs)
data_class: public
lane: m1/core-data

## Контекст

- Крейт `stellwerk-core`, единственный файл контракта — `src/contract.rs`.
- Контракт заморожен: правится ТОЛЬКО предикат, названный в этом пакете. Любая другая
  правка `contract.rs` — нарушение packet'а.
- Предикаты объявлены с телом `async { todo!(...) }` и строкой `let _ = (…)`, чтобы
  `clippy -D warnings` оставался зелёным. При реализации `let _ = …` УДАЛЯЕТСЯ.
- RED-тесты написаны ночной сессией (фаза 3) в `crates/stellwerk-core/tests/`.
  Их нельзя править: они приказ, а не предложение.
- Правило проекта: `todo!()` лучше неверного ответа — пока предикат не реализован,
  вызов падает loudly.

## Что сделать

Два правила покрывают всю матрицу 3×2×3 (проверяет тест `routing_matrix`):
1. **Потолок привязки:** `self <= binding.data_class` — binding, объявленный как Public,
   не может обработать Internal.
2. **Retention:** при `RetentionClass::ThirdParty` допустим только `DataClass::Public`.
   Internal и Personal — нет.

Формула: `self <= binding.data_class && (binding.retention == FirstParty || self == Public)`.
Это и есть запрет «Personal → Big Pickle» из `.omo/omo.jsonc`.

1. Реализовать по формуле. `DataClass` уже `Ord`: Public < Internal < Personal.
2. Удалить `let _ = (self, binding);`.
3. `ModelBinding` и `RetentionClass` не трогать.

## Evidence

- `cargo test -p stellwerk-core --test data_class_routing` → 7 passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
