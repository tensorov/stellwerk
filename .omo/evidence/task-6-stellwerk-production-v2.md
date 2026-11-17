# Task 6 — V2-05a F-05: wire the model and gate channels into run_task

Commit: `01aca4d5ef4f0b9216bb22ae00e03733f9405b2c` — `feat(cli): wire model and gate channels into run`
Files (exactly 3): `crates/stellwerk-cli/src/host_model.rs`,
`crates/stellwerk-cli/src/lib.rs`, `crates/stellwerk-cli/tests/run.rs`.

## RED — test written before the seam existed

Command:
```
cargo test -p stellwerk-cli --test run
```

Output (verbatim):
```
   Compiling stellwerk-cli v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-cli)
error[E0432]: unresolved import `stellwerk_cli::run_task_with`
  --> crates/stellwerk-cli/tests/run.rs:16:5
   |
16 | use stellwerk_cli::run_task_with;
   |     ^^^^^^^^^^^^^^-------------
   |                    |
   |                    no `run_task_with` in the root

error: could not compile `stellwerk-cli` (test "run") due to 1 previous error
```
Right reason: the new test needs the seam (`run_task_with`), the model
`serve_until_stopped`, and the `RunResult` counters before it can exercise the
sockets. No fallback/fixture path was added to turn it green.

## GREEN — after `run_task_with`, `HostModel::serve_until_stopped`, `CliPolicy`

Command:
```
cargo test -p stellwerk-cli --test run
```

Output (verbatim):
```
running 10 tests
test failed_run_is_not_reported_as_success ... ok
test missing_task_file_exits_nonzero ... ok
test run_task_without_model_config_refuses_by_name ... ok
test run_without_git_head_refuses ... ok
test run_attestation_carries_bindings ... ok
test run_attestation_reads_linked_worktree_commit ... ok
test missing_engine_is_named_refusal ... ok
test run_writes_receipt_to_sqlite ... ok
test both_entrypoints_share_one_authorization ... ok
test run_task_with_drives_one_model_turn_and_one_gate_request ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
```

New test names:
- `run_task_with_drives_one_model_turn_and_one_gate_request`
- `run_task_without_model_config_refuses_by_name`

What the socket test proves: a local `TcpListener` HTTP fixture counts exactly
one backend request; `RunResult.model_turns == 1`; `RunResult.gate_requests == 1`;
the fake Pi exits 3 unless the gate answered `allow`, and the run returns
`verdict == "Allow"`. `run_task_without_model_config_refuses_by_name` removes
`HOME`/`XDG_CONFIG_HOME` and asserts the named refusal
`model setting ... is not configured`, with no `verdict=` on stdout.

## Adjacent suites

```
cargo test -p stellwerk-cli --test host_model   -> 5 passed; 0 failed
cargo test -p stellwerk-cli --test plan         -> 41 passed; 0 failed
```

## Gates

```
cargo clippy -p stellwerk-cli --all-targets -- -D warnings   -> CLIPPY_EXIT=0
cargo fmt --all -- --check                                   -> FMT_EXIT=0
```

## Commit / diff

`git show --numstat`:
```
01aca4d5ef4f0b9216bb22ae00e03733f9405b2c feat(cli): wire model and gate channels into run

17	0	crates/stellwerk-cli/src/host_model.rs
174	24	crates/stellwerk-cli/src/lib.rs
182	2	crates/stellwerk-cli/tests/run.rs
```

## C2 (`scripts/plan-lint.py::added_counts` on the commit)

```
src 157  (limit 200)
tests 182 (limit 250)
budget_violations []
```

Note: raw diff is 373 added lines; C2 counts logical src 157 / full tests 182,
both under budget. No `unwrap`/`expect`/`#[allow]`/`#[ignore]` added in `src`.
`run_task` is now a thin wrapper; the channel wiring lives in `run_task_with`;
`CLI_AGENT_TOOLS` stays the worker registry only, and `CliPolicy` is the sole
gate authorization path for inner tools.

## PolyForge chain (on main at 01aca4d)

```
evidence_append(ModelClaim) task=V2/packet-05a  -> seq 385 hash 22a47ed7...
evidence_verify(tool="cargo test", args=[-p stellwerk-cli --test run])
                                                -> seq 386 Verified, exit_code 0,
                                                   stdout_hash 6b18e09a...
gate_evaluate(task=V2/packet-05a, required=[Verified])
                                                -> passed=true verified=1 missing=[]
chain_tail_hash: a45461067e02a6e31962f46df539bfea64c400a51737dfb8a9c79e04f5afb252
```
