# N2-00h3c — один прогон Build: `pi` в песочнице через наши каналы, без сети

**label:** — · **mutation_gate:** no · **depends_on:** [N2-00h3b, 00h3b]

## Что здесь доказывается и чего нет

Доказывается: `pi` **действительно стартует** внутри `bwrap`, говорит через `model.sock`,
получает вердикт через `gate.sock`, и оболочка запрещается **до эффекта** — канарейка не
создана, причина `gate_denied` в выводе агента. Копия дерева уходит в песочницу без `.git`.

Не доказывается: ответ настоящей модели. Бэкенд здесь **скриптованный** — токена в окружении
нет, и подставлять заглушку под видом живого хода было бы ровно тем, что запрещает закон 1.
Живой ход — N2-00a, после того как владелец положит токен в
`$XDG_CONFIG_HOME/stellwerk/neuraldeep.token` (0600).

## files_allowed

- `crates/stellwerk-cli/src/build_runner.rs`
- `crates/stellwerk-cli/tests/build_runner.rs`

## Что делать

1. `BuildRun { request, repo, root, extension, pi }` — расширение и `pi` приходят путями:
   модуль не угадывает раскладку крейтов и не ищет бинарь по `PATH`.
2. `run_build(run, backend)`:
   - каталоги `work`/`tmp`/`sock` под `root`; `copy_workspace` в `work`; расширение
     копируется в `work`;
   - `ModelProxy::bind` + `GateChannel::bind` в одном каталоге (иначе `with_sockets`
     отказывает);
   - `Sandbox` с read-only `/usr /bin /lib /lib64 /etc` + каталог `pi` и `install`;
   - argv: `pi_transport_args(&extension, &["bash","read"])` + `--provider stellwerk
     --model proxy --mode json -p <план>`; окружение явное (`HOME`, `PATH`, `TMPDIR`,
     оба сокета);
   - слуги модели и гейта в `thread::scope`; после `exec` — флаг и пробуждение обоих
     каналов, затем join;
   - `BuildPolicy` из 00h3b: `read` разрешён, `bash` запрещён (R5), `edit` — `Deny` с
     причиной.
3. `BuildOutcome { model_turns, gate_requests, output }`. Отказ гейта **не** превращается
   здесь в «прогон не удался»: он виден в выводе агента (`gate_denied`) и в счётчике заявок.
   Кода возврата в `ExecOutcome` нет — поэтому его нет и в отчёте: выдумывать поле нельзя.

## Тесты (`tests/build_runner.rs`), 2 новых

- `the_build_runs_pi_through_our_channels` — `pi` стартует, отвечает `pong` через наш
  прокси, `model_turns >= 1`, `.git` в `work` отсутствует;
- `the_shell_is_denied_before_it_touches_anything` — модель просит `bash` с `touch`
  на канарейку: файла нет, в выводе `gate_denied`, `gate_requests >= 1`.

Оба теста живые: требуют `pi` и `bwrap` на машине (как 00f). В CI `stellwerk-cli` не
прогоняется.

## Приёмка

```
cargo test -p stellwerk-cli --test build_runner
cargo test -p stellwerk-cli
cargo clippy -p stellwerk-cli --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-cli -- --check
```

C2: `src` ≤ 200 логических строк, `tests` ≤ 250, 2 файла.

## Запрещено

Ходить в сеть, читать `auth.json`, подставлять `HttpBackend` без токена, выдавать
скриптованный бэкенд за живой ход, классифицировать `bash` ниже R5, отключать `bwrap`,
добавлять `--unshare-net`-обход, трогать `stellwerk-task/src/cli.rs`.
