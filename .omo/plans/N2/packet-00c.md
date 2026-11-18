# N2-00c — песочница отдаёт хостовый AF_UNIX-сокет, не открывая сеть

**label:** — · **mutation_gate:** yes (граница песочницы)
**depends_on:** [N2-00b] · **ADR:** не требуется
overlap_waiver: C9 — N1 и N2 делят crates/stellwerk-task/src/orchestrator.rs, последовательное исполнение

## Цель

Первый шаг транспорта N2-00: у песочницы появляется **явный, проверяемый канал** к
хостовому процессу. `Sandbox::command_with_socket` добавляет к argv ровно один
`--ro-bind` на файл unix-сокета; `--unshare-net` остаётся на месте. Сокет — не
каталог и не записываемое дерево: агент не может его подменить, потому что bind
только на чтение.

Инвариант:

> Канал модели — это **файл сокета**, отданный внутрь только для чтения. Сеть
> по-прежнему отсутствует (`--unshare-net`), а сокет, лежащий в записываемом
> дереве, отвергается ДО запуска: иначе bind рабочей копии перекрыл бы его, а
> агент успел бы подменить файл между проверкой и монтированием.

Сам сокет здесь **не создаётся**: пакет даёт песочнице возможность его принять.
Протокол и хост-прокси — N2-00d.

## production_call

- `Sandbox::command_with_socket` ← `SandboxWorker::exec` (всегда, с `None` или
  `Some(path)`; отдельного «второго пути запуска» не появляется).
- `SandboxWorker::with_socket` ← проводка в Pi-запуск (N2-00f).

## files_allowed

- `crates/stellwerk-task/src/m5/sandbox_bwrap.rs`
- `crates/stellwerk-task/src/orchestrator.rs`
- `crates/stellwerk-task/tests/sandbox_socket.rs`

**В лимите ADR-0003 (3 файла).**

## Что делать

1. `Sandbox::command_with_socket(write, program, args, budget, socket: Option<&Path>)`.
   При `Some` — один `--ro-bind <path> <path>` **до** bind'ов `work`/`tmp`; при
   `None` — поведение `command` байт в байт (существующие тесты не меняются).
   `command` делегирует в `command_with_socket(.., None)`.
2. Проверка сокета — до запуска, три РАЗНЫХ отказа:
   - путь внутри `work` или `tmp` → `SocketInside`;
   - `NotFound` → `SocketMissing`;
   - не unix-сокет (в т.ч. симлинк: проверка `symlink_metadata`) → `SocketNotSocket`;
   - иное чтение метаданных → `SocketUnreadable` (ошибка с причиной).
   Молчаливого `Ok` на «не посмотрели» нет: отсутствие канала — состояние, а не
   разрешение.
3. `SandboxWorker` получает приватное `socket: Option<PathBuf>` (в `new` — `None`,
   поэтому существующие конструкторы не меняются) и builder `with_socket`.
   `exec` зовёт **только** `command_with_socket` — второго пути запуска нет.
4. Тесты (`tests/sandbox_socket.rs`):
   - `socket_seam_keeps_unshare_net_and_binds_read_only` — `--unshare-net` на месте;
     ровно один `--ro-bind` на файл сокета; ни одного `--bind` на него;
   - `a_socket_inside_the_writable_tree_is_refused_before_launch` — `work` и `tmp`
     дают `SocketInside`, маркер не создан;
   - `an_absent_socket_is_absent_not_allowed` — `SocketMissing`; обычный файл —
     `SocketNotSocket`;
   - `a_sandboxed_client_reaches_the_host_socket_without_network` — настоящий
     roundtrip: хост слушает сокет, клиент внутри `bwrap` (python3 из ro-bind `/usr`)
     шлёт `PING`, получает `PONG`. Без сети, без `--bind` сокета.

## Почему `mutation_gate: yes`

Мутант, заменяющий `--ro-bind` сокета на `--bind`, убирающий проверку «внутри
записываемого дерева» или возвращающий `Ok` на отсутствующий сокет, превращает
канал в дыру. Критическая логика — граница; тест обязан её удерживать.

## Запрещено трогать

`--unshare-net` и остальные namespace-флаги; `gateway::decide` / `RISK_PERMISSION`;
`runner.rs`; `secrets.rs` (токен и `TokenProxy` — N2-00d/g); `evidence.rs`; store;
`justfile`; `.github/`; `scripts/plan-lint.py`; корневой `Cargo.toml`;
`modules.lock`; `~/.pi/agent/**` (в частности `auth.json`). Новых зависимостей нет:
только `std::os::unix`.

## Приёмка

```sh
cargo build --workspace --all-targets; echo "BUILD_EXIT=$?"
cargo test -p stellwerk-task --test sandbox_socket; echo "TEST_EXIT=$?"
cargo test -p stellwerk-task --test sandbox_bwrap; echo "REGRESSION_EXIT=$?"
cargo fmt -p stellwerk-task -- --check; echo "FMT_EXIT=$?"
grep -rn "command_with_socket" crates/stellwerk-task/src/orchestrator.rs   # есть
grep -c '"--unshare-net"' crates/stellwerk-task/src/m5/sandbox_bwrap.rs    # 1
```

**Все четыре `0`.** Отсутствие сокета проверяется отказом `SocketMissing`, а не
отсутствием маркера: маркер доказывает лишь, что процесс не стартовал.
