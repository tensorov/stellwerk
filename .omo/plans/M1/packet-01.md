# packet-01
plan_id: M1
goal: Зафиксировать RED-базлайн ядра: убедиться, что 27 RED-тестов падают именно на `not yet implemented: RED: …`, и записать сводку как evidence.
files_allowed: [crates/stellwerk-core/tests/baseline_red.md]
max_diff_lines: 200
test: cargo test -p stellwerk-core
acceptance: Вывод показывает 27 failed и 20 passed, и КАЖДЫЙ panic содержит `not yet implemented: RED:`. Если хоть один тест падает по иной причине (ошибка компиляции, неверный assert) — вернуть BLOCKED и описать расхождение.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [—]
data_class: public
lane: m1/core-red

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

Пакет **не пишет код**. Он доказывает, что оставленная работа честная: красное — по
задуманной причине, а не по случайной.

1. Выполнить `cargo test -p stellwerk-core 2>&1 | tee /tmp/m1-core-red.txt`.
2. Убедиться, что падают ровно 4 набора: `task_lifecycle` (10), `data_class_routing` (6),
   `workspace_policy` (4), `rbac_level` (7); зелёных 20.
3. Проверить, что каждый panic содержит подстроку `not yet implemented: RED:`.
4. Скопировать сводку в `crates/stellwerk-core/tests/baseline_red.md` (markdown, не тест).
5. НЕ приступать к реализации предикатов: это packet-02…06.

## Evidence

- `cargo test -p stellwerk-core` → сводка 27 failed / 20 passed с причиной `not yet implemented`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
