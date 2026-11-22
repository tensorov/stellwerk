# N1 — ревью раунд 1: NOT-READY

**Дата:** 2026-10-11 · **Ревьюеры:** Metis (пробелы/двусмысленности), Momus (ясность/верифицируемость)
**Вердикт:** `NOT-READY`. Раздавать нельзя: часть пакетов неисполнима в заявленных границах.

Оба ревьюера независимо сошлись на четырёх блокерах. Ниже — только то, что подтверждено
чтением исходников, с file:line.

---

## BLOCKER 1 — N1-02 ломает компиляцию workspace, а приёмка это не видит

`TaskSpec` (`contract.rs:296`) не имеет `Default`, поэтому добавление двух обязательных
полей ломает **все** литералы `TaskSpec { .. }` вне `files_allowed` пакета:

| файл | строка |
|---|---|
| `crates/stellwerk-task/tests/lifecycle.rs` | 23 |
| `crates/stellwerk-task/tests/inbox.rs` | 19 |
| `crates/stellwerk-task/tests/events.rs` | 19 |
| `crates/stellwerk-store/tests/status_transition.rs` | 15 |
| `crates/stellwerk-store/tests/task_crud.rs` | 14 |

`files_allowed` N1-02 = 4 файла, ни один из этих не в списке. Исполнитель не сможет
починить сборку, не нарушив пакет.

**Хуже: приёмка N1-02 этого не заметит.** Она гоняет `cargo test -p stellwerk-task
--test taskfile && cargo test -p stellwerk-core` — только два таргета. Baseline-сверка
из `index.md` ищет строки `test … FAILED`; **компиляционная ошибка не порождает таких
строк**, то есть падает молча: пакет «зелёный» при разрушенном workspace.

**Нужен разбор:** либо расширить `files_allowed` (ломает ADR-0003 — 3 файла), либо
не добавлять поля в `TaskSpec`, а вынести `program`/`engine` в отдельный параметр.

---

## BLOCKER 2 — N1-04 требует правки `runner.rs`, который запрещён

`runner.rs:18` — `use crate::sandbox::Sandbox;`, и `RunRequest` (`runner.rs:46`)
несёт поле `sandbox: Sandbox` именно этого **битого** типа. Инструкция «убрать
`ShellSandbox` из `RunRequest`» = правка `runner.rs`.

`runner.rs` входит в список «существующее НЕ переписывать» (`index.md`) и **не**
входит в `files_allowed` N1-04. Удаление `sandbox.rs` ломает компиляцию `runner.rs`
целиком.

**Нужен разбор:** либо разрешить `runner.rs` в этом пакете, либо не трогать
`RunRequest`, а лишь перестать вызывать `ShellSandbox` где-либо в production.

---

## BLOCKER 3 — N1-03 структурно невыполним

`EngineAdapter::session(argv, cwd, timeout)` (`m5/mod.rs:176`) **spawn'ит сам**
через голый `Command::new(program)`. Он не принимает подготовленный `Command`,
поэтому bwrap-argv и `env_clear` через него не провести. `m5/*` трогать запрещено.

Две взаимоисключающие ветви:

- **ветвь A:** взять `Sandbox::command(...)` и spawn'ить сам →
  `EngineAdapter::session` не имеет production-вызова, `production_call` пакета ложен;
- **ветвь B:** вызвать `session` → процесс **не** в bwrap, `env_clear` невозможен,
  а grep-проверки при этом проходят, то есть изоляция останется дырой при «зелёном» пакете.

Пакет предусматривает выход через BLOCKED, но это означает остановку N1 на ключевом пакете.

---

## BLOCKER 4 — приёмочные команды ссылаются на несуществующее

| Команда | Где | Проблема |
|---|---|---|
| `run crates/stellwerk-task/tests/fixtures/vertical.toml` | N1-01 | каталога `tests/fixtures/` нет |
| `run fixtures/n1-smoke.toml` | `index.md`, N1-10 | файла нет; **ни один пакет его не создаёт** в этом пути |
| `cargo run -p stellwerk-cli -- cancel t-1` | N1-10 | `cancel` живёт в `stellwerk-task`, не в `stellwerk-cli` |
| `… -- cancel t-1 \| grep -qiE 'not implemented'` | N1-10 | **проходит вакуумно:** текущая заглушка уже печатает `not implemented: cancel (packet-06)` (`cli.rs:177`) |

---

## BLOCKER 5 — N1-10: `evaluate()` недостижим, источника `RunOutcome` нет

`run_once` возвращает `CallVerdict`, **не** `RunOutcome`. А `evaluate(outcome, chain)`
требует `RunOutcome` с `verdict: Verdict::Valid`.

`Verdict::Valid` получается только через `verdict_for(stop, tool_steps, turns)`,
а счётчики — из `Progress::absorb`, то есть из **Pi-событий в stdout агента**.
`Worker::exec` возвращает `ExecOutcome { output: String }` — потока нет, парсить нечего.

Плюс `verdict_for` требует `tool_steps ≥ GATE_REQUIRE_TOOL_STEPS` (1),
`turns > GATE_REQUIRE_TURNS` (1, т.е. **≥2**), `stop == Stop`.
`/bin/echo` не печатает ни одного события.

Итог: требование N1-10 «вертикальный тест с `gate.passed == true`» недостижимо,
и **ни один пакет не поручает** построить `RunOutcome`.

---

## Расхождения внутри плана (чинится без владельца)

1. **Таблица «Разрывы → пакеты» в `index.md` устарела** — систематический сдвиг на 1–2:
   п.10 → указан N1-04 (фактически N1-05), §5 → N1-05 (фактически N1-06),
   п.5 → N1-06 (фактически N1-08), п.7 → N1-08 (фактически N1-09).
2. **`index.md` рисует N1-04 ∥ N1-05 параллельно, но `packet-05` объявляет
   `depends_on: [N1-04]`.** Два источника истины; параллельная раздача столкнётся
   на `lib.rs`.
3. **N1-09 исходит из устаревшего чтения:** «`src/main.rs` заменить на `src/lib.rs`» —
   `crates/stellwerk-registry/src/lib.rs` **уже существует** и уже экспортирует
   `pub mod cmd_audit;` и др. Замена файла не нужна, нужна лишь правка `Cargo.toml`.
4. **`TaskFileLimits::default()` даёт `timeout_seconds: 0`, а `validate()` требует
   `> 0`** — фикстура обязана задавать `limits` явно, иначе `run` упадёт на валидации.
5. **`ToolLimits` требует ненулевых `max_files`/`max_diff_lines`** — тот же подвох.

---

## Что ревью подтвердило как здоровое

- `Sandbox::command` возвращает `Command` → `env_clear()` вызывающий наносит сам,
  `m5/*` править не нужно (это ядро N1-01/N1-03 и оно корректно);
- `SqliteStore` действительно реализует `EventStore` (`sqlite.rs:318`) — store писать не надо;
- `rusqlite` есть в `[workspace.dependencies]` и в `store`, но **не** в `task` —
  правка N1-08 (добавить `rusqlite = { workspace = true }`) корректна и достаточна;
- `EvidenceEntry` (`contract.rs:449`) имеет ровно 6 полей — конструкторов
  `tool_attestation`/`validation` достаточно, `contract.rs` трогать не нужно;
- baseline владельца подтверждён: оба имени тестов существуют, `modules.toml:17`
  содержит `risk = "R3"`, `cli.rs:131-132` — заглушки.

---

## Что нужно от владельца

Четыре развилки — они меняют форму пакета, а не текст. Без ответов второй круг
ревью бессмысленен.

| # | Развилка |
|---|---|
| 1 | N1-02: расширять `files_allowed` под 5 файлов тестовых хелперов, или не расширять `TaskSpec` полями? |
| 2 | N1-03: отказаться от `EngineAdapter` в N1 (spawn `Sandbox::command` напрямую), или ввести contract-change, научив `EngineAdapter` принимать готовый `Command`? |
| 3 | N1-04: разрешить `runner.rs` в `files_allowed`, или оставить `RunRequest` и лишь перестать вызывать битый путь? |
| 4 | N1-10: где N1 берёт `RunOutcome`? — добавить парсинг Pi-потока в N1, или признать `gate.passed=true` недостижимым до N2 и проверять в N1 только цепочку записей? |