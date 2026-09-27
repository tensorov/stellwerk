# packet-01
plan_id: M4
goal: contract-change — проложить `stellwerk-gateway` в зависимости `stellwerk-task`, чтобы `decide` стал достижим из раннера.
files_allowed: [crates/stellwerk-task/Cargo.toml, Cargo.lock]
max_diff_lines: 200
test: cargo build -p stellwerk-task
acceptance: `cargo build -p stellwerk-task` → exit 0, и `cargo tree -p stellwerk-task` показывает `stellwerk-gateway` среди зависимостей. Зависимость объявлена как path, без `cargo add`.
forbidden: [cargo add, cargo install, правка крейтов кроме task/Cargo.toml, новые внешние зависимости, "попутные" правки]
depends_on: [—]
data_class: public
lane: m4/wire

Contract-Change: crates/stellwerk-task/Cargo.toml [dependencies] += stellwerk-gateway = { path = "../stellwerk-gateway" }

## Контекст

- **Этот пакет существует ради F-10, и он же его закрывает.**
- Факт, установленный аудитом: `decide()` живёт в `crates/stellwerk-gateway`, и
  `crates/stellwerk-task` от `stellwerk-gateway` **не зависит** — ноль вхождений
  в его `Cargo.toml`. Граф: `core ← store ← gateway`, и `task → core, store`.
  Значит раннер физически не может вызвать `decide`, пока зависимости нет.
- Проверено поиском: **0 пакетов** M2 и M3 подключают gateway к task. Все
  упоминания `stellwerk-gateway` в M3 — про его собственные тесты.
- Почему это критично, а не «недоделка»: M3-04 доведён до зелёного (`bd3a20d`,
  27/27), политика по R0–R5 и `Unclassified` выглядит полной — и при этом
  **в рантайме не вызывается ниоткуда**. Зелёный M3 даёт покрытие и документацию,
  но не авторизацию. Это тот же класс, что фильтр монтажа, убивший R3/R4 в M2:
  политика есть в тесте, но не на пути вызова.

## Что сделать

1. `crates/stellwerk-task/Cargo.toml`, секция `[dependencies]`:
   добавить `stellwerk-gateway = { path = "../stellwerk-gateway" }`.
   Только path, только из существующих крейтов. Новых внешних зависимостей нет.
2. `Cargo.lock` обновится сам при сборке — это единственная причина, по которой
   он в `files_allowed`. Правь руками не нужно.
3. Больше ничего. Ни одной строки кода.

## Почему пакет такой маленький

Проводка без первого потребителя мертва: если пакет только добавит зависимость и
закроется, через месяц неизвестно, зачем она. Нужен **потребитель** — им является
packet-02 (RED), который требует, чтобы раннер спрашивал разрешение на каждый
вызов. Связь видна из `depends_on`, и она не формальная: без packet-01
packet-02 не компилируется.

## Evidence

- `cargo build -p stellwerk-task` → exit 0.
- `cargo tree -p stellwerk-task` → среди зависимостей `stellwerk-gateway`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
