# Todo 8 / V2-06 (F-07) — route execution admission through `execution::resolve`

- Repo: `/home/izyabretatel/gits/stellwerk`, branch `main`, direct commit (no worktree/PR/push).
- Commit: `d8bdd8ada4ba9674f7f819f66b8562f53567769b`
  `refactor(task): route execution admission through resolve`
- Files (exactly 3):
  - `crates/stellwerk-task/src/taskfile.rs`
  - `crates/stellwerk-cli/src/lib.rs`
  - `crates/stellwerk-task/tests/taskfile.rs`

## RED (before implementation)

Command: `cargo test -p stellwerk-task --test taskfile`
Exit: 101. The new tests fail to compile because the zero-arg `resolve` still exists,
`ResolvedExecution` methods are unavailable on the old return type, and
`TaskFileError::EmptyProgram` does not exist yet.

```
   Compiling stellwerk-task v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-task)
error[E0061]: this method takes 0 arguments but 1 argument was supplied
   --> crates/stellwerk-task/tests/taskfile.rs:196:10
    |
196 |         .resolve("/opt/pi/bin/pi")
    |          ^^^^^^^ ---------------- unexpected argument of type `&'static str`
    |
note: method defined here
   --> crates/stellwerk-task/src/taskfile.rs:119:12
    |
119 |     pub fn resolve(&self) -> Result<&'static EngineProfile, TaskFileError> {
    |            ^^^^^^^
help: remove the extra argument
```

```
error[E0599]: no method named `program` found for reference `&EngineProfile` in the current scope
   --> crates/stellwerk-task/tests/taskfile.rs:198:25
    |
198 |     assert_eq!(resolved.program(), "/opt/pi/bin/pi");
    |                         ^^^^^^^ method not found in `&EngineProfile`
```

```
error[E0599]: no variant, associated function, or constant named `EmptyProgram` found for enum `TaskFileError` in the current scope
   --> crates/stellwerk-task/tests/taskfile.rs:212:46
    |
212 |             matches!(refusal, TaskFileError::EmptyProgram),
    |                                              ^^^^^^^^^^^^ variant, associated function, or constant not found in `TaskFileError`
```

`error: could not compile 'stellwerk-task' (test "taskfile") due to 6 previous errors`
(`cargo test ... > /tmp/red-taskfile.txt 2>&1; exit=101`; 7 lines starting with `error`).

## GREEN (after implementation)

Command: `cargo test -p stellwerk-task --test taskfile --test execution`
Exit: 0.

```
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/taskfile.rs (target/debug/deps/taskfile-6a7b42fff7b134e3)

running 8 tests
test resolve_refuses_a_blank_program_as_empty_program ... ok
test resolve_returns_the_named_program_and_the_profile_engine ... ok
test resolve_keeps_missing_and_unknown_profile_distinct ... ok
test test_taskfile_from_toml_parses_valid_fixture ... ok
test test_taskfile_from_toml_rejects_unknown_key ... ok
test test_taskfile_from_toml_rejects_negative_limits ... ok
test test_taskfile_into_spec_converts_id_and_goal ... ok
test profile_must_be_explicit_pi ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Command: `cargo test -p stellwerk-cli --test run`
Exit: 0, 13 passed (including `run_task_with_real_pi`).

```
running 13 tests
test missing_task_file_exits_nonzero ... ok
test run_task_without_model_config_refuses_by_name ... ok
test failed_run_is_not_reported_as_success ... ok
test missing_workspace_is_named_refusal ... ok
test run_without_git_head_refuses ... ok
test run_attestation_carries_bindings ... ok
test run_against_workspace_copy_sees_files ... ok
test missing_engine_is_named_refusal ... ok
test run_attestation_reads_linked_worktree_commit ... ok
test run_writes_receipt_to_sqlite ... ok
test both_entrypoints_share_one_authorization ... ok
test run_task_with_drives_one_model_turn_and_one_gate_request ... ok
test run_task_with_real_pi ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.86s
```

Command: `cargo clippy --workspace --all-targets -- -D warnings` — exit 0.
Command: `cargo fmt --all -- --check` — exit 0.

`execution.rs` is untouched; `cargo test -p stellwerk-task --test execution` stays green
(12 passed, see above).

## Live call sites

```
$ rg -n "execution::resolve" crates/stellwerk-task/src/taskfile.rs
121:    /// `execution::resolve`; второй публичный путь разрешения расходился бы с ним.
141:    /// `execution::resolve` — единственную точку допуска. Программа берётся из
151:        crate::execution::resolve(Some(&spec), &REGISTRY).map_err(|refusal| match refusal {

$ rg -n "task_file\.resolve|resolved\.program" crates/stellwerk-cli/src/lib.rs
449:    let resolved = task_file.resolve(program);
534:        resolved.program(),
613:            let command = format!("{} {}", resolved.program(), args.join(" "));
```

`TaskFile::resolve` has no zero-arg form; `resolve_profile` is private and is reached only
from `validate()` (`self.resolve_profile()?`).

## C2 (authoritative, `scripts/plan-lint.py::added_counts`, after commit)

```
$ python3 -c "import importlib.util,subprocess; s=importlib.util.spec_from_file_location('pl','scripts/plan-lint.py'); m=importlib.util.module_from_spec(s); s.loader.exec_module(m); print(m.added_counts(subprocess.run(['git','show','--format=','d8bdd8ada4ba9674f7f819f66b8562f53567769b'],capture_output=True,text=True).stdout))"
(35, 58)
```

src 35 / 200, tests 58 / 250 — within budget.

## `Absent` decision

`execution::resolve` also returns `ExecutionRefusal::Absent`. `TaskFile::resolve` always
passes `Some(&spec)`, so `Absent` is unreachable by construction. It is handled with an
explicit visible arm mapped to `TaskFileError::Validation("execution spec absent: ...")`,
plus a code comment stating why it cannot occur. It is NOT mapped to `MissingProfile`, no
`AbsentExecutionSpec` variant was invented (the plan forbids it), and no
`unreachable!()`/`panic!`/`unwrap()`/wildcard arm was used. `execution.rs` was not edited.

## Admission semantics preserved

- named-but-wrong profile → `TaskFileError::UnknownProfile { profile, known: ["pi"] }`
  through both `from_toml_str` (validate) and the new `resolve(program)`;
- unnamed profile → `TaskFileError::MissingProfile`;
- blank / whitespace program (`""`, `"   "`) → `TaskFileError::EmptyProgram`;
- valid TOML + program → `ResolvedExecution` with `.program() == input` and `.engine() == "pi"`.

## Dirty baseline

`git status` shows the accepted unstaged baseline (`.omo/omo.jsonc`, `opencode.json`,
`tests/test_omo_config.py`, `.omo/archive/`, `.omo/drafts/`, other `.omo/evidence/`,
`.playwright-mcp/`, `mutants.out*`). Only the 3 declared files were staged explicitly.
This evidence file is committed by `chore(evidence): attest V2/packet-06 after merge`
(the separate evidence-commit convention).

## PolyForge chain (on main at d8bdd8a)

```
evidence_append(ModelClaim) task=V2/packet-06   -> seq 389 hash 6c9f6a94...
evidence_verify(tool="cargo test", args=[-p stellwerk-task --test taskfile --test execution])
                                                -> seq 390 Verified, exit_code 0,
                                                   stdout_hash 4a3fdac4...
gate_evaluate(task=V2/packet-06, required=[Verified])
                                                -> passed=true verified=1 missing=[]
chain_tail_hash: 5e4313bcac8362d905a2259179412b43c315614d7ab4a41bdc455796eda59d00
```

## Orchestrator re-verification (independent of the worker)

- `git show --numstat d8bdd8a` → exactly 3 files: `lib.rs 10/4`, `taskfile.rs 41/4`,
  `tests/taskfile.rs 58/0`.
- `cargo test -p stellwerk-task --test taskfile` → 8 passed;
  `--test execution` → 12 passed (untouched).
- `cargo test -p stellwerk-cli --test run` → 13 passed, 0 failed, 0 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings` → exit 0;
  `cargo fmt --all -- --check` → exit 0.
- `added_counts d8bdd8a` → (35, 58), within the C2 limits.
- `rg`: `execution::resolve` at `taskfile.rs:151`, `task_file.resolve(program)` at
  `lib.rs:451`, `resolved.program()` at `lib.rs:533` and `:612`;
  `fn resolve_profile` private at `taskfile.rs:128`, `pub fn resolve` at `:145`.
- `git status --porcelain` (tracked) → only the accepted baseline
  (`.omo/omo.jsonc`, `opencode.json`, `tests/test_omo_config.py`); no scope creep.
