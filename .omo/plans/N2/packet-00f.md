# N2-00f — Pi-расширение: инструмент только после вердикта хоста

**label:** — · **mutation_gate:** no (обоснование ниже)
**depends_on:** [N2-00e, 00c] · **ADR:** не требуется
overlap_waiver: C9 — N1 и N2 делят crates/stellwerk-task/src/orchestrator.rs, последовательное исполнение

## Цель

Pi стартует внутри `bwrap --unshare-net` с ровно одним явно подгруженным расширением:
модельный трафик идёт через `model.sock` (кадры N2-00d), каждый `tool_call` спрашивает
`gate.sock` (кадры N2-00e) **до** эффекта. Нет ответа — нет эффекта.

Инвариант:

> Ни один вызов инструмента не исполняется без `allow` от хоста. Недоступный гейт,
> ошибка расширения и расхождение набора инструментов — это блокировка (fail-safe), но
> причина блокировки различает `denied`, `suspended`, `unreachable` и `tool_set_mismatch`:
> «запрещено политикой» и «спросить было некого» остаются разными состояниями в статусе.

## Ограничение происхождения (зафиксировано явно)

Pi 0.87.0 **не отдаёт** `parentToolCallId`: в `@earendil-works/pi-coding-agent` тип
события `tool_call` несёт только `toolCallId`, `toolName`, `input` (Oracle, `types.d.ts`).
Расширение поэтому шлёт `"parent_tool_call_id": null`, и это означает «верхнеуровневый
вызов из хода модели, как его видит Pi», а **не** «хост подтвердил отсутствие родителя».
Вложенный вызов внутри инструмента хуком не виден; граница — песочница, а не хук.
Если политике понадобится происхождение — отдельный packet с `contract-change` и Pi,
который отдаёт поле.

## Известные пробелы (до P5)

- Структурные блоки без текста (assistant `toolCall`) в модельный канал не передаются:
  кадр `turn` знает только `user`/`assistant` + `text` (схема N2-00d). Текст результата
  инструмента при этом сохраняется с префиксом `[<role>]`, а не выбрасывается.
- `--mode json` в тестах, но не в `pi_transport_args`: режим вывода выберет P5, когда
  появится живой маршрут; `measure` воркера уже ждёт Pi-события.

## production_call

- `pi_transport_args` ← `SandboxWorker::exec`: argv несёт `-e <ext>`, `--offline`,
  `--no-session`, `--no-extensions`, `--no-context-files`, `--tools <csv>`.
- `stellwerk_transport.ts` ← Pi (`-e`), сокеты из env `STELLWERK_MODEL_SOCK` и
  `STELLWERK_GATE_SOCK`.
- Живой модели в этом пакете нет: backend в тестах — скриптовый (`ModelBackend`).

## files_allowed

- `crates/stellwerk-task/pi/stellwerk_transport.ts` (новый, ассет, не `src`)
- `crates/stellwerk-task/src/orchestrator.rs`
- `crates/stellwerk-task/tests/pi_transport.rs` (новый)

**В лимите ADR-0003 (3 файла).**

## Что делать

1. `pi_transport_args(extension: &Path, tools: &[&str]) -> Vec<String>` — детерминированный
   argv: `--offline --no-session --no-extensions --no-context-files -e <ext> --tools <csv>`.
   `--api-key` не передаётся никогда: токена в argv нет и быть не может (argv виден в
   `/proc` любому в namespace).
2. Второй канал виден внутри без правки песочницы: `SandboxWorker::with_sockets(model, gate)`
   требует один каталог на оба сокета и добавляет его в `read_only` (bind каталога, а не
   файла) — `gate.sock` монтируется тем же `--ro-bind`, что и остальные корни профиля.
   Разные каталоги — отказ до запуска; отсутствие сокета ловит `command_with_socket`.
   Обычный `with_socket` остаётся для одного канала и не меняется.
3. Расширение:
   - `registerProvider` + `streamSimple` поверх `model.sock`: кадр `turn` → поток Pi;
   - `pi.on("tool_call")` → `gate.sock`: кадр `tool_call`, ответ разбирается целиком;
   - `allow` → без `block`; `deny` → `{block:true, reason:"gate_denied:<reason>"}`;
     `suspended` → `gate_suspended`; `unclassified`/`unrecognised_source`/`unknown_tool`/
     `refused`/`ask` → `gate_<kind>`; транспорт/таймаут → `gate_unreachable:<detail>`;
   - текст из нетекстовых ролей (результат инструмента) не выбрасывается: он уходит
     `user`-сообщением с префиксом `[<role>]`, чтобы модель видела ход целиком.
4. Отсутствие env-переменной — ошибка загрузки расширения, а не «работаем без гейта».
5. Набор инструментов проверяет **ХОСТ**: расхождение `--tools` с набором сессии приходит
   как `unknown_tool`, политика при этом не вызывается. Клиентской сверки в расширении нет:
   Pi 0.87.0 не отдаёт набор сессии по проводу, а сравнение `getActiveTools()` с `--tools`
   сравнивало бы argv с самим собой и выдавало клиентскую проверку за границу доверия.
6. `event.input` не мутируется: гейт решает по ровно тем аргументам, которые увидел.

## Тесты (`tests/pi_transport.rs`), 7

Два юнит-теста на шов: `pi_args_carry_one_extension_and_no_key` (точный argv, без
`--api-key`) и `two_sockets_must_share_one_directory` (общий каталог обязателен).

Пять живых, `pi` в `bwrap --unshare-net`, `--mode json`:

- `pi_text_turn_through_socket_proxy` — скриптовый backend отдаёт `pong`, `stopReason == "stop"`;
- `pi_tool_call_is_blocked_before_effect` — модель просит `bash` создать canary, вердикт
  `deny`: canary **отсутствует**, `tool_execution_end.isError == true`, reason `gate_denied`;
- `pi_gate_unreachable_blocks` — гейта нет: canary отсутствует, reason `gate_unreachable`;
- `pi_allowed_tool_executes` — вердикт `allow`: canary есть (доказывает, что это не
  сплошной запрет);
- `pi_tool_set_mismatch_refused` — `--tools` шире набора сессии: гейт отвечает
  `unknown_tool`, canary отсутствует.

Внутрь монтируются только `/usr`, `/bin`, `/lib`, `/lib64`, `/etc` и
`~/.pi/agent/{bin,install}`; `auth.json` не монтируется. Env явный: `HOME` (внутри work),
`PATH`, `TMPDIR`, два сокета. Путь к запускатору — `PI_BIN` или `$HOME/.pi/agent/bin/pi`.
`pi` и `bwrap` — системные зависимости теста: их отсутствие падает, а не пропускается
(как `in_system` в `tests/pi_stream.rs`). `tool_execution_start` не считается эффектом.

## Почему `mutation_gate: no`

Rust-часть пакета — сборка argv, её ловит точное сравнение вектора. Расширение на
TypeScript вне `cargo-mutants`; его поведение закрывают пять интеграционных тестов выше,
включая оба исхода (заблокировано и исполнено) и недоступный гейт.

## Запрещено трогать

`gate_channel.rs`, `model_transport.rs`, `session_identity.rs`, `contract.rs`,
`sandbox_bwrap.rs`, `secrets.rs`, `justfile`, `.github/`, `scripts/plan-lint.py`,
корневой `Cargo.toml`, `modules.lock`. Новых зависимостей нет. Токен не появляется ни в
argv, ни в env, ни в файлах; `~/.pi/agent/auth.json` в песочницу не монтируется.

## Приёмка

```
cargo test -p stellwerk-task --test pi_transport
cargo test -p stellwerk-task
cargo build --workspace --all-targets
cargo clippy -p stellwerk-task --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-task -- --check
```

C2: `src` ≤ 200 логических строк, `tests` ≤ 250 строк, ровно 3 файла; TS-ассет ≤ 200
строк (он не `src`, но потолок тот же, чтобы не тащить файл-монстр).
