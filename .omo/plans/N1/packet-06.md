# N1-06 — risk из реестра Stellwerk и сетевая политика по факту

**label:** — · **mutation_gate:** yes (≥75%)
**depends_on:** [N1-05] · **ADR:** `docs/adr/0017-risk-classification-authority.md`

## Цель

Два разрыва одним пакетом, потому что оба решаются **до** запуска и оба —
про вход.

1. **§5:** `modules.toml:17` содержит `risk = "R3"`, читается в
   `registry/src/model.rs:47` и **именно это значение** `permission_for`
   (`gateway/permission.rs:82`) принимает за истину. Репозиторий сам себя
   оценивает: кто добавил PR, тот и выдал себе разрешение.
2. **п.10:** `NetworkPolicy::allows` (`contract.rs:333`) не вызывается в `src/`.

## production_call

- `gateway::permission::RISK_PERMISSION` ← `orchestrator::risk_for`
- `core::NetworkPolicy::allows` ← `orchestrator::network_gate`

## files_allowed

- `docs/adr/0017-risk-classification-authority.md`
- `crates/stellwerk-task/src/orchestrator.rs`
- `crates/stellwerk-task/tests/risk_and_network.rs`

## Что делать

1. **ADR-0017.** Источник классификации — реестр в коде, не поле в конфиге
   проекта.

2. **`risk_for(tool, module)`** строит `ToolRisk`:
   - класс берётся **из реестра Stellwerk**, а не из `modules.toml`;
   - инструмент вне реестра → `ToolRisk::Unclassified`, а не `R0` —
     иначе дыра неотличима от чтения (`permission.rs:31`, закон 6);
   - `modules.toml:17` после пакета **не влияет** на решение — отдельный тест.

3. **Ключевой факт (найдено ревью):** `RISK_PERMISSION` (`permission.rs:63`)
   индексирован по `RiskClass`, **а не по именам инструментов**. Отдельной
   таблицы «инструмент → класс» нет. Поэтому пакет вводит **явный реестр
   инструментов** в коде и проверяет его полноту против `RISK_PERMISSION`:
   класс, отсутствующий в таблице → `PermissionError::TableIncomplete`
   (`permission.rs:52`, уже существует — не заводить свой отказ).
   **Копию `RISK_PERMISSION` не дублировать.**

4. **`network_gate`** проверяет `NetworkPolicy` из рабочего места **до** запуска.
   - `Deny` запрещает любой URL, **включая пустую строку** (инвариант
     `contract.rs:332` — не ослаблять);
   - `AllowList` сравнивает по хосту между `://` и первым из `/?#` —
     разбор уже есть в `allows`, вторая копия запрещена;
   - проверка **до** spawn. Post-flight — это `DETECTED_AFTERWARDS`, не запрет.

## Почему `mutation_gate: yes`

Мутанты, превращающие `Unclassified` в `R0`, подставляющие `risk` из конфига
или обходящие проверку пустой строки, обязаны быть пойманы.

## Запрещено трогать

`crates/stellwerk-gateway/**` — включая `RISK_PERMISSION`, она **читается**.
`crates/stellwerk-core/src/contract.rs` — `allows` уже реализован и верен.
`modules.toml` — самодекларация просто перестаёт использоваться, файл не чинится.
`m5/**`, `evidence.rs`, `runner.rs`, `justfile`, `.github/`,
`scripts/plan-lint.py`, `xtask/`.

## Приёмка

```sh
cargo build --workspace --all-targets; echo "BUILD_EXIT=$?"
cargo test -p stellwerk-task --test risk_and_network; echo "TEST_EXIT=$?"
```

**Оба 0.** Обязательные тесты:

- инструмент с `risk = "R0"` в `modules.toml`, отсутствующий в реестре →
  `Unclassified`, **не** `R0`;
- известный инструмент → класс из реестра, подтверждённый `RISK_PERMISSION`;
- класс вне `RISK_PERMISSION` → `TableIncomplete`;
- `Deny` отклоняет **пустую строку** URL.

Проверки:

```sh
grep -rn "modules.toml\|module.risk" crates/stellwerk-task/src/ | grep -v tests/   # пусто
grep -rn "allows(" crates/stellwerk-task/src/ | grep -v tests/                      # есть
```