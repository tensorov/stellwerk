# packet-02
plan_id: M4
goal: RED — раннер обязан спрашивать разрешение на КАЖДЫЙ вызов инструмента, и `Unclassified` обязан давать `Deny`.
files_allowed: [crates/stellwerk-task/src/runner.rs, crates/stellwerk-task/tests/permission_wire.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --test permission_wire
acceptance: RED: тест компилируется, прогон падает на заглушке. Каждый сценарий закреплён отдельным `#[test] fn`.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [01, M2-09, M2-10, M2-11]
data_class: public
lane: m4/authorize

## Контекст

- `runner.rs` появляется в M2-09/10/11. Этот пакет **зависит от M2-05…12**:
  без `runner.rs` файл нечего создавать, а создавать его здесь нельзя — его
  создаёт M2. Раздавать этот пакет до закрытия M2 нельзя.
- `decide` реализован (M3-04, `bd3a20d`) и живёт в `stellwerk_gateway::gate`.
- `permission_wire.rs` — новый интеграционный тест. Он проверяет не результат
  `decide` (это уже сделано в `tests/gate.rs` и `tests/gate_ask.rs`), а
  **достижимость**: вызывается ли `decide` из пути раннера.

## Что сделать

1. `src/runner.rs` — объявить весь публичный интерфейс, который тестирует пакет 03:
   - `pub struct Call { pub tool: String, pub module: String, pub risk: ToolRisk, pub data: DataClass, pub model: ModelBinding, pub interactive: bool }`;
   - `pub fn authorize(call: &Call, caller: PolicyLevel) -> Verdict` — **RED-заглушка**
     (`unimplemented!()`), сигнатура как в packet-03.
   Ничего больше: ни диспетчеризации, ни песочницы, ни БД. Это пакет о ВОПРОСЕ,
   а не о запуске.
2. `tests/permission_wire.rs` — RED-тесты, по одному на сценарий:
   - `Unclassified` → `Deny`: **не** `Allow` и **не** `Skip`. Инструмент без
     классификации в `modules.toml` не проходит, и это `Deny`, а не «пропустим
     и посмотрим»;
   - `R5` при `L5Owner` → `Deny`: необратимое разрушение не делегируется агенту
     ни при каком уровне;
   - `R2` при `L2Contributor` → `Deny`: нехватка уровня отказывает раньше `Ask`;
   - `R2` при `L3Maintainer`, интерактивно → `Ask`;
   - `R0` при `L2Contributor` → `Allow`: не всё подряд запрещено, иначе тест
     доказывает только пессимизм;
   - **`authorize` не имеет пути, возвращающего `Allow` без вызова `decide`.**
     Это отдельный тест: зафиксировать, что «пропустить» невозможно в принципе,
     а не что сегодня невозможно.

## Почему последний тест главный

F-10 вырос не из «забыли вызвать», а из того, что прямой вызов был **возможен, но
никто его не сделал**. Пока существует хотя бы один путь, возвращающий `Allow`
мимо `decide`, дыра воспроизводится. Тест на отсутствие пути закрывает класс, а
не экземпляр.

## Evidence

- RED-вывод `cargo test -p stellwerk-task --test permission_wire`: компилируется
  и падает на заглушке.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
