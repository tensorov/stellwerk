# packet-09d
plan_id: M0
goal: закрыть F-68 — на пути ошибки `audit_module` дочерний MCP-процесс не reap-ится. Добавить гарантированный reap на каждом раннем выходе: `impl Drop` для живого транспорта `Rpc<BufReader<ChildStdout>, ChildStdin>` с `kill`+`wait`, плюс сквозной тест, который доказывает исчезновение PID после отказа на handshake и не накопление процессов при повторе.
files_allowed: [crates/stellwerk-registry/src/cmd_audit.rs, crates/stellwerk-registry/tests/audit_live.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-registry
acceptance: `cargo test -p stellwerk-registry` → 0 failed, включая новый `handshake_failure_reaps_the_child_process`; новый тест падает на `main` до правки (RED) и проходит после (GREEN); `cargo fmt --all -- --check` → чисто; `cargo clippy --all-targets -- -D warnings` → чисто; полный `cargo test --workspace --no-fail-fast` → 0 failed.
forbidden: [cargo add, правка корневого Cargo.toml, правка Cargo.toml крейта, правка modules.lock, правка contract.rs, правка lib.rs/model.rs/cli.rs, правка modules.toml, правка пакетов 09/09b/09c и их тестов, #[ignore], #[allow], ослабление существующих тестов, async, tokio-рантайм, изменение соседних arms в match, перенаправление аргументов пользователя в команду]
depends_on: [packet-09b, packet-09c]

## Что чиним

F-68, измерен построчно. `audit_module`, ветка `kind = mcp`
(`crates/stellwerk-registry/src/cmd_audit.rs:357-360`):

```
357  let mut probe = Rpc::spawn(command, timeout_ms)?;
358  probe.handshake()?;
359  let report = audit_manifest(&module.tools, &mut probe, timeout_ms)?;
360  probe.shutdown();
```

`child.wait()` встречается в коде ровно один раз — строка 272, внутри `fn shutdown(self)`.
`impl Drop for Rpc` отсутствует. Любой отказ на строке 358 или 359 выходит из функции до
строки 360, `shutdown()` не вызывается, `Child` роняется без `wait()`, процесс остаётся зомби
до конца жизни родителя. Отказ на 358 — не редкий путь: битый JSON в ответе, оборванный кадр,
сервер, упавший на `initialize`.

## Почему нельзя просто вызвать `shutdown()` пораньше

`shutdown` объявлен как `fn shutdown(self)` и перемещает поля из `Self`. Rust не позволяет
частично перемещать поля из типа, у которого есть `Drop`, поэтому как только появится
`impl Drop`, существующий `shutdown(self)` перестанет компилироваться (E0509). Отсюда форма
правки: `shutdown` переезжает в concrete-impl и становится `fn(&mut self)`, а работа выносится в
`reap(&mut self)`, который вызывается и явно, и из `drop`.

Второе следствие того же ограничения: в `drop` нельзя переместить `writer`, а без его
освобождения `wait()` встал бы в deadlock (сервер ждёт EOF, родитель ждёт сервер). Поэтому
`reap` не relies на EOF, а делает `kill` + `wait`: `Child::kill` в std есть, транспорт его
поддерживает, и reap становится ограниченным по времени — зависший сервер не сможет подвесить
аудит. Побочный эффект осознанный: на пути успеха процесс-проба тоже завершается через `kill`,
а не мягким EOF. Для аудита это правильно: сервер поднят ради измерения, и его чистый выход
ничем не наблюдаем, тогда как мягкий `wait()` без предела — это отдельная зависание.

## Что сделать

1. **Тест сначала (RED).** Фикстура `SERVER_SH_CRASH`: sh-скрипт, который пишет свой PID в файл
   и завершается с ненулевым кодом, не ответив на `initialize`. Файл PID уникален на прогон
   (`unique_dir`), потому что `Command::new(command)` в `Rpc::spawn` не передаёт аргументов и
   путь приходится вписывать в тело скрипта.
2. **Утверждение на `/proc`.** PID, записанный фикстурой, обязан исчезнуть из `/proc` после
   ошибки. Зомби — это запись в `/proc` со state `Z`, то есть проверка через существование пути
   различает именно «reap» и «просто бросили `Child`». Прогон повторяется 25 раз, и проверяются
   все собранные PID: это отличает «ре reap-ится всегда» от «ре reap-ится почти всегда».
3. **Правка (GREEN).** `reap(&mut self)` + `impl Drop` для живого транспорта, `shutdown(&mut self)`
   вызывает `reap`. Явный вызов на 360 остаётся: он нужен, чтобы reap происходил до конца области
   видимости, а не в произвольный момент.

## Границы этого пакета

`request`, `list_tools` и транспорт в памяти не трогаем: `Drop` добавляется только для
`Rpc<BufReader<ChildStdout>, ChildStdin>`, то есть только для процесса. У транспорта в памяти
детей нет, и `shutdown` в concrete-impl для него недоступен — это осознанно, вызывать завершать
нечего.

`reader` остаётся `Option<R>`, потому что он и так уезжает в поток ожидания.

## Что этот пакет НЕ доказывает

Он не доказывает, что `read_response` не оставляет иных утечек, и не проверяет поведение под
нагрузкой на настоящих MCP-серверах. Проверка ограничена тем, что воспроизводимо детерминированно:
процесс, который упал на handshake.
