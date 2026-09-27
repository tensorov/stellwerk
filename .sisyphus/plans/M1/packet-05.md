# packet-05
plan_id: M1
goal: Реализовать `PolicyLevel::permits` — лестницу полномочий федерации.
files_allowed: [crates/stellwerk-core/src/contract.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-core --test rbac_level
acceptance: `cargo test -p stellwerk-core --test rbac_level` → 11 passed; 0 failed.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [01]
data_class: public
lane: m1/core-rbac

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

Лестница прав из `tests/rbac_level.rs`:
- `ReadWorkspace` — всем, начиная с `L0Local`;
- `L0Local` НЕ может `WriteWorkspace` и НЕ может `RunTask`;
- `L2Contributor`: WriteWorkspace, RunTask; НЕ UpdateModule/ManageMembers/GrantRoles;
- `L3Maintainer`: + UpdateModule; НЕ ManageMembers/GrantRoles;
- `L4Admin`: + ManageMembers; НЕ GrantRoles;
- `GrantRoles` — ТОЛЬКО `L5Owner`: иначе скомпрометированный админ тихо расширит права;
- монотонность: если право есть у уровня, оно есть у всех верхних.

1. Таблица «действие → минимальный уровень», затем `self >= required`. Таблица
   гарантирует монотонность конструктивно.
2. `GrantRoles` → `PolicyLevel::L5Owner`.
3. Удалить `let _ = (self, action);`.

## Evidence

- `cargo test -p stellwerk-core --test rbac_level` → 11 passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
