# packet-03a
plan_id: M3
goal: RED: объявить публичную поверхность решения по вызову и закрепить ось риска R0-R5 и ось данных.
files_allowed: [crates/stellwerk-gateway/src/gate.rs, crates/stellwerk-gateway/src/lib.rs, crates/stellwerk-gateway/tests/gate.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test gate
acceptance: RED: крейт собирается, тест компилируется, прогон падает на заглушке `decide`. Все 10 сценариев ниже закреплены отдельным `#[test] fn`. Реализации `decide` нет.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [02]
data_class: public
lane: m3/gate-red

## Контекст

- **Новый крейт `stellwerk-gateway`** — виртуальный MCP-сервер: он НЕ выполняет
  инструменты сам, а решает, что можно, спрашивает, когда надо, и пишет чек.
- Крейт зависит от `stellwerk-core` и `stellwerk-store` по path. Новых крейтов нет;
  `cargo add` запрещён.
- Решения принимаются ТОЛЬКО через предикаты контракта:
  `RiskClass::required_level`, `PolicyLevel::permits`, `DataClass::permits_model`
  (все реализованы в M1, packet-05/06/04). Своих копий правил не заводить.
- Ключи и секреты не читать и не логировать: `auth.json`, `models.json`, `env | grep KEY`
  запрещены (`AGENTS.md`).
- **Пакет разделён на 03a и 03b по границе файлов, а не по логике.** Оси R0-R5 и
  данных (сценарии 1-10) помещены сюда; ось В2/В4 (`Unclassified`, интерактивность,
  `Suspended`) — в packet-03b. Причина чистая: 17 сценариев не помещались в
  `max_diff_lines: 200`, и покрытие сокращать нельзя. Логика `decide` целиком в
  packet-04.

## Что сделать

1. `src/gate.rs` — вся публичная поверхность:
   - `pub struct CallRequest { tool: String, module: String, risk: ToolRisk,
     caller: PolicyLevel, data: DataClass, model: ModelBinding,
     interactive: bool }`.
     **`risk: ToolRisk`, а не `RiskClass`.** Иначе `Unclassified` структурно не
     может попасть в `decide`, и В2 («Unclassified → Ask только в интерактивной
     L5-сессии») окажется непроверяемым не потому, что про него забыли, а
     потому, что для него нет места в типе.
     **`interactive: bool`** объявляется здесь, а не в packet-04: тест на это
     поле пишется в этом пакете, а тест, ссылающийся на несуществующее поле, не
     скомпилируется — а acceptance этого пакета требует, чтобы компилировался.
     Значение по умолчанию — `false`: неинтерактивный прогон безопаснее, и
     выдавать `Ask` по умолчанию значило бы превратить «не спросили» в «спросили,
     возможно разрешили»;
   - `pub enum Verdict { Allow, Ask, Suspended, Deny { reason: String } }`.
     `Suspended` объявляется здесь, а не в packet-04: тесты на В4 пишутся в
     packet-03b, а тест, ссылающийся на ещё не существующий вариант, не
     компилируется. Разделение пакетов поэтому такое: **RED-пакеты объявляют ВСЮ
     публичную поверхность, GREEN наполняет только логикой.** Иначе тесты
     систематически оказываются впереди своего типа;
   - `pub fn decide(req: &CallRequest) -> Verdict` — **RED-заглушка**.
2. `src/lib.rs` — добавить `pub mod gate;`. **Ровно одна строка.** Без неё
   `tests/gate.rs` не может обратиться к `stellwerk_gateway::gate`, и критерий
   «тест компилируется» недостижим: модуль, не экспортированный из `lib.rs`,
   публичным не является. Это регистрация модуля, а не расширение скоупа.
3. Правила решения (три независимых, порядок важен) — **в этом пакете только
   фиксируются тестом, реализуются в packet-04**:
   1. **Риск инструмента** (`permission_for`): `Deny` → `Deny` сразу, `Ask` → кандидат на `Ask`;
   2. **Полномочия** (`PolicyLevel::permits` на действие `Action::RunTask` + сравнение
      с `RiskClass::required_level`): у вызывающего нет нужного уровня → `Deny`;
   3. **Данные** (`DataClass::permits_model`): класс данных выше, чем binding держит →
      `Deny` (это и есть «Personal не уходит в Big Pickle»).
   Итог: `Deny` по любому пункту; иначе `Ask`, если риск R2/R3/R4; иначе `Allow`.
   R5 — `Deny` всегда, он не попадает в ветку `Ask` (ADR-0002, В1).
4. Тесты в `tests/gate.rs` — 10 сценариев, каждый отдельным `#[test] fn`:
   - R0 + L2 + Public + first-party binding → `Allow`;
   - R1 + L0 → `Deny` (нет прав писать);
   - R2 + L2 → **`Deny`**: R2 требует `L3Maintainer`, у L2 нет прав (правило 2
     срабатывает раньше `Ask` — этот случай раньше был записан как «R2 + L2 → Ask»,
     что противоречило собственному правилу пакета);
   - R2 + L3 → `Ask` (спрашиваем);
   - R3 + L4 → **`Deny`**: после ADR-0002 R3 требует `L5Owner`, L4 недостаточен;
   - R3 + L5 → `Ask` (человек решает доступ к секретам);
   - R4 + L4 → `Deny`; R4 + L5 → **`Ask` на одно действие** (В1), не `Deny`.
     Это два отдельных теста: `Deny` при нехватке уровня и `Ask` при `L5Owner`
     обязаны различаться, иначе тест не различит «не хватило прав» и «спросили»;
   - R5 → `Deny` при любом уровне, включая `L5Owner` (необратимо — не делегируется);
   - Personal + third-party binding → `Deny`, даже при R0 и `L5Owner`;
   - Public + third-party → не `Deny` по данным.
5. `Verdict::Deny` обязан нести причину: без неё агент не поймёт, что делать.
   Проверяется здесь на сценариях выше; в packet-03b добавляется проверка, что
   причина непустая и что `Suspended` отличается от `Deny`.

## Evidence

- RED-вывод `cargo test -p stellwerk-gateway --test gate`: тесты скомпилировались и
  упали на заглушке, а не отказались собираться.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
