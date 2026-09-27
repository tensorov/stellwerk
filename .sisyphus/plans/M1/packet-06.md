# packet-06
plan_id: M1
goal: Добавить тест и реализацию `RiskClass::required_level` — единственный предикат контракта, оставшийся без теста.
files_allowed: [crates/stellwerk-core/src/contract.rs, crates/stellwerk-core/tests/risk_level_policy.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-core --test risk_level_policy
acceptance: Новый тест сначала падает на `not yet implemented: RED: policy::required_level` (зафиксировать вывод), затем после реализации проходит. Итог по крейту: 21 passed, 27 failed.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [01]
data_class: public
lane: m1/core-risk

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

**Особенность пакета:** `required_level` — единственный предикат, под который
ночная сессия не написала тест. Это осознанный долг, и он закрывается здесь.

## Что сделать

Лестница, согласованная с `rbac_level`:
- R0 → L0Local (только чтение);
- R1 → L2Contributor (запись);
- R2 → L3Maintainer (сеть);
- R3 → L4Admin (секреты);
- R4 → L5Owner (внешний наблюдаемый эффект);
- R5 → L5Owner (необратимо).

1. RED: создать `tests/risk_level_policy.rs` — таблица выше, проверка каждой пары плюс
   монотонность на соседних уровнях (R0 < R1, R4 == R5). Убедиться, что тест ПАДАЕТ,
   зафиксировать вывод.
2. GREEN: `match self` с возвратом `PolicyLevel`.
3. Удалить `let _ = self;`.
4. `PolicyLevel::permits` не трогать — он в packet-05.

## Evidence

- Сначала RED-вывод нового теста, затем `cargo test -p stellwerk-core --test risk_level_policy` → passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
