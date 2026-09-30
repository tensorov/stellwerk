# packet-09e
plan_id: M0
goal: слайс 1 лестницы ADR-0005 — ввести `EventRisk` как публичный тип ядра с рукописным `Deserialize`, принимающим и legacy-голую строку `"R3"`, и новые токены. Тип пока НЕ подключается к `Event.risk`: это отдельный слайс. Слайс не зависит от развилки A/B, потому что тип нужен в обоих вариантах.
files_allowed: [crates/stellwerk-core/src/contract.rs, crates/stellwerk-core/tests/event_risk.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-core --test event_risk
acceptance: `cargo test -p stellwerk-core --test event_risk` → 5 passed, 0 failed, 0 filtered out; на RED до реализации тест падает на отсутствии типа; `cargo fmt --all -- --check` → чисто; `cargo clippy --all-targets -- -D warnings` → чисто; полный `cargo test --workspace --no-fail-fast` → 0 failed и число тестов выросло ровно на 5.
forbidden: [cargo add, правка корневого Cargo.toml, правка Cargo.toml крейта, правка modules.lock, правка gateway/task/store/registry, переключение Event.risk на EventRisk, serde(default) внутри EventRisk, Option<RiskClass> как замена, #[ignore], #[allow], async, ослабление существующих тестов]
depends_on: [packet-09c, packet-09d]

**Помечен contract-change:** правится `crates/stellwerk-core/src/contract.rs`, который AGENTS.md
запрещает трогать без метки `contract-change` и ADR. ADR — `docs/adr/0005-event-risk-three-states.md`.

## Почему тип вводится раньше, чем подключается

ADR-0005 вернул `SPLIT_REQUIRED` с радиусом 14 файлов при лимите 3. Лестница обязана иметь
слайсы, которые зелёные независимо, иначе первый же коммит оставит дерево красным.

Развилка A/B касается того, вытесняет ли `EventRisk` существующий `ToolRisk`
(`crates/stellwerk-gateway/src/permission.rs:32-38`). **Сам тип `EventRisk` нужен в обоих
вариантах**, поэтому этот слайс от развилки не зависит и может быть выполнен до решения владельца.

## Ключевое проектное решение: миграция данных не нужна

`RiskClass` не имеет `#[serde(rename_all)]`, поэтому на проводе он лежит голой строкой `"R3"`
(проверено: `core/tests/event_serde.rs`). Если `EventRisk::Classified(c)` сериализуется тем же
способом, то `Event { risk: EventRisk::Classified(R3) }` даёт **байт-в-байт ту же строку**
`"risk":"R3"`, что и до изменения.

Следствие: строка, лежащая в `events.data` (`TEXT`, пишется на `store/src/sqlite.rs:325`),
читается новым кодом без всякой миграции базы. Это снимает вопрос, который ADR считал
необходимым, и вместо него оставляет правило совместимости на десериализаторе.

Отдельная ловушка, из-за которой нельзя полагаться на один лишь код возврата:
`store/src/sqlite.rs:361-362` и `386-387` превращают ошибку десериализации в
`rusqlite::Error::QueryReturnedNoRows`, то есть строка просто исчезает. Поэтому совместимость
обязана быть проверена на настоящем serde_json, а не «на глаз».

## Что сделать

1. **Красный тест сначала.** `tests/event_risk.rs`:
   - десериализация legacy-строки `"R3"` → `Classified(R3)`;
   - десериализация `"R2"` → `Classified(R2)`, то есть класс не зашит;
   - десериализация `"unclassified"` → `Unclassified`;
   - десериализация `"not_applicable"` → `NotApplicable`;
   - **неизвестная строка `"R9"` и отсутствие значения дают ошибку, а не R0** (закон 6);
   - round-trip `Classified(R3)` даёт ровно `"R3"`, то есть форма не изменилась;
   - round-trip `Unclassified` и `NotApplicable` дают свои токены.
2. **Тип.** `pub enum EventRisk { Classified(RiskClass), Unclassified, NotApplicable }` с
   рукописным `Deserialize`. `Serialize` не выводитьderive-ом, а написать так, чтобы
   `Classified` уходил голой строкой. `serde(default)` внутри запрещён.
3. **Не подключать** к `Event.risk` в этом слайсе. Это следующий слайс, и он трогает
   конструкторы событий.

## Чего этот слайс не доказывает

Не доказывает, что `Event { risk: EventRisk }` компилируется во всех местах, — это работа
следующего слайса. Не доказывает, что миграция базы не понадобится, если следующий слайс решит
подключить тип к `Event.risk`: сейчас доказано лишь то, что сами значения совместимы.
