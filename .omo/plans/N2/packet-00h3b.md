# N2-00h3b — площадка Build: копия без истории, политика C, слуга каналов

**label:** — · **mutation_gate:** no · **depends_on:** [N2-00h3a, 00h2a]

## Решение, которое здесь зафиксировано

Владелец выбрал **вариант C**: гейт разрешает чтение (`read`, R0) и запрещает оболочку
(`bash`, R5). Это значит, что в canary агент **не меняет рабочую копию** — доказывается
работающий гейт и провод, а не исполнение плана. Запись появится позже, когда будет
авторизация по путям; выдавать `bash` за «авторизованный инструмент» нельзя, потому что
гейт авторизует инструмент целиком, а не команду внутри него.

Что доказывает пакет: историю в песочницу не кладут; классы инструментов берутся из
доверенного реестра, а не из вызова; неизвестный инструмент получает `Deny` **с причиной**;
слуга каналов переживает неизвестное число обращений и выходит по флагу, а не висит в
`accept`.

## files_allowed

- `crates/stellwerk-cli/src/build_runner.rs` (новый)
- `crates/stellwerk-cli/src/lib.rs`
- `crates/stellwerk-cli/tests/build_runner.rs` (новый)

## Что делать

1. `copy_workspace(repo, dest)` — рекурсивная копия без `.git` (не «скопировать и удалить»:
   история не должна появляться в `work` даже на мгновение). Ссылки остаются ссылками.
2. `BUILD_TOOLS` / `BUILD_REGISTRY` — доверенный реестр: `read` = R0, `bash` = R5
   (единственный класс с безусловным `Deny` при любом уровне).
3. `BuildPolicy: GatePolicy` — `request_for` берёт класс из реестра, `interactive: false`
   (спросить некого, поэтому `Ask` не должен превращаться в ожидание). `Unclassified` —
   тоже ответ, а не `None`.
4. `serve_gate_until_stopped` / `serve_model_until_stopped` — цикл по `serve_once`, выход по
   флагу; `wake(socket)` — пустое подключение, дающее `EOF` и `NoRequest`, иначе `accept`
   не просыпается никогда. Возвращается число обслуженных обращений: ноль заявок
   отличается от «все разрешены».

`pi`, `bwrap`, `ModelProxy`+`HttpBackend` в этом пакете **не запускаются** — это 00h3c.

## Тесты (`tests/build_runner.rs`), 6 новых

- `a_workspace_copy_carries_no_history` — `.git` в источнике, копия без него, вложенные
  файлы на месте;
- `read_is_allowed_and_the_shell_is_denied` — `Allow` для `read`, `Deny` с классом `R5` для
  `bash` (через `gateway::gate::decide`, то есть настоящей таблицей, а не своим суждением);
- `an_unknown_tool_is_denied_with_a_reason` — `edit` вне реестра → `Deny` с
  `unclassified_tool` в причине;
- `the_gate_servant_stops_on_a_wake_connection` — флаг + пробуждение, поток выходит,
  обслужено 0;
- `the_model_servant_stops_on_a_wake_connection` — то же для канала модели;
- `the_gate_speaks_for_the_build_module` — модуль назван (`stellwerk-build`), а не взят из
  имени процесса.

## Приёмка

```
cargo test -p stellwerk-cli --test build_runner
cargo test -p stellwerk-cli
cargo clippy -p stellwerk-cli --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-cli -- --check
```

C2: `src` ≤ 200 логических строк, `tests` ≤ 250, 3 файла.

## Запрещено

Запускать `pi`/`bwrap`, ходить в сеть, писать `build.json`; классифицировать `bash` ниже R5
«чтобы canary прошёл»; возвращать `None` из `request_for` для неизвестного инструмента;
копировать `.git` с последующим удалением; трогать `stellwerk-task/src/cli.rs`.
