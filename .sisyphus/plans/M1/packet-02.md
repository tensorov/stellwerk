# packet-02
plan_id: M1
goal: Реализовать предикаты жизненного цикла `TaskStatus::is_terminal` и `TaskStatus::can_transition_to`.
files_allowed: [crates/stellwerk-core/src/contract.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-core --test task_lifecycle
acceptance: `cargo test -p stellwerk-core --test task_lifecycle` → 11 passed; 0 failed. Результат остальных core-тестов не меняется.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [01]
data_class: public
lane: m1/core-task

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

Таблица переходов задана тестами в `tests/task_lifecycle.rs`:
- цепочка Draft→Planned→Reviewed→Delegated→Running→EvidenceSubmitted→Verified→Validated→Done;
- Suspended: Running→Suspended, Suspended→Running, Delegated→Suspended;
- из Running/Suspended/Delegated разрешены Failed и Blocked;
- Blocked→Draft и Blocked→Planned (Blocked — запрос решения, не тупик);
- Done/Failed терминальны, исходящих переходов нет;
- переход в себя запрещён;
- EvidenceSubmitted достижим ТОЛЬКО из Running/Suspended;
- запрещены прыжки: Delegated→Verified, Planned→Validated, Draft→Done;
- из Validated/Verified нельзя вернуться в Running.

1. `is_terminal`: `matches!(self, TaskStatus::Done | TaskStatus::Failed)`.
2. `can_transition_to`: `match (self, next)` по таблице. Таблица, а не цепочка `if` —
   порядок читается как спецификация.
3. Удалить `let _ = (self, next);`.
4. Больше в `contract.rs` ничего не трогать.

## Evidence

- `cargo test -p stellwerk-core --test task_lifecycle` → 11 passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
