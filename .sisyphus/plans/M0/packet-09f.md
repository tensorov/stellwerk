# packet-09f — ПРЕДЛОЖЕН, не исполнять без подписи владельца

plan_id: M0
status: предложен
goal: слайс 2 лестницы ADR-0005 — переключить `Event.risk` на `EventRisk` и перестать выдавать R1 за неизвестный риск.
files_allowed: [crates/stellwerk-core/src/contract.rs, crates/stellwerk-core/tests/event_serde.rs, crates/stellwerk-gateway/src/receipt.rs, crates/stellwerk-gateway/tests/receipt.rs, crates/stellwerk-store/tests/concurrent.rs, crates/stellwerk-store/tests/event_append.rs, crates/stellwerk-task/src/events.rs, crates/stellwerk-task/src/runner.rs, crates/stellwerk-task/tests/cli_watch.rs, crates/stellwerk-task/tests/events.rs, crates/stellwerk-task/tests/vertical.rs]
limit_waiver: ТРЕБУЕТ ПОДПИСИ ВЛАДЕЛЬЦА. Заявлено 11 файлов при лимите 3, и это число **измерено, а не оценено**: проба в отдельном worktree меняла тип поля и чинила волну ошибок до нуля компиляции, после чего `git diff --name-only` дал ровно эти 11 файлов. Обоснование: смена типа поля атомарна — все конструкторы `Event { risk: … }` перестают компилироваться разом, промежуточного компилируемого состояния нет, поэтому разбить на зелёные слайсы невозможно в принципе, а не по нехватке времени. Без waiver пакет остаётся `SPLIT_REQUIRED`, и я не могу его выполнить.
max_diff_lines: 200
test: cargo test -p stellwerk-core -p stellwerk-gateway -p stellwerk-store -p stellwerk-task
forbidden: [cargo add, правка корневого Cargo.toml, правка modules.lock, правка contract.rs кроме поля risk, правка policy.rs/gate.rs/mount.rs/session.rs, ToolRisk, Receipt.risk, Module.risk, serde(default), #[ignore], #[allow], ослабление существующих тестов, обёртывание column_risk в Classified]

**Помечен contract-change.** Правится `crates/stellwerk-core/src/contract.rs` — файл из
запрещённого списка `AGENTS.md`. ADR: `docs/adr/0005-event-risk-three-states.md`, статус
«предложен».

## Почему пакет ждёт подписи, а не исполняется

`ADR-0005` имеет статус «предложен». Вариант A (`EventRisk` только для `Event.risk`, `ToolRisk`
не трогаем) выбран мной по дисциплине скоупа: закон 4 запрещает расширять скоуп, а владелец не
просил переписывать политику разрешений. Но выбор варианта не даёт права превысить лимит файлов
на публичном контракте. Это два разных решения, и второе — владельца.

## Три места, где правка НЕ механическая

Замер: при смене типа поля компилятор требует обернуть конструктор в `EventRisk::Classified(...)`.
Для девяти из десяти точек это верно. Для трёх — нет, и они важнее остальных.

1. **`task/src/runner.rs:516` — `column_risk`, и это главный сайт пакета.** Сегодня функция
   возвращает `RiskClass::R1` для `ToolRisk::Unclassified`. Механическая обёртка даст
   `EventRisk::Classified(R1)` — то есть ровно ту подмену закона 6, ради устранения которой
   ADR-0005 написан. Здесь обязательно `EventRisk::Unclassified`. Функция меняет возвращаемый
   тип на `EventRisk`, и её `ToolRisk::Unclassified => RiskClass::R1` заменяется на
   `ToolRisk::Unclassified => EventRisk::Unclassified`.
2. **`gateway/src/receipt.rs:89` — `risk: self.risk`.** `Receipt.risk` остаётся `RiskClass`, так
   как владелец запретил `NotApplicable` для receipt. Значит здесь
   `EventRisk::Classified(self.risk)` — и это совместимо, но по причине, которую надо назвать в
   коде, а не угадать.
3. **`gateway/tests/session_contract.rs` — правки не требует, и это проверено по составу, а не
   предположено.** Два места выглядят походами на `Event.risk`, и оба — ловушки:
   - `:95` — `risk: RiskClass::R0` внутри литерала **`Receipt`**, а не `Event`. `Receipt.risk`
     остаётся `RiskClass`, потому что владелец запретил `NotApplicable` для receipt.
     Преобразование живёт в `receipt.rs:89` (`EventRisk::Classified(self.risk)`) — в другом
     файле, который и так правке подлежит.
   - `:167` — `assert_eq!(back.risk, ToolRisk::Unclassified)`, то есть `ApprovalRequest.risk`,
     а не `Event.risk`; под вариантом A его тип не меняется.

   По той же причине не трогаются `store/tests/module_store.rs:43,140`: `loaded.risk` там —
   `ModuleRecord.risk: Option<RiskClass>`, риск объявления модуля, а не события.

   Записано явно, потому что «правки не требует» без указания причины есть неподкреплённое
   отрицание: следующий проход по правке одного из этих файлов снесёт их по незнанию.

## Что здесь НЕ трогаем, хотя слово «risk» встречается

- `ToolRisk` в `permission.rs`, `gate.rs`, `mount.rs`, `session.rs`, `runner.rs:267` — отдельный
  словарь, под вариантом A не меняется.
- `Module.risk` и `ModuleRecord.risk` (`registry/src/model.rs:47`, `store/src/traits.rs:62`) —
  риск объявления модуля, не риск события. Их `unwrap_or("R3")` и отсутствие
  `#[serde(default)]` — отдельные находки F-72, здесь не трогаем.
- `PermissionError::TableIncomplete { risk: RiskClass }` — поле ошибки, не событие.

## Приёмка

- `cargo test --workspace --no-fail-fast` → 0 failed; число тестов **не уменьшается** ни на один.
- `cargo test -p stellwerk-core --test event_risk` → 5 passed, 0 failed, **0 filtered out**.
- `cargo fmt --all -- --check` → чисто; `cargo clippy --all-targets -- -D warnings` → чисто.
- **Обязательный новый тест:** `runner.rs:516` обязан вернуть `EventRisk::Unclassified` для
  `ToolRisk::Unclassified`, и тест обязан утверждать именно это. Без этого теста пакет не
  выполняется: без него дефект F-72 вернётся, замаскированный компиляцией.
- **Обязательная сверка с базой:** прочитать сохранённое событие из `events.data` и убедиться,
  что `risk: "R3"` читается как `Classified(R3)`. Проверка на настоящем serde_json, а не на
  `from_str` в тесте: `store/src/sqlite.rs:361-362` и `386-387` превращают ошибку
  десериализации в «нет строк», то есть битая совместимость выглядит как пустая выборка.

## Связанные находки

F-72 (схлопывания риска), F-74 (`cargo test` с фильтром даёт exit=0, ничем не проверив),
F-75 (радиус нельзя узнать одним `cargo check` — список обрезан цепочкой зависимостей).
