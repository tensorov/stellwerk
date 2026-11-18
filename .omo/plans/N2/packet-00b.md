# N2-00b — авторизация до эффекта: петля по запрошенным вызовам

**label:** — · **mutation_gate:** yes (≥75%)
**depends_on:** [N1-09] · **ADR:** не требуется
overlap_waiver: C9 — N1/packet-04 и N2/00b делят crates/stellwerk-task/src/runner.rs, последовательное исполнение

## Цель

Одиночный гейт **уже существует и доказан**: `run_once` (`runner.rs:394`)
вызывает `authorize` (`runner.rs:296`) **до** `Worker::exec`, а `authorize`
fail-closed отказывает при ошибке реестра (`registry_refused`). Тесты
`denied_never_starts`, `gate_before_spawn`, `registry_refused` уже фиксируют
«вызов запрошен — эффекта нет» для **одного** вызова.

Пакет добавляет **петлю**: последовательность запрошенных вызовов гейтится
**по одному**, и **первый не-`Allow` останавливает проход**. Инвариант:

> Решение по вызову выносится **до** его эффекта. Первый вызов, получивший
> не-`Allow` (`Ask` / `Suspended` / `Deny`, в т.ч. из-за оборванного канала
> реестра), эффекта **не производит** и завершает проход: хвост пачки не
> запрашивается вовсе — он не исполнен, не одобрен и не появляется в потоке.

Пакет не расширяет правила: вердикт по-прежнему считает `gateway::decide`
(единственный источник правил), риск — доверенный реестр, не `call.risk`.

## production_call

- `runner::run_gated` ← вызывается на пути запуска; отдельная проводка в
  реальный Pi-Build — N2-00a (здесь доказательство на `FixtureEngine`).

## files_allowed

- `crates/stellwerk-task/src/runner.rs`
- `crates/stellwerk-task/tests/gated_loop.rs`

**В лимите ADR-0003 (2 файла).**

## Что делать

1. `GatedPass` и `run_gated(attempts, worker, store) -> Result<GatedPass, RunError>`:
   пройти запрошенные вызовы по порядку через существующий `run_once`. Первый
   не-`Allow` записывается в чек и завершает проход (`Blocked` — обработанный
   префикс, последний элемент — остановивший вердикт); иначе — `Completed`.
   «Дошёл до конца» и «остановлен» — разные состояния (закон 6), поэтому это
   вариант, а не длина вектора.
2. Эффект (`worker.exec`) по-прежнему зовётся **только** внутри `run_once` и
   **только** на `Allow`. Хвост пачки не передаётся в `run_once` вовсе.
3. Негативные тесты на `FixtureEngine` (главный артефакт):
   - `[Allow, R2(интерактив), R5, Allow]` → `Blocked([Allow, Ask])`, `spawned == 1`,
     `a2` статус `Asked`, по `a3`/`a4` чека нет (хвост не запрашивался);
   - `[Allow, R5, Allow]` → `Blocked([Allow, Deny])`, `spawned == 1`, `b2` статус
     `Denied` с причиной `risk_denied`, по `b3` чека нет;
   - `[Allow, Allow]` → `Completed([Allow, Allow])`, `spawned == 2`, маркер создан;
   - **оборванный канал**: реестр отвечает ошибкой → `Blocked([Deny])`
     (`registry_refused`), `spawned == 0`, маркер `STARTED` не создан.
   - **ответ на `Ask` не воскрешает хвост** (характеризующий тест): `[Allow, Ask,
     Allow_B]` → `Blocked([Allow, Ask])`; возобновление — НОВЫЙ `run_gated` с НОВЫМ
     набором вызовов. `Allow_B` исходной пачки не исполняется сам по себе: чека по
     нему нет, `spawned` считает только явно переданное. У `run_gated` нет `self` и
     статики — скрытого состояния пачки между вызовами нет by construction; тест это
     фиксирует (внесённое скрытое состояние его роняет).

## Почему `mutation_gate: yes`

Мутант, который переносит решение после эффекта, исполняет хвост после не-`Allow`
или подменяет не-`Allow` на исполнение, превращает авторизацию в наблюдение и
обязан быть пойман. Критическая логика — гейт; тест обязан её удерживать.

## Запрещено трогать

`gateway::decide` и таблицу `RISK_PERMISSION` (`crates/stellwerk-gateway/**`) —
единственный источник правил; `m5/**`; `evidence.rs`; store; `justfile`;
`.github/`; `scripts/plan-lint.py`; `xtask/`; корневой `Cargo.toml`;
`modules.lock`. `run_once` и `authorize` использовать как есть.

## Приёмка

```sh
cargo build --workspace --all-targets; echo "BUILD_EXIT=$?"
cargo test -p stellwerk-task --test gated_loop; echo "TEST_EXIT=$?"
cargo fmt -p stellwerk-task -- --check; echo "FMT_EXIT=$?"
```

**Все три `0`.** Обязательные проверки:

```sh
grep -rn "run_gated" crates/stellwerk-task/src/ | grep -v tests/   # есть
grep -rn "registry_refused" crates/stellwerk-gateway/src/runner.rs 2>/dev/null || \
  grep -rn "registry_refused" crates/stellwerk-task/src/runner.rs   # есть
```

Эффект запрещённого вызова не произведён — проверяется `spawned()` и
отсутствием маркера `STARTED`, а не отсутствием события (отсутствие события
неотличимо от «событие записать забыли»).

## Семантика (решение владельца)

Первый не-`Allow` **останавливает** проход: после отказа политики (или `Ask`)
хвост пачки не запускается и не одобряется. Продолжение после ответа на `Ask` —
новый ход модели, а не повтор хвоста. `R5` — `Deny`, без inbox и без `Suspend`.
