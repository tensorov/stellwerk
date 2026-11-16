---
id: V2/01
task_id: V2-01
milestone: V2
status: proposed
depends_on: [N1/01, N1/10, N2/00h1, N2/00h2a, N2/00h2b, N2/00h3b, TUI/02, TUI/03, V1/04, V1/r01, V1/r04]
files_allowed: [crates/stellwerk-cli/src/lib.rs, crates/stellwerk-cli/src/main.rs, crates/stellwerk-cli/tests/run.rs]
max_files: 3
max_diff_lines: 200
acceptance: cargo test -p stellwerk-cli --test run
---

# V2/01 — F-02: bind the production ToolAttestation to recipe, policy, and commit

## Цель
Цель: V2-01 F-02: bind the production ToolAttestation to recipe, policy, and commit

## Шаг 1 — падающий тест
RED: тест `run_without_git_head_refuses` падает до правки; проверка — `cargo test -p stellwerk-cli --test run run_without_git_head_refuses`. После правки — GREEN.

## Что делать
keep `tool_attestation` in `crates/stellwerk-task/src/evidence.rs` for fixtures; add a private `bound_attestation(...)` helper in `crates/stellwerk-cli/src/lib.rs` that calls `tool_attestation(...)` and then fills `recipe_digest`, `policy_digest`, and `commit_sha` with `Some(...)`; do not construct `EvidenceEntry` literally, so the constructor-mint count stays 3. In `run_task` (the body Task 6 will later move into `run_task_with`), after the ledger is opened (line 125), resolve the commit binding before any workspace resolution or worker construction, add a local `sha256_hex` helper using `sha2::{Digest, Sha256}` (already a dependency) and compute `recipe_digest = sha256_hex(program + "\0" + args.join("\0"))`; `policy_digest = sha256_hex("stellwerk-cli|pi|R0|L2Contributor")`; `commit_sha` via `read_commit_sha(root)` where `root` is the nearest ancestor of `std::env::current_dir()` that contains `.git` (walk up, do not require cwd itself). Resolve the git directory: if `<root>/.git` is a directory, use it; if it is a file, read its single `gitdir: <path>` line and resolve a relative path against `root`; if neither is a directory, refuse. Read `<gitdir>/HEAD`; for `ref: refs/heads/...`, choose the ref root: if `<gitdir>/commondir` exists, read its single line and resolve it relative to `<gitdir>` (linked worktree), otherwise use `<gitdir>`. Read `<refroot>/<ref>`; if that ref file is absent, read `<refroot>/packed-refs` and take the 40-hex value on the line ending in ` <ref>`; accept a detached 40-hex `HEAD` directly. Add a test with a synthetic linked-worktree layout (`<root>/.git` file -> `.git/worktrees/<name>`, `<gitdir>/commondir` -> `../..`, ref under the common dir) so this path is locked without invoking `git worktree`. Accept only 40 lowercase hex. If the git dir, `HEAD`, or the resolved ref is missing/invalid, print `ledger=<path>` to stderr and return `Err("MissingAttestationBinding: commit_sha ...")` before appending anything. Add `pub ledger_path: PathBuf` to `RunResult` and print `ledger=` in `crates/stellwerk-cli/src/main.rs` on the success path; on every early-refusal path after the ledger is opened, print `ledger=<path>` to stderr before returning (do not rely on the final `match` arm), so the integration test can read the ledger even when the run refuses. Use `sha2` (already in `stellwerk-cli`). `model_fingerprint` may stay `None`; the non-`None` requirement applies to `recipe_digest`, `policy_digest`, and `commit_sha`. Must NOT call `evaluate` in production; `evaluate` requires a human `Validation`. Must NOT invent a commit SHA. Must NOT use `workspace_ref` as the commit source: the attestation binds the CLI's control-plane revision, and `workspace_ref` is not guaranteed to be a git repository.

## Приёмка
`cargo test -p stellwerk-cli --test run`; new test `run_attestation_carries_bindings` runs the CLI from the repo root, parses `ledger=`, opens `SqliteEvidenceLedger`, and asserts `recipe_digest.is_some()`, `policy_digest.is_some()`, `commit_sha.is_some()`; new test `run_without_git_head_refuses` uses a new `run_cli_with_cwd` helper that writes a dummy `<scratch>/.config/stellwerk/neuraldeep.token` before spawning, removes `XDG_CONFIG_HOME` so `ModelConfig::from_env` resolves that path under `HOME`, and runs the CLI in a scratch dir with no `.git` ancestor, parses the error-path `ledger=` from `stderr`, opens the ledger, asserts the named `MissingAttestationBinding` error, and asserts no `ToolAttestation` was appended; the test sets `PI_BIN` and `STELLWERK_PI_EXTENSION` to the fixture so `pi_program()` succeeds and the refusal is the git binding, not a missing engine. `cargo test -p stellwerk-task --test evidence` stays green.

## QA
happy - `cargo test -p stellwerk-cli --test run run_attestation_carries_bindings`; failure - `cargo test -p stellwerk-cli --test run run_without_git_head_refuses` and inspect the ledger via `SqliteEvidenceLedger::entries`. Evidence `.omo/evidence/task-2-stellwerk-production-v2.md`.

## Запрещено
- do not construct `EvidenceEntry` literally, so the constructor-mint count stays 3.
- `commit_sha` via `read_commit_sha(root)` where `root` is the nearest ancestor of `std::env::current_dir()` that contains `.git` (walk up, do not require cwd itself).
- on every early-refusal path after the ledger is opened, print `ledger=<path>` to stderr before returning (do not rely on the final `match` arm), so the integration test can read the ledger even when the run refuses.
- Must NOT call `evaluate` in production
- Must NOT invent a commit SHA.
- Must NOT use `workspace_ref` as the commit source: the attestation binds the CLI's control-plane revision, and `workspace_ref` is not guaranteed to be a git repository.

## Ссылки
`crates/stellwerk-cli/src/lib.rs:90-96` (`RunResult`); `crates/stellwerk-cli/src/lib.rs:210-250` (`tool_attestation` call); `crates/stellwerk-task/src/evidence.rs:86-105` (`tool_attestation`); `crates/stellwerk-task/src/evidence.rs:284` (`evaluate` requires the three bindings); `crates/stellwerk-core/src/contract.rs:489-538` (`EvidenceEntry`, `RecipeDigest`, `PolicyDigest`, `CommitSha`); `crates/stellwerk-cli/src/main.rs:54-66` (`RunResult` printing); `crates/stellwerk-cli/tests/run.rs:74-102` (`run_cli_with`, `run_cli`); `crates/stellwerk-cli/Cargo.toml` (`sha2` already present).
