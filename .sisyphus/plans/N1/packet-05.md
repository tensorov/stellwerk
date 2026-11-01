# N1-05 — RunOutcome: прогон превращается в измеримый факт

**label:** — · **mutation_gate:** yes (≥75%)
**depends_on:** [N1-04, 03] · **ADR:** не требуется

## Цель

Разрыв: `run_once` возвращает `CallVerdict`, а `evaluate()` требует
`RunOutcome` с `verdict: Verdict::Valid`. `Verdict::Valid` считается **только**
из Pi-событий в stdout агента: `tool_steps ≥ 1`, `turns ≥ 2`, `stop == Stop`
(пороги `GATE_REQUIRE_TOOL_STEPS`, `GATE_REQUIRE_TURNS`).

`/bin/echo` не печатает ни одного события, поэтому годность прогона из N1-01
неизмерима — блокер 5 из `REVIEW.md`.

**Решение 4:** `Progress`/`RunOutcome` входят в N1. `/bin/echo` — smoke
процесса, **не proof агентного успеха**. Нужны fixture-stream тесты и
настоящий Pi smoke через единый CLI.

## production_call

- `runner::Progress::absorb` ← `orchestrator::SandboxWorker::exec`
- `runner::verdict_for` ← `orchestrator::execute`

## files_allowed

- `crates/stellwerk-task/src/orchestrator.rs`
- `crates/stellwerk-task/tests/pi_stream.rs`
- `crates/stellwerk-cli/examples/pi-fixture.sh`

**limit_waiver:** `files: 3` — в лимите ADR-0003, waiver не нужен.

## Что делать

1. `SandboxWorker::exec` получает stdout запуска и **прогоняет строки через
   `Progress::absorb`**, а не возвращает `ExecOutcome { output }` вслепую.
   `Worker::exec` не переписывается — разбор живёт в оркестраторе.

2. Из счётчиков `Progress` и `StopFacts` строится `RunOutcome`, из него —
   `verdict_for(stop_reason, tool_steps, turns)`.

3. **Fixture-агент** `examples/pi-fixture.sh`: печатает в stdout корректную
   последовательность Pi-событий — `tool_execution_start`, **два** `message_end`
   (порог turns ≥ 2), `turn_end` со `stopReason: "stop"`.
   Это тестовый вход, а не имитация успеха: он позволяет **доказать**, что
   подсчёт работает, и не более того.

4. **Обязательные тесты в `pi_stream.rs`:**
   - поток из fixture → `tool_steps = 1`, `turns = 2`, `verdict == Valid`;
   - поток без `tool_execution_start` → `verdict != Valid`;
   - `turns = 1` (ниже порога) → `verdict != Valid`;
   - `/bin/echo` (пустой поток) → **`verdict != Valid`**, и это **корректное**
     поведение, а не сбой.

5. Пороги читаются из существующих констант раннера. **Своих порогов не
   вводить** и не «подгонять» под fixture: тест обязан падать, если агент
   не сделал работу.

## Почему `mutation_gate: yes`

Годность прогона — то, на чём держится приёмка. Мутанты, превращающие
`turns < 2` в «достаточно» или `tool_steps = 0` в валидные, обязаны быть
пойманы.

## Запрещено трогать

`crates/stellwerk-task/src/runner.rs` — **`Progress` и `verdict_for` уже
реализованы, не переписывать** (решение 3: разбор потока без нужды не трогать).
`evidence.rs`, `crates/stellwerk-gateway/**`, `m5/**`, `justfile`, `.github/`,
`scripts/plan-lint.py`, `xtask/`.

## Приёмка

```sh
cargo build --workspace --all-targets; echo "BUILD_EXIT=$?"
cargo test -p stellwerk-task --test pi_stream; echo "TEST_EXIT=$?"
```

**Оба 0.** Проверки:

```sh
grep -rn "Progress::absorb" crates/stellwerk-task/src/ | grep -v tests/   # есть
grep -rn "verdict_for" crates/stellwerk-task/src/ | grep -v tests/       # есть
```

**Критично:** тест с `/bin/echo` обязан давать `verdict != Valid`. Если он
зелёный — пакет не выполнен: это значит, что годность выводится из чего-то,
кроме измеренного потока.