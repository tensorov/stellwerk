# N1-10 — единый вход, честные заглушки, приёмка N1

**label:** — · **mutation_gate:** no (сборка CLI и вывод)
**depends_on:** [N1-09] · **ADR:** не требуется

## Цель

1. **п.7:** `stellwerk` — **один** бинарник. `stellwerk-registry/Cargo.toml:9`
   объявляет `[[bin]] name = "stellwerk"`, а `stellwerk-task/src/cli.rs:18`
   объявляет `#[command(name = "stellwerk")]`. `stellwerk-cli` остаётся
   отдельным крейтом (решение владельца).
2. `cancel`/`logs` (`cli.rs:131-132`) перестают быть молчаливыми заглушками.
3. Финальная сборка цепочки и приёмка N1.

**Поправка к прошлой версии пакета (найдено ревью):**
`crates/stellwerk-registry/src/lib.rs` **уже существует** и уже экспортирует
`pub mod cmd_audit;`, `cmd_measure`, `cmd_add`, `cmd_update`, `cli`. Замена
`main.rs` на `lib.rs` **не требуется** — нужен только отказ от `[[bin]]`.

## production_call

- `stellwerk_cli::main` — единственная точка входа
- `stellwerk_registry::cmd_audit::audit_module` ← через `stellwerk_cli`

## files_allowed

- `crates/stellwerk-registry/Cargo.toml`
- `crates/stellwerk-cli/src/main.rs`
- `crates/stellwerk-task/src/cli.rs`
- `crates/stellwerk-task/tests/vertical_n1.rs`
- `crates/stellwerk-cli/examples/pi-smoke.toml`

**limit_waiver:** `files: 5` — снятие `[[bin]]` и перенос подкоманд меняются
вместе с точкой входа; вертикальный тест обязателен по закону 2; `pi-smoke.toml`
создаётся здесь, раньше его не существовало и создавать его негде.

## Что делать

1. `stellwerk-registry`: убрать `[[bin]]` из `Cargo.toml`. `src/main.rs`
   удаляется, `src/lib.rs` **остаётся как есть** — он уже корректен.
2. `stellwerk-cli`: `[[bin]] name = "stellwerk"`, единственный. Подкоманда
   `module` вызывает `stellwerk_registry::cmd_*` — **логика не дублируется**.
3. `stellwerk-task`: `src/cli.rs` перестаёт объявлять `#[command(name =
   "stellwerk")]` — это имя теперь у `stellwerk-cli`. Бинарник остаётся
   `stellwerk-task`.
4. **`cancel`/`logs`:** заглушка `stub("cancel", "packet-06", …)` уже печатает
   `not implemented: cancel (packet-06)` (`cli.rs:177`). Заменить на честный
   выход: сообщение, что возможность в N2+, ненулевой код. Итоговое состояние
   — «ждёт человека», а не `Done` (решение 5).
5. Финальная сборка: `run` → вердикт → receipt → Pi-`RunOutcome` →
   evidence-цепочка → статус → терминал. **`Done`/`gate PASS` не выводить**
   (решение 5): без человеческой `Validation` вывод — «ждёт человека».

6. **Приёмка N1 идёт на fixture-агенте**, а не на настоящем Pi: запускается
   `examples/pi-fixture.sh`, печатающий корректный Pi-поток. Настоящий Pi и
   model-transport — **N2-00**, отдельный security-критичный пакет
   (`index.md`). В N1 канал к модели **не строится** — это осознанный размен.

## Почему `mutation_gate: no`

Перенос подкоманд, снятие `[[bin]]` и замена заглушек — обвязка и вывод.
Решение о допуске принимает `gateway::decide`, а не этот пакет.

## Запрещено трогать

Логику `cmd_audit`/`cmd_measure`/`cmd_add`/`cmd_update` — только видимость.
`modules.toml`, `crates/stellwerk-gateway/**`, `runner.rs`, `evidence.rs`,
`m5/**`, `justfile`, `.github/`, `scripts/plan-lint.py`, `xtask/`.

## Приёмка

```sh
cargo build --workspace --all-targets; echo "BUILD_EXIT=$?"
cargo test --workspace --no-fail-fast; echo "TEST_EXIT=$?"
```

**BUILD_EXIT=0 обязателен.** `TEST_EXIT` = 0 либо совпадает с baseline
(`index.md`), который **перепроверен на текущем снимке** (решение 7).

Разрывы, которые обязаны быть закрыты — каждый проверяется:

```sh
# п.3 — run_once вызывается из production
grep -rn "run_once" crates/stellwerk-task/src/ crates/stellwerk-cli/src/ | grep -v tests/

# §4 — битый ShellSandbox и его тестовая подмена исчезли
grep -rn "crate::sandbox" crates/stellwerk-task/src/ || echo "OK: ссылок нет"
grep -rn "STELLWERK_SANDBOX" crates/stellwerk-task/tests/ || echo "OK: фикстуры нет"

# env_clear — решение 2
grep -rn "env_clear" crates/stellwerk-task/src/ | grep -v tests/

# п.7 — ровно один [[bin]] с именем stellwerk
grep -rn 'name = "stellwerk"' crates/*/Cargo.toml

# решения 5–6 — evidence и авторизация не выдают себя за большее
grep -rn "authorize" crates/stellwerk-task/src/ | grep -v tests/
```

`grep -rn 'name = "stellwerk"' crates/*/Cargo.toml` обязан вернуть **ровно одну**
строку. Две — коллизия не снята.

**Финальный марш N1 (fixture-агент):**

```sh
cargo run -p stellwerk-cli -- run crates/stellwerk-cli/examples/pi-smoke.toml; echo "RUN_EXIT=$?"
```

`RUN_EXIT=0`, в выводе `verdict=`, `receipt=`, `gate=ждёт человека`.
Отсутствие этих строк при нулевом коде — **провал**.

`pi-smoke.toml` указывает на `examples/pi-fixture.sh` и создаётся **этим
пакетом** — добавьте его в `files_allowed` вместе с правкой `examples/`.

**Ожидаемый итог N1:** цепочка `authorize → execute → receipt → evidence →
ожидание Validation` доказана на fixture-агенте. Настоящий Pi появится только
в N2 вместе с model-transport (N2-00).