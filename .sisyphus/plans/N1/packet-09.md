# N1-09 — блокирующая авторизация до исполнения

**label:** — · **mutation_gate:** yes (≥75%)
**depends_on:** [N1-08, 06] · **ADR:** не требуется

## Цель

**Решение 6.** stdout-наблюдение **не является** pre-execution authorization.
Для каждого защищённого tool call нужен **блокирующий** путь до выполнения.
Если его нет — **назвать blocker**, не имитировать проверку по логам.

`authorize()` (`runner.rs:279`) вызывается **до** `Worker::exec` — это правильная
точка. Пакет обязан доказать, что блокирующий путь покрывает вызов, включая
те, что инициирует агент внутри процесса, а не только вызов оркестратора.

## Границы пакета (решение владельца)

**Model-transport в N1 НЕ строится.** Пакет проверяет **уже существующий**
код `secrets.rs` и ничего в нём не расширяет:

- `AgentEnv::assemble` (`secrets.rs:125`) — allowlist окружения. **Использовать как есть.**
- `agent_argv` (`secrets.rs:181`) — токен уходит в argv, не в env. **Использовать как есть.**
- `TokenProxy::send` (`secrets.rs:225`) — **НЕ реализовывать в N1.**

Транспорт вынесен в **N2-00** (`index.md`): security-критичный, отдельный
adversarial review, отдельный mutation gate, блокирует настоящий Pi smoke.

## production_call

- `runner::authorize` ← `orchestrator::execute` (до `run_once`, до spawn)
- `m5::secrets::AgentEnv::assemble` ← `orchestrator::SandboxWorker::exec`

## files_allowed

- `crates/stellwerk-task/src/orchestrator.rs`
- `crates/stellwerk-task/tests/authorization.rs`

**В лимите ADR-0003 (2 файла).** `secrets.rs` **не изменяется** — только
используется.

## Что делать

1. Доказать тестом, что `authorize()` вызывается **до** любого `spawn`.
   Регрессия: агент печатает вредоносный вызов в свой stdout — блокирующий путь
   обязан сработать **до** его исполнения, а не после.

2. Если для вызовов, инициируемых агентом внутри процесса, блокирующего пути
   **нет** — вернуть **BLOCKED с точным описанием разрыва** (решение 6).
   Это предусмотренный исход пакета, а не сбой.

3. Подключить `AgentEnv::assemble` к оркестратору: агентское окружение
   собирается по allowlist, а не наследуется целиком.

4. Проверить `agent_argv`: токен не должен оказываться в окружении агента.
   Это **существующая** гарантия — пакет её подключает, а не создаёт.

## Почему `mutation_gate: yes`

Мутант, переносящий `authorize` после spawn или пропускающий его при
определённом типе вызова, превращает блокировку в наблюдение. Обязан быть
пойман.

## Запрещено трогать

**`crates/stellwerk-task/src/m5/secrets.rs`** — использовать как есть, не менять.
`runner.rs` (`authorize` — как есть), `crates/stellwerk-gateway/**`
(`decide` — единственный источник правил), `sandbox_bwrap.rs`,
`session_identity.rs`, `integrator_profile.rs`, `m5/mod.rs`, `evidence.rs`,
store, `justfile`, `.github/`, `scripts/plan-lint.py`, `xtask/`.

## Приёмка

```sh
cargo build --workspace --all-targets; echo "BUILD_EXIT=$?"
cargo test -p stellwerk-task --test authorization; echo "TEST_EXIT=$?"
```

**Оба 0.** Обязательные тесты:

- запрещённый вызов → процесс **не стартовал** вообще (не «стартовал и был
  остановлен постфактум»);
- блокирующая точка срабатывает **до** `spawn`, а не после;
- окружение агента соответствует allowlist: переменная вне списка **не**
  попадает в процесс;
- токен присутствует в argv, но **не** в окружении агента;
- `secrets.rs` не изменён:

```sh
git diff --stat crates/stellwerk-task/src/m5/secrets.rs    # пусто
```

Проверки:

```sh
grep -rn "authorize" crates/stellwerk-task/src/ | grep -v tests/      # есть
grep -rn "AgentEnv::assemble" crates/stellwerk-task/src/ | grep -v tests/  # есть
```