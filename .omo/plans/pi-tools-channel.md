# pi-tools-channel - Work Plan

## TL;DR (For humans)

**What you'll get:** в режиме Build агент наконец правит файлы сам: модель получает список инструментов, вызывает `edit`/`write`, а результат вызова возвращается ей с тем же идентификатором. Текстовые SEARCH/REPLACE-патчи вместо действий остаются в прошлом; `bash` агенту не предлагается, а политика R5 по-прежнему отказывает, если такой вызов придёт из чужого набора.

**Why this approach:** чинится не промпт, а протокол в две стороны. Вниз едет `tools`, вверх — `tool_calls` и `tool_results` с `tool_call_id`, а причина завершения хода перестаёт быть константой `"stop"`. Отсутствие `tools` и пустой набор — разные состояния: первое означает «старый клиент, набор неизвестен», второе — сознательное «инструментов нет».

**What it will NOT do:** не ослабляет R5; не добавляет зависимости, сеть и стриминг; не обрезает результаты инструментов и не считает их доказательством.

**Effort:** Large
**Risk:** Medium - протокол меняется в трёх местах сразу (Rust-транспорт, HTTP-бэкенд, TS-расширение), а конец цепочки проверяется реальным бинарём Pi.

**Decisions to sanity-check:** `bash` исчезает из `--tools`, но остаётся в `BUILD_HELD` гейта (иначе R5 неотличим от «инструмента нет»); потолок кадра 512 КиБ общий для модели и гейта; `tools` становится обязательным только последним пакетом; ошибка инструмента едет как `<tool_error>`, а не как evidence.

Your next move: одобрить и запустить исполнение (`$start-work`), либо сначала прогнать независимое ревью (Momus + Oracle). Полный план ниже.

---

> TL;DR (machine): Large/Medium — 9 пакетов 00j1–00j7 + 00j3a + 00j7a: tools вниз, tool_calls/tool_results вверх с id, честный done.reason, Pi-расширение, bash вне --tools, потолок 512 КиБ, tools обязателен в конце.

## Scope

### Must have

1. `--tools` для Build содержит только `read`, `edit`, `write`; `BUILD_HELD` гейта по-прежнему держит `bash`, поэтому вызов из чужого/legacy набора получает `R5 Deny`, а не `unknown_tool`.
2. Один кадр транспорта вмещает транскрипт с результатами `read` (потолок 512 КиБ, общий для model proxy и gate channel).
3. Кадр `turn` несёт `tools: Option<Vec<ToolDef>>`: `None` — legacy, набор неизвестен, валидация не применяется; `Some([])` — явное «инструментов нет», любой вызов отвергается; `Some(list)` — вызов вне списка отвергается.
4. `tool_not_offered` — протокольный отказ: пишется только error-кадр, ни текст, ни `done` хода не пишутся.
5. `done.reason` выводится из фактически отправленных кадров: `tool_use` при наличии `Frame::ToolCall`, иначе `stop`.
6. HTTP-бэкенд рендерит `tools` как OpenAI function definitions только при непустом `Some`; при `None`/`Some([])` ключа `tools` в теле нет.
7. Транскрипт сохраняет форму разговора: `system`, `user`, `assistant` с `tool_calls`, `tool` с `tool_call_id`/`name`/`is_error`; `tool_calls` по умолчанию пуст.
8. Расширение Pi собирает `tools` через `getCurrentTools(context.messages)`, возвращает результаты вызовов как `toolResult` с `toolCallId` и сверяет `done.reason` с кадрами.
9. `tools` обязателен в кадре `turn`; отсутствие — `schema`, пустой массив — явное «инструментов нет».
10. Планировщик явно шлёт `"tools": []` и остаётся без инструментов; это отдельный пакет, чтобы не нарушить ≤3 файла.

### Must NOT have (guardrails, anti-slop, scope boundaries)

- Не менять `permission.rs`, `gate.rs`, `contract.rs`, классификацию `BUILD_TOOLS`, `authorize()` в расширении.
- Не добавлять зависимости (`cargo add`, npm) и не расширять `[workspace.dependencies]`.
- Не включать сеть в песочнице, не читать `~/.pi/agent/auth.json`.
- Не обрезать и не суммаризировать результаты инструментов; превышение потолка — честный `oversize_frame`.
- Не считать вывод инструментов evidence: evidence — только stdout Pi, ModelClaim/ToolAttestation.
- Не включать стриминг, `tool_choice`, `parallel_tool_calls`, `--mode rpc`.
- Не давать планировщику инструменты: он шлёт `"tools": []`, `tool_call` остаётся ошибкой.
- Не использовать `#[serde(deny_unknown_fields)]` на internally tagged enum (serde это не поддерживает): неизвестные ключи проверяются по сырому объекту в `parse`.
- Не трогать owner-WIP: `.omo/omo.jsonc`, `.gitignore`, `crates/stellwerk-gateway/src/shaping.rs`, `crates/stellwerk-gateway/tests/{mount,session_failure,shaping}.rs`, `crates/stellwerk-registry/src/cmd_add.rs`, `crates/stellwerk-task/src/cli.rs`.
- Не править `plan-lint.py`: его слепота — отдельный FINDING, не этот пакет.

## Verification strategy

> Zero human intervention - all verification is agent-executed.

- Test decision: **TDD** (Rust `cargo test` + интеграционные тесты с реальным бинарём Pi 0.87.0).
- Evidence: `.omo/evidence/pi-tools-channel/task-<N>-<name>.txt` (не в ulw-loop; каталог создаётся воркером). Каждый пакет: RED-вывод, GREEN-вывод, QA-вывод.
- Гейт пакета: `cargo fmt --check`, `cargo clippy -p <crate> --all-targets --no-deps -- -D warnings`, профильные тесты. `plan-lint` **не** считается доказательством (слепой `parse()`, Appendix A).
- Порядок merge: тест-сьютка на `main` **до** merge (AGENTS: INFRA-04), затем `merge --no-ff`, затем `evidence_verify` на `main`, `gate_evaluate`, отдельный `chore(evidence)` коммит.

## Execution strategy

### Parallel execution waves

> Target 5-8 todos per wave. Fewer than 3 (except the final) means you under-split.

Пакеты 00j3–00j7 последовательны по протоколу и по файлам (C1: один и тот же
`model_transport.rs` / `model_backend.rs`), параллелить их нельзя без конфликта.
Реальный параллелизм есть только в W1.

- **W0** — 1 (материализация канон-пакетов).
- **W1** — 2 и 3 независимы (разные крейты, разные файлы).
- **W2** — 4 (после 2 и 3: тот же `model_transport.rs`).
- **W3** — 5 (планировщик, после 4).
- **W4** — 6 (backend, после 5).
- **W5** — 7 (сообщения, после 6).
- **W6** — 8 (Pi, после 7).
- **W7** — 9 (обязательность `tools`, после 8).
- **W7a** — 10 (test-only follow-up `00j7a`: `model_backend` wire-хелпер, после 9).
- **FW** — F1–F4 параллельно.

### Dependency matrix

| Todo | Depends on | Blocks | Can parallelize with |
| --- | --- | --- | --- |
| 1 | — | 2-10 | — |
| 2 | 1 | 4 | 3 |
| 3 | 1 | 4 | 2 |
| 4 | 2, 3 | 5 | — |
| 5 | 4 | 6 | — |
| 6 | 5 | 7 | — |
| 7 | 6 | 8 | — |
| 8 | 7 | 9 | — |
| 9 | 8 | 10 | — |
| 10 | 9 | F1-F4 | — |
| F1-F4 | 1-10 | — | друг с другом |

## Todos

> Implementation + Test = ONE todo. Never separate.
<!-- APPEND TASK BATCHES BELOW THIS LINE WITH edit/apply_patch - never rewrite the headers above. -->

- [x] 1. Материализовать канон-пакеты 00j1–00j7 и 00j3a
  What to do / Must NOT do: создать `.sisyphus/plans/N2/packet-00j1.md`, `packet-00j2.md`, `packet-00j3.md`, `packet-00j3a.md`, `packet-00j4.md`, `packet-00j5.md`, `packet-00j6.md`, `packet-00j7.md` **дословно** из тел todos 2–9 этого плана (todo 1 — сама материализация). Каждый файл: YAML-frontmatter (`id: N2/00jN`, `milestone: N2`, `title`, `depends_on`, `allowed_changes` ровно из соответствующего todo) + разделы «Зачем / Что делается / RED / Приёмка / Не делается / Бюджет». Не менять формулировки по существу; не писать код и тесты. Причина шага: песочница планировщика не пишет в `.sisyphus/`, канон создаёт исполнитель.
  Parallelization: Wave 0 | Blocked by: — | Blocks: 2-10
  References: `.sisyphus/plans/N2/packet-00i1.md` (формат frontmatter), `AGENTS.md` («Источник истины для пакетов»), этот план, todos 2-10.
  Acceptance criteria: 8 файлов; `grep -l '^id: N2/00j' .sisyphus/plans/N2/packet-00j*.md | wc -l` = 8; у каждого есть `^depends_on:` и `^allowed_changes:`; набор файлов в `allowed_changes` совпадает с todo; в каждом встречаются имена его RED-тестов; `git status --short` показывает только эти 8 файлов.
  QA scenarios (happy + failure): happy — 8 файлов, 8 id, 8 `depends_on`, 8 `allowed_changes`; failure — пропустить `allowed_changes` в одном файле, `grep -L '^allowed_changes:'` его ловит; вернуть. Evidence `.omo/evidence/pi-tools-channel/task-1-packets.txt`.
  Commit: Y | docs(plan): materialize N2 packets 00j1-00j7 and 00j3a

- [x] 2. Build больше не предлагает bash, но гейт помнит R5 (пакет 00j1)
  What to do / Must NOT do: `pub const BUILD_OFFERED: &[&str] = &["read","edit","write"]` и `pub const BUILD_HELD: &[&str] = &["read","bash","edit","write"]`; `pub fn build_args(extension: &Path) -> Vec<String>` использует `pi_transport_args(extension, BUILD_OFFERED)` (публичная, потому что тест живёт в отдельном крейте `tests/`); `run_build` вызывает `build_args` (вместо литерала `:383`) и `ToolSet::listed(BUILD_HELD)` (вместо литерала `:362`). Заменить end-to-end `the_shell_is_denied_before_it_touches_anything` (`tests/build_runner.rs:243-281`) на два теста: `the_build_does_not_offer_bash` (`BUILD_OFFERED` и `build_args` без bash) и `bash_stays_a_gate_level_deny` (`BUILD_HELD` содержит bash; `session(BUILD_HELD).adjudicate("bash")` + `BuildPolicy::decide_call` даёт R5-решение). Старый тест удаляется: Pi не вызывает необъявленный инструмент, end-to-end R5 через Build-сессию недостижим by design; это покрывается legacy/чужим набором в 00j3 и `tests/gate_channel.rs`. Не трогать `permission.rs`, `gate.rs`, командный фильтр.
  Parallelization: Wave 1 | Blocked by: 1 | Blocks: 4
  allowed_changes: crates/stellwerk-cli/src/build_runner.rs, crates/stellwerk-cli/tests/build_runner.rs
  References: `crates/stellwerk-cli/src/build_runner.rs:34-66` (BUILD_TOOLS), `:356-367` (held), `:383` (args); `crates/stellwerk-cli/tests/build_runner.rs:243-281` (удаляемый тест), `:300-320` (`adjudicate`); `crates/stellwerk-task/src/m5/permission.rs` (R5 `Deny`); `crates/stellwerk-task/tests/gate_channel.rs` (R5-путь чужого набора).
  Acceptance criteria: `the_build_does_not_offer_bash` — `BUILD_OFFERED == ["read","edit","write"]`, `build_args` содержит `--tools` + `read,edit,write`, без `bash`; `bash_stays_a_gate_level_deny` — `BUILD_HELD` содержит bash, решение R5; `cargo test -p stellwerk-cli --test build_runner` — 16 passed (15 − 1 + 2); clippy/fmt чисто.
  QA scenarios (name the exact tool + invocation): happy — оба новых теста; failure — вернуть `"bash"` в `BUILD_OFFERED`, `the_build_does_not_offer_bash` падает; вернуть. Evidence `.omo/evidence/pi-tools-channel/task-2-build-set.txt`.
  Commit: Y | feat(00j1): stop offering bash to the build agent

- [x] 3. Кадр вмещает транскрипт с инструментами (пакет 00j2)
  What to do / Must NOT do: `MAX_FRAME_BYTES = 512 * 1024` (`model_transport.rs:21`), комментарий «512 КиБ, общий для model proxy и gate channel»; `a_frame_at_the_size_limit_is_served_not_refused` собирает кадр `payload.resize(512 * 1024 - 1, b' '); payload.push(b'\n')` — литерал, не только константа; `an_oversize_frame_is_refused_before_it_is_buffered` и `tests/gate_channel.rs` используют `MAX_FRAME_BYTES + 1`; переименовать `an_oversize_gate_frame_is_refused` в `a_gate_frame_over_the_limit_is_refused`. `gate_channel.rs` **не** меняется: он импортирует константу и получает новый потолок автоматически. Не обрезать сообщения, не включать стриминг.
  Parallelization: Wave 1 | Blocked by: 1 | Blocks: 4
  allowed_changes: crates/stellwerk-task/src/m5/model_transport.rs, crates/stellwerk-task/tests/model_transport_wire.rs, crates/stellwerk-task/tests/gate_channel.rs
  References: `crates/stellwerk-task/src/m5/model_transport.rs:21`, `:176-205` (`read_frame`); `crates/stellwerk-task/src/m5/gate_channel.rs:20,192,260-268`; `crates/stellwerk-task/tests/gate_channel.rs:258-266`; `crates/stellwerk-task/tests/model_transport_wire.rs:160-185`.
  Acceptance criteria: at-limit (`512 * 1024` bytes включая `\n`) — `Served::Turn`; oversize (`MAX_FRAME_BYTES + 1`) — `OversizeFrame`; `a_gate_frame_over_the_limit_is_refused` (MAX+1); `cargo test -p stellwerk-task --test model_transport_wire --test gate_channel`; clippy/fmt чисто.
  QA scenarios: happy — оба набора; failure — вернуть `64 * 1024`, at-limit и oversize падают (at-limit использует литерал 512 КиБ); вернуть. Evidence `.omo/evidence/pi-tools-channel/task-3-frame-limit.txt`.
  Commit: Y | feat(00j2): let one frame carry a tool-sized transcript

- [x] 4. Ход несёт определения инструментов и честную причину (пакет 00j3)
  What to do / Must NOT do: `ToolDef {name, description, parameters: Value}` и `TurnRequest {tools: Option<Vec<ToolDef>>, messages}`; `parse` возвращает `TurnRequest`, белый список `kind|tools|messages`; `tools` **необязателен** и различает состояния: `None` — legacy (набор неизвестен, валидация вызовов не применяется), `Some([])` — явное «инструментов нет» (любой `ToolCall` отвергается), `Some(list)` — вызов вне списка отвергается. Каждый `ToolDef`: непустое имя, `parameters` — объект, иначе `Schema`. В трейт добавить `turn_request(&TurnRequest)` с умолчанием: `Some` непустой ⇒ `BackendError("backend does not accept tools")`, иначе `turn(&messages)`. `serve_once` зовёт `turn_request`; при `Some` проверяет каждый `ToolCall.name` и при нарушении пишет **только** `Error{ToolNotOffered}` + `Served::Refused`. `done.reason` = `tool_use` при наличии `ToolCall`, иначе `stop` (это меняет ожидание `a_turn_roundtrip_streams_text_tool_and_done` с `stop` на `tool_use`, потому что `TURN` содержит bash-вызов). `ErrorCode::ToolNotOffered` (`tool_not_offered`). Добавить `Recording` backend в `tests/model_transport_wire.rs`: реализует `turn` и `turn_request`, запоминает **имена** `tools` (не `TurnRequest` целиком: `ToolDef`/`TurnMessage` ещё не `Clone`), возвращает сценарий; `TurnOnly` — только `turn` для проверки умолчания. Планировщик **не** трогать: его `"tools": []` — отдельный пакет 00j3a. Не менять рендер HTTP и `TurnMessage`.
  Parallelization: Wave 2 | Blocked by: 2, 3 | Blocks: 5
  allowed_changes: crates/stellwerk-task/src/m5/model_transport.rs, crates/stellwerk-task/tests/model_transport_wire.rs
  References: `crates/stellwerk-task/src/m5/model_transport.rs:30-34`, `:56-68`, `:70-76`, `:107-174` (`serve_once`), `:136-140` (`done.reason`), `:207-240` (`parse` + allowlist); `crates/stellwerk-task/tests/model_transport_wire.rs:9-30` (`Scripted`), `:92-113` (`TURN` и roundtrip).
  Acceptance criteria: `a_legacy_turn_is_forwarded_without_offer_validation` (`TURN` без tools + bash → Text, ToolCall, Done `tool_use`); `an_explicit_empty_tool_set_refuses_any_call` (payload `"tools":[]` + bash → только `tool_not_offered`); `a_call_outside_the_offered_set_is_refused` (`Some([read])` + bash); `a_call_inside_the_offered_set_is_forwarded` (`Some([bash])` + bash → Done `tool_use`); `a_text_only_turn_still_ends_with_stop`; `a_backend_without_tool_support_refuses_offered_tools` (`TurnOnly` + `Some([read])`); `cargo test -p stellwerk-task --test model_transport_wire`, `--test pi_transport` (legacy Scripted идёт через умолчание), `cargo test -p stellwerk-cli --test build_runner`.
  QA scenarios: happy — весь wire-набор; failure — снять проверку `Some(list)`, `a_call_outside...` падает; вернуть. Evidence `.omo/evidence/pi-tools-channel/task-4-turn-tools.txt`.
  Commit: Y | feat(00j3): carry tool definitions in a turn and derive its ending

- [x] 5. Планировщик явно не предлагает инструменты (пакет 00j3a)
  What to do / Must NOT do: вынести сборку кадра в `fn turn_frame(prompt: &str) -> Value` — `{"kind":"turn","tools":[],"messages":[{"role":"user","text":prompt}]}`; `ask` использует `turn_frame`. `read_reply` по-прежнему отвергает `tool_call` (`:118`). Unit-тесты в `planner_host.rs`: `the_planner_sends_an_explicit_empty_tool_set` проверяет `frame["tools"] == json!([])`; `a_tool_call_is_still_unexpected` через `UnixStream::pair` проверяет `UnexpectedTool`. В `tests/planner_host.rs` добавить **только** `the_planner_does_not_forward_tools_to_the_service`: HTTP-тело не содержит `"tools"` — ключ опускается и в 00j3a, и в 00j4, потому что `HttpBackend` не рендерит `Some([])`. **Не** проверять `"tools":[]` в HTTP: это невозможно по контракту 00j4. Не давать планировщику инструменты, не менять его промпт.
  Parallelization: Wave 3 | Blocked by: 4 | Blocks: 6
  allowed_changes: crates/stellwerk-cli/src/planner_host.rs, crates/stellwerk-cli/tests/planner_host.rs
  References: `crates/stellwerk-cli/src/planner_host.rs:91-103`, `:106-118`; `crates/stellwerk-cli/tests/planner_host.rs:32-44` (harness `planner`), `:89-104` (`one_turn...`).
  Acceptance criteria: unit `the_planner_sends_an_explicit_empty_tool_set`; unit `a_tool_call_is_still_unexpected`; integration `the_planner_does_not_forward_tools_to_the_service`; `cargo test -p stellwerk-cli --lib planner_host` (2 tests), `cargo test -p stellwerk-cli --test planner_host` (7 tests); clippy/fmt чисто.
  QA scenarios: happy — три теста; failure — убрать `"tools":[]`, unit-тест кадра падает; вернуть. Integration-тест проверяет отсутствие ключа, а не его значение — это осознанно. Evidence `.omo/evidence/pi-tools-channel/task-5-planner.txt`.
  Commit: Y | feat(00j3a): tell the model the planner offers no tools

- [x] 6. Backend предлагает инструменты сервису (пакет 00j4)
  What to do / Must NOT do: `HttpBackend` переопределяет `turn_request`; `turn` делегирует ему с `tools: None` (без рекурсии, потому что `turn_request` не зовёт `turn`); при `Some` непустом в тело добавить `"tools":[{"type":"function","function":{"name","description","parameters"}}]` в порядке `TurnRequest.tools`; при `None` и `Some([])` ключа `"tools"` в теле нет; разбор ответа не менять. Не добавлять `tool_choice`, `parallel_tool_calls`, стриминг.
  Parallelization: Wave 4 | Blocked by: 5 | Blocks: 7
  allowed_changes: crates/stellwerk-task/src/m5/model_backend.rs, crates/stellwerk-task/tests/model_backend.rs
  References: `crates/stellwerk-task/src/m5/model_backend.rs:40-60` (тело), `:78-134` (разбор `tool_calls`); `crates/stellwerk-task/tests/model_backend.rs` (локальный сервер, захват тела).
  Acceptance criteria: `the_request_offers_the_turn_tools`; `an_empty_tool_set_sends_no_tools_key` (`Some([])`); `a_legacy_turn_sends_no_tools_key` (`None`); `tools_render_as_function_definitions`; `cargo test -p stellwerk-task --test model_backend`; wire-тесты не регрессировали.
  QA scenarios: happy — четыре теста; failure — слать `"tools":[]` при `Some([])`, `an_empty_tool_set_sends_no_tools_key` падает; вернуть. Evidence `.omo/evidence/pi-tools-channel/task-6-body-tools.txt`.
  Commit: Y | feat(00j4): offer the turn's tools to the model service

- [x] 7. Результат инструмента возвращается в разговор (пакет 00j5)
  What to do / Must NOT do: `TurnMessage` — internally tagged enum (`role`): `system{text}`, `user{text}`, `assistant{text: Option<String>, #[serde(default)] tool_calls: Vec<AssistantCall>}`, `tool{tool_call_id, name, text, is_error}`; `AssistantCall{id, name, arguments: Value}`; **не** ставить `deny_unknown_fields` на enum — `parse` проверяет набор ключей сырого объекта по роли (неизвестный ключ ⇒ `Schema`), затем десериализует; валидация: assistant без текста и вызовов, пустой `ToolCall.id`, `arguments` не объект, пустой `tool_call_id` — `Schema`. Рендер: system/user как есть; assistant — `content: text|null`, `tool_calls` (ключ опускается при пустом), `arguments` **строкой**; tool — `{"role":"tool","tool_call_id","content"}`, при `is_error` контент в `<tool_error>…</tool_error>`; `TurnMessage: Clone`. Обновить `user()` в `tests/model_backend.rs:26-31`. Не реплеить `thinking`, не передавать картинки (пометка `[non-text content omitted: N]`).
  Parallelization: Wave 5 | Blocked by: 6 | Blocks: 8
  allowed_changes: crates/stellwerk-task/src/m5/model_transport.rs, crates/stellwerk-task/src/m5/model_backend.rs, crates/stellwerk-task/tests/model_backend.rs
  References: `crates/stellwerk-task/src/m5/model_transport.rs:56-68`; `crates/stellwerk-task/src/m5/model_backend.rs:78-134`; `crates/stellwerk-task/tests/model_backend.rs:15,26-31`.
  Acceptance criteria: `a_tool_result_keeps_the_call_id_it_answers`, `an_assistant_turn_carries_the_calls_it_made`, `a_system_message_stays_a_system_message`, `tool_results_render_as_openai_tool_messages`, `an_assistant_tool_call_renders_arguments_as_a_string`, `a_tool_message_without_a_call_id_is_refused`, `an_assistant_with_neither_text_nor_calls_is_refused`, `an_unknown_message_key_is_refused`, `tool_calls_default_to_empty`; `cargo test -p stellwerk-task --test model_backend`, `--test model_transport_wire`, `cargo test -p stellwerk-cli --test planner_host`.
  QA scenarios: happy — набор; failure — сериализовать `arguments` объектом, тест падает; вернуть. Evidence `.omo/evidence/pi-tools-channel/task-7-message-shapes.txt`.
  Commit: Y | feat(00j5): keep the shape of a tool conversation

- [x] 8. Pi перестаёт быть текстовым реле (пакет 00j6)
  What to do / Must NOT do: импорт `getCurrentTools`/`getCurrentSystemPrompt` из `@earendil-works/pi-ai` (сверить фактический экспорт в `package.json`; объявления — `dist/utils/transcript.d.ts:26,34`, `Tool` — `dist/types.d.ts:423-428`); `MAX_FRAME_BYTES` в TS — `512 * 1024`; `transcript(context)`: system из `getCurrentSystemPrompt(messages)` если непустой; user как есть; assistant — текст + `content.filter(type==="toolCall")` → `tool_calls` (пустой текст ⇒ `null`, сообщение с вызовами не выбрасывать); `toolResult` → `{tool_call_id: toolCallId, name: toolName, text, is_error: isError}` (`dist/types.d.ts:376-386`); не-текстовые блоки — `[non-text content omitted: N]`; прочие роли — `user` с видимым `[role]`; запрос `{kind:"turn", tools: getCurrentTools(messages).map(...), messages}`; `finish` берёт `done.reason`, `tool_use` → `toolUse`, противоречие причина/кадры — ошибка. `authorize()` и `pi.on("tool_call")` не менять. `Scripted` в `tests/pi_transport.rs` и `tests/build_runner.rs` получает `turn_request(&TurnRequest)` (в pi — с записью имён `tools` и копии `messages`, в build_runner — делегирование), иначе умолчание отвергнет непустые tools. В `tests/pi_transport.rs` `case()` (`:130`) список `["bash","read"]` не переиспользовать для проверки tools — брать отдельный read/edit/write.
  Parallelization: Wave 6 | Blocked by: 7 | Blocks: 9
  allowed_changes: crates/stellwerk-task/pi/stellwerk_transport.ts, crates/stellwerk-task/tests/pi_transport.rs, crates/stellwerk-cli/tests/build_runner.rs
  References: `crates/stellwerk-task/pi/stellwerk_transport.ts:13` (лимит), `:99-118` (`textOf`/`transcript`), `:135-143` (request/finish), `:170`, `:198` (`authorize`); `crates/stellwerk-task/tests/pi_transport.rs:79-84` (Scripted), `:130` (`case`), `:171-192` (Scripted), `:227-229` (R5); `crates/stellwerk-cli/tests/build_runner.rs:157-168` (Scripted); `~/.pi/agent/install/releases/0.87.0/node_modules/@earendil-works/pi-ai/dist/{types.d.ts,utils/transcript.d.ts,package.json}`.
  Acceptance criteria: `pi_offers_its_tools_on_every_turn` (read/edit/write, без bash; сегодня падает), `the_second_turn_carries_the_tool_result_with_its_id` (сегодня падает), `a_done_reason_that_contradicts_the_frames_is_an_error` (сегодня падает), `a_large_inbound_frame_is_accepted` (>64 КиБ, сегодня падает), `pi_executes_an_edit_the_model_asked_for` (регрессия; может быть зелёным сегодня); `cargo test -p stellwerk-task --test pi_transport` (≥7), `cargo test -p stellwerk-cli --test build_runner` (все 16, потому что Scripted теперь принимает tools); ручной canary Plan→Build с реальным `edit`.
  QA scenarios: happy — canary: модель вызывает `edit`, гейт разрешает R1, файл меняется, `ходов модели` > 1; failure — подменить `done.reason` на `stop` при наличии `tool_call`, поток обязан упасть. Evidence `.omo/evidence/pi-tools-channel/task-8-pi.txt`.
  Commit: Y | feat(00j6): let Pi carry the tool conversation

- [x] 9. Ход без tools отвергается (пакет 00j7)
  What to do / Must NOT do: `parse` требует ключ `tools`; `None` ⇒ `ErrorCode::Schema` с сообщением «turn has no `tools` field»; `Some([])` остаётся допустимым; `a_legacy_turn_is_forwarded_without_offer_validation` заменяется на `a_turn_without_tools_is_refused`; добавить `an_empty_tool_set_is_an_explicit_choice`. `TURN` получает **валидный `ToolDef` для bash** (`{"name":"bash","description":"shell","parameters":{"type":"object"}}`), потому что roundtrip-тест ждёт `tool_use`; для отказа — отдельный `NO_TOOLS` payload без ключа, для пустого — `EMPTY_TOOLS` с `"tools":[]`. В `tests/model_transport_wire.rs` у `Scripted` и `Failing` добавить `turn_request`: оба делегируют в `turn` (иначе `TURN` с `Some([bash])` падает на умолчании `BackendError("backend does not accept tools")`, и `a_turn_roundtrip...`/`a_backend_error...` не доходят до сценария). Добавить `"tools":[]` в at-limit и второй ход (`tests/model_transport_wire.rs:177,216-217`) и в `crates/stellwerk-cli/tests/host_model.rs` (`exchange`, `:49-58`). Route-тест (`:129-140`) не трогать: неизвестное поле проверяется до `tools`, поэтому `UnexpectedField` остаётся. Проверка лишнего поля идёт **до** требования `tools`. Схему сообщений не менять.
  Parallelization: Wave 7 | Blocked by: 8 | Blocks: F1-F4
  allowed_changes: crates/stellwerk-task/src/m5/model_transport.rs, crates/stellwerk-task/tests/model_transport_wire.rs, crates/stellwerk-cli/tests/host_model.rs
  References: `crates/stellwerk-task/src/m5/model_transport.rs:207-240`; `crates/stellwerk-task/tests/model_transport_wire.rs:12-32` (`Scripted`/`Failing`), `:92` (`TURN`), `:164,177,216-217`; `crates/stellwerk-cli/tests/host_model.rs:49-58`.
  Acceptance criteria: `a_turn_without_tools_is_refused` (`NO_TOOLS`); `an_empty_tool_set_is_an_explicit_choice` (`EMPTY_TOOLS` + text backend → Done `stop`); `an_unknown_field_is_still_refused_first`; `a_turn_roundtrip_streams_text_tool_and_done` по-прежнему `tool_use`; `a_backend_error_is_an_error_frame_not_done` по-прежнему `BackendError` (через явный `Failing::turn_request`); `cargo test -p stellwerk-task --test model_transport_wire`, `--test pi_transport`, `cargo test -p stellwerk-cli --test host_model`.
  QA scenarios: happy — три набора; failure — вернуть приём отсутствующего ключа, `a_turn_without_tools_is_refused` падает; вернуть. Evidence `.omo/evidence/pi-tools-channel/task-9-required-tools.txt`.
  Commit: Y | feat(00j7): refuse a turn that does not say which tools it offers

- [x] 10. Тестовый wire-хелпер `model_backend` объявляет пустой набор (пакет 00j7a)
  What to do / Must NOT do: в `crates/stellwerk-task/tests/model_backend.rs` хелпер `turn(messages)` (`:494-495`) должен формировать wire-кадр с `"tools":[]`, потому что 00j7 сделал ключ обязательным. `a_legacy_turn_sends_no_tools_key` (`:281`) не трогать: он проверяет `TurnRequest { tools: None }` на уровне backend, а не wire-хелпер. Не менять продовый код, `model_transport.rs`, `TurnRequest`/`TurnMessage`; не ослаблять тест, не добавлять `#[ignore]`/`#[allow]`; не трогать `model_transport_wire.rs`. Это test-only follow-up вне `00j7.allowed_changes` — отдельный пакет, чтобы не нарушить C1/≤3 файла.
  Parallelization: Wave 7a | Blocked by: 9 | Blocks: F1-F4
  allowed_changes: crates/stellwerk-task/tests/model_backend.rs
  References: `crates/stellwerk-task/tests/model_backend.rs:494-495`; `tool_calls_default_to_empty` (`:578-590`); `a_legacy_turn_sends_no_tools_key` (`:281`); evidence `task-9-required-tools.txt`.
  Acceptance criteria: `cargo test -p stellwerk-task --test model_backend` → 18/18; `cargo test -p stellwerk-task` зелёный; C2 src 0 / tests 1; scoped rustfmt/clippy без нового.
  QA scenarios: happy — `tool_calls_default_to_empty` зелёный, backend получает `tools:[]`; failure — убрать `tools` из хелпера, тест снова `Schema`; evidence `.omo/evidence/pi-tools-channel/task-9a-model-backend.txt`.
  Commit: Y | test(00j7a): send an explicit empty tool set in model_backend turns

## Final verification wave

> Runs in parallel after ALL todos. ALL must APPROVE. Surface results and wait for the user's explicit okay before declaring complete.

- [x] F1. Plan compliance audit — сверить дифф с `allowed_changes` каждого пакета: ни одного файла вне списка, ≤3 файла на пакет, C2 (src ≤200 логических, tests ≤250), нет `#[allow]`, `#[ignore]`, `cargo add`, правок `contract.rs`/корневого `Cargo.toml`. Канон-пакеты: 9 файлов, у каждого `id/depends_on/allowed_changes`, набор файлов совпадает с todo, RED-имена присутствуют. `plan-lint` не используется как доказательство (Appendix A).
- [x] F2. Code quality review — clippy `--all-targets --no-deps -D warnings` по трём крейтам, `cargo fmt --check`, отсутствие `unwrap()`/`expect()` в новом продовом коде (тесты — можно), ошибки типизированы, нет молчаливых `Deny`/`Allow` по умолчанию (AGENTS §6).
- [x] F3. Real manual QA — **scripted** canary Plan→Build на копии репозитория (без реальной модели, чтобы не нарушать zero-human-intervention): Scripted backend возвращает `edit`, гейт показывает R1 allow, файл меняется, `done.reason=tool_use`, `ходов модели` > 1, `--tools` без bash. Плюс legacy/чужой набор с bash → `Deny R5`, отличимо от `unknown_tool`.
- [x] F4. Scope fidelity — `git diff` не содержит `permission.rs`, `gate.rs`, `contract.rs`, корневого `Cargo.toml`, `plan-lint.py`, owner-WIP файлов; planner шлёт `[]`; tool-результаты не попали в evidence-цепочку; TS `authorize` не ослаблен.

## Commit strategy

Один коммит на пакет, сообщения выше. Тесты и реализация — вместе (TDD-цикл в одном коммите). Перед merge каждого пакета: сьютка на `main` (не только в worktree), `merge --no-ff`, `evidence_verify` на `main`, `gate_evaluate`, затем отдельный `chore(evidence): attest <packet> after merge` с `chain_tail`. Если гейт fail — пакет не считается выполненным, отчёт не пишется. Ветку не пушить. Перед 00j6 worktree перебазируется на смерженный `main` (INFRA-04).

## Success criteria

1. `cargo test -p stellwerk-task --test model_transport_wire`, `--test model_backend`, `--test pi_transport`, `--test gate_channel` — зелёные; `cargo test -p stellwerk-cli --test build_runner`, `--test planner_host`, `--test host_model`, `--test plan` — зелёные.
2. Scripted canary: модель в Build вызывает `edit`, гейт разрешает R1, файл в копии меняется; `ходов модели` больше одного; `гейт: решений` содержит allow R1.
3. `done.reason` равен `tool_use` при вызове и `stop` без вызова; противоречие — ошибка.
4. `bash` отсутствует в `--tools`; Build-расширение не может его вызвать; legacy/чужой набор, предлагающий bash, получает `Deny R5`, отличимый от `unknown_tool`.
5. `tools`: отсутствие ключа — `schema`; `[]` — явное «инструментов нет»; вызов вне `Some(list)` — `tool_not_offered` без кадров хода.
6. clippy `--no-deps -D warnings` и `cargo fmt --check` чисто; ни одного нового `#[allow]`/`#[ignore]`.
7. Каждый пакет: ≤3 файла, C2-бюджет соблюдён, evidence-коммит с `chain_tail` после merge.

## Appendix A — FINDING (вне скоупа, не чинить здесь)

1. **Канон N2 неполон**: 18 файлов, последний — `packet-00i1.md`; `00i2`–`00i7` отсутствуют при аттестованной работе.
2. **`just plan-lint` слеп**: `scripts/plan-lint.py` `parse()` (строки 69-111) требует первую строку `#` и голые `key: value`; канон использует `---` (`00i1`) или `**depends_on:**` (`00b`), поэтому линтер видит «пакетов нет» для всех milestone и падает на C7 (M5 `packet-05/06/REVIEW`), exit 2. Как гейт в этом прогоне не используется.
3. **Песочница планировщика**: запись в `.sisyphus/plans/**` запрещена хуком; канон-пакеты материализует шаг 1 воркера. Если владелец даст доступ — шаг 1 снимается и пакеты пишутся планировщиком; это устраняет риск расхождения плана и канона.

## Appendix B — Точные типы Pi 0.87.0 (проверено)

- `getCurrentTools(messages: TranscriptMessages): Tool[]`, `getCurrentSystemPrompt(messages): string` — `dist/utils/transcript.d.ts:26,34`.
- `Tool { name: string; description: string; parameters: TSchema }` — `dist/types.d.ts:423-428`.
- `ToolCall { type:"toolCall"; id: string; name: string; arguments: JsonObject }` — `dist/types.d.ts:261-269`.
- `ToolResultMessage { role:"toolResult"; toolCallId; toolName; content: (TextContent|ImageContent)[]; details?; usage?; isError: boolean; timestamp }` — `dist/types.d.ts:376-386`.
- `TranscriptContext { messages: Message[] }` — `dist/types.d.ts:450-453`.
- `StopReason` включает `"toolUse"` — `dist/types.d.ts:292`.
