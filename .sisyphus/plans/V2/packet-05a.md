---
id: V2/05a
task_id: V2-05a
milestone: V2
status: proposed
depends_on: [V2/01, 01, N1/01, N2/00h1, N2/00h2a, N2/00h3b, V1/04, V1/r01, V1/r04]
files_allowed: [crates/stellwerk-cli/src/host_model.rs, crates/stellwerk-cli/src/lib.rs, crates/stellwerk-cli/tests/run.rs]
max_files: 3
max_diff_lines: 200
acceptance: cargo test -p stellwerk-cli --test run
---

# V2/05a — F-05: wire the model and gate channels into run_task

## Цель
Цель: V2-05a F-05: wire the model and gate channels into run_task

## Шаг 1 — падающий тест
RED: новый тест с локальным `TcpListener` и fake Pi через `$STELLWERK_MODEL_SOCK`/`$STELLWERK_GATE_SOCK` падает до проброса каналов; приёмка — один model turn и один gate request, `verdict=Allow`.

## Что делать
split `run_task` into a thin production wrapper and a testable seam: `pub fn run_task(path: &Path) -> Result<RunResult, String>` resolves `let config = ModelConfig::from_env(HOST_BASE_URL, HOST_MODEL_ID).map_err(|e| e.to_string())?;`, `let pi = pi_program()?;`, `let extension = pi_extension()?;`, then calls `pub fn run_task_with(path: &Path, config: ModelConfig, pi: &Path, extension: &Path) -> Result<RunResult, String>`; move the existing body (load, ledger, workspace, worker, gate) into `run_task_with` and remove the inner `pi_program()`/`pi_extension()` calls so the seam uses the passed `pi` and `extension`; keep `ModelConfig::with_token_path` for tests only; do not add `from_parts`; do not invent `NEURALDEEP_BASE_URL`. Add `pub fn serve_until_stopped(&self, stop: &AtomicBool) -> std::io::Result<usize>` to `HostModel` (loop `self.serve_once()` while `!stop`; `serve_once` exists at `host_model.rs:141`). In `crates/stellwerk-cli/src/lib.rs`, define `const CLI_GATE_MODULE: &str = "stellwerk-cli";`, `const CLI_GATE_TOOLS: &[ToolEntry] = &[ToolEntry { module: CLI_GATE_MODULE, tool: "read", class: RiskClass::R0 }, ToolEntry { module: CLI_GATE_MODULE, tool: "edit", class: RiskClass::R1 }, ToolEntry { module: CLI_GATE_MODULE, tool: "write", class: RiskClass::R1 }, ToolEntry { module: CLI_GATE_MODULE, tool: "bash", class: RiskClass::R5 }]`, `const CLI_GATE_OFFERED: &[&str] = &["read", "edit", "write"];`, and `const CLI_GATE_HELD: &[&str] = &["read", "bash", "edit", "write"];`; keep `CLI_AGENT_TOOLS` only for the worker registry. In `run_task_with`: create a `sock` directory **outside** `work` and `tmp` (for example under the run root, sibling of `work`); build `HostModel::prepare(config, &sock, &visible)` with `visible = &[work.clone()]` and `sock` created by `unique_temp_path("sock", "")` outside `work`; bind `GateChannel` in the same `sock`; bind `let held = SessionDescription::new(&spec.id, &work, ToolSet::listed(CLI_GATE_HELD).map_err(|e| e.to_string())?, window, CodemodeState::AbsentAfterConfigLoad).map_err(|e| e.to_string())?;`; build `ModelBinding` with the exact literal already at `lib.rs:178-187` (`id: spec.model_ref.clone()`, `provider: "local".to_owned()`, `model_id: "pi".to_owned()`, `data_class: spec.limits.data_class`, `retention: RetentionClass::FirstParty`, `context_window: 1_000`, `max_output: 1_000`, `budget_tokens_per_task: 1_000`), bind it as `let model = ...`, and pass `model.clone()` to both the `Call` and `CliPolicy`; do not call the private `build_binding()`; construct `SandboxWorker::new(FixtureEngine::new(pi.to_path_buf()), sandbox, pi.to_path_buf(), args.clone(), window, env).with_sockets(host.socket(), gate.socket()).map_err(|e| e.to_string())?`; add `STELLWERK_MODEL_SOCK`/`STELLWERK_GATE_SOCK` to the explicit env; run `host.serve_until_stopped(&stop)` and `serve_gate_until_stopped_logged(&gate, &held, &policy, &stop, &work)` in `std::thread::scope` threads (the gate function is synchronous and borrows `&dyn GatePolicy`); after `execute`, set `stop`, `wake` both sockets, and join both threads. Add `pub struct CliPolicy { pub model: ModelBinding }` implementing `GatePolicy`: `request_for` finds `CLI_GATE_TOOLS` by `tool` and `module == CLI_GATE_MODULE`, maps its class to `ToolRisk::Classified` or `ToolRisk::Unclassified` when not found, and always returns `Some(CallRequest { tool, module: CLI_GATE_MODULE.to_owned(), risk, caller: PolicyLevel::L2Contributor, data: DataClass::Public, model: self.model.clone(), interactive: false })`; `decide_call` returns `PolicyVerdict::Evaluated { request, basis: AllowBasis::RiskTable }` for every `request_for` result; `request_for` always returns `Some`, so an unknown tool gets `ToolRisk::Unclassified` and still has a named request, never `None`/`PolicyVerdict::Unclassified`. Do not reuse `BuildPolicy`, which classifies `BUILD_TOOLS`/`BUILD_MODULE`, and do not use `CLI_AGENT_TOOLS` for gate authorization. Keep `run_task_with`'s existing `.with_registry(StaticRegistry(CLI_AGENT_TOOLS))` at line 169; change `pi_transport_args(&extension, &[])` to `pi_transport_args(&extension, CLI_GATE_OFFERED)`. Map every non-`String` error with `.map_err(|e| e.to_string())?`, including `HostModelError`, `WorkerError`, `IdentityError`, and `io::Error`; they do not convert to `String`. The integration test calls `run_task_with(path, ModelConfig::with_token_path("http://127.0.0.1:<port>", HOST_MODEL_ID, token_path).map_err(|e| e.to_string())?, &fake_pi, &pi_extension())` directly, plus a local `TcpListener` HTTP fixture (pattern `tests/host_model.rs:75-97`), and creates a temporary fake Pi script that connects to `$STELLWERK_MODEL_SOCK` and `$STELLWERK_GATE_SOCK`, sends one model frame and one `read` gate frame, then exits (reuse frame shapes from `tests/host_model.rs:75-97` and `crates/stellwerk-task/src/m5/gate_channel.rs`, not a build-runner fixture); update `run_cli_with` to create `<home>/.config/stellwerk/neuraldeep.token` with a dummy token for every CLI-based test and remove `XDG_CONFIG_HOME` so `ModelConfig::from_env` uses the HOME path, so `HostModel::prepare` reaches the fake Pi and no real network request is made; `examples/pi-fixture.sh` only prints Pi events and is not sufficient for the socket proof. Must NOT add a fake/no-channel fallback. Must NOT add a second authorization path. Must NOT use `#[ignore]`.

## Приёмка
`cargo test -p stellwerk-cli --test run`; a new test uses `ModelConfig::with_token_path` + a local `TcpListener` HTTP fixture, proves one model turn and one gate request through the sockets, and the run returns `verdict=Allow`; a missing model config returns a named `Unconfigured`/`MissingModelConfig` error, not a fake run.

## QA
happy - `cargo test -p stellwerk-cli --test run` with the fixture; failure - omit the model config and assert the named refusal. Evidence `.omo/evidence/task-6-stellwerk-production-v2.md`.

## Запрещено
- do not add `from_parts`
- do not invent `NEURALDEEP_BASE_URL`.
- do not call the private `build_binding()`
- Do not reuse `BuildPolicy`, which classifies `BUILD_TOOLS`/`BUILD_MODULE`, and do not use `CLI_AGENT_TOOLS` for gate authorization.
- they do not convert to `String`.
- Must NOT add a fake/no-channel fallback.
- Must NOT add a second authorization path.
- Must NOT use `#[ignore]`.

## Ссылки
`crates/stellwerk-cli/src/lib.rs:120-260` (`run_task`), `:33-37` (`CLI_AGENT_TOOLS`), `:125-127` (ledger), `:169` (registry); `crates/stellwerk-cli/src/build_runner.rs:789-855` (the proven channel wiring); `crates/stellwerk-cli/src/build_runner.rs:270-390` (`BuildPolicy`, `serve_gate_until_stopped_logged`, `serve_model_until_stopped`, `wake`), `:361` (gate signature), `:731` (`build_binding` is private); `crates/stellwerk-cli/src/host_model.rs:50-54` (`ModelConfig`), `:59-75` (`with_token_path`), `:79-88` (`from_env`), `:116` (`prepare`); `crates/stellwerk-cli/src/plan.rs:51-52` (`HOST_BASE_URL`/`HOST_MODEL_ID`); `crates/stellwerk-task/src/orchestrator.rs:370-386` (`with_sockets` rejects sockets under `work`); `crates/stellwerk-task/src/m5/session_identity.rs:76` (`ToolSet::listed` returns `Result`); `crates/stellwerk-cli/tests/run.rs`; `crates/stellwerk-cli/tests/host_model.rs:75-97`.
