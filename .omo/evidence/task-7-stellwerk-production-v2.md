# Task 7 — V2-05b F-05: run against a workspace copy and prove real Pi 1.0.2

Commit: `e47311a80f5f4733cff2d77adbbea0df26466693` — `feat(cli): run against a workspace copy`
Files (exactly 3): `crates/stellwerk-cli/src/lib.rs`,
`crates/stellwerk-cli/tests/run.rs`, `crates/stellwerk-cli/examples/n1-smoke.toml`.

## RED — tests written before the seam existed

Command:
```
cargo test -p stellwerk-cli --test run
```

Output (verbatim excerpts):
```
running 13 tests
...
test missing_workspace_is_named_refusal ... FAILED
test run_against_workspace_copy_sees_files ... FAILED
test run_task_with_real_pi ... FAILED

---- missing_workspace_is_named_refusal stdout ----
thread 'missing_workspace_is_named_refusal' panicked at crates/stellwerk-cli/tests/run.rs:822:18:
a missing workspace must refuse, not run

---- run_against_workspace_copy_sees_files stdout ----
thread 'run_against_workspace_copy_sees_files' panicked at crates/stellwerk-cli/tests/run.rs:794:5:
assertion `left == right` failed: workspace copy must carry files and exclude .git
  left: "SAW: GIT:absent"
 right: "SAW:workspace-copy-/tmp/stellwerk-r04-workspace-copy-469941-4 GIT:absent"

---- run_task_with_real_pi stdout ----
real Pi /home/izyabretatel/.local/bin/pi -> 1.0.2
thread 'run_task_with_real_pi' panicked at crates/stellwerk-cli/tests/run.rs:1023:5:
the second turn carries the read result: ... <tool_error>read_failed:ENOENT:
no such file or directory, open 'readme.txt'</tool_error> ...

test result: FAILED. 10 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out
```

Right reason: before the change `work` is empty (no `readme.txt`), a missing
`workspace_ref` silently ran, and the real Pi's `read` failed with `ENOENT`
because the agent had no copied tree. No fallback/fixture path was added to make
them green.

## GREEN — after `resolve_workspace` + `copy_workspace`

Command:
```
cargo test -p stellwerk-cli --test run
```

Output (verbatim):
```
running 13 tests
test failed_run_is_not_reported_as_success ... ok
test missing_task_file_exits_nonzero ... ok
test run_task_without_model_config_refuses_by_name ... ok
test run_without_git_head_refuses ... ok
test run_attestation_reads_linked_worktree_commit ... ok
test missing_engine_is_named_refusal ... ok
test missing_workspace_is_named_refusal ... ok
test run_against_workspace_copy_sees_files ... ok
test run_writes_receipt_to_sqlite ... ok
test run_attestation_carries_bindings ... ok
test both_entrypoints_share_one_authorization ... ok
test run_task_with_drives_one_model_turn_and_one_gate_request ... ok
test run_task_with_real_pi ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.03s
```

New tests:
- `run_against_workspace_copy_sees_files` — fixture compares `readme.txt` in
  `work` with a per-test marker and exits 7 if it is absent/mismatched, 8 if
  `.git` leaked into `work`; a valid run requires both checks to pass.
- `missing_workspace_is_named_refusal` — absent absolute `workspace_ref` returns
  an error containing `MissingWorkspace` and the resolved path.
- `run_task_with_real_pi` — calls `run_task_with` directly (not `run_cli_with`)
  with a local `TcpListener` fixture and the real Pi from `PI_BIN` or `PATH`.

## Real Pi 1.0.2

```
$ pi --version
1.0.2
```

```
cargo test -p stellwerk-cli --test run run_task_with_real_pi -- --nocapture
running 1 test
real Pi /home/izyabretatel/.local/bin/pi -> 1.0.2
test run_task_with_real_pi ... ok

test result: ok. 1 passed; 0 failed; 12 filtered out
```

The test resolves `PI_BIN` when set, otherwise `pi` on `PATH`; absence is a
panic with the exact path, never a skip. `pi_transport_args` supplies only the
transport argv and stdin is `Stdio::null()`, so the test builds a managed-layout
copy under its scratch dir (launcher copy + `install` symlink) and a wrapper
that appends `--provider stellwerk --model proxy --mode json -p go`. The version
assertion runs against the same binary that the run executes.

## Gates

```
cargo clippy -p stellwerk-cli --all-targets -- -D warnings   -> exit 0
cargo fmt --all -- --check                                   -> exit 0
```

## Commit / diff

`git show --numstat`:
```
e47311a80f5f4733cff2d77adbbea0df26466693 feat(cli): run against a workspace copy

1	1	crates/stellwerk-cli/examples/n1-smoke.toml
38	1	crates/stellwerk-cli/src/lib.rs
219	4	crates/stellwerk-cli/tests/run.rs
```

## C2 (`scripts/plan-lint.py::added_counts` on the same diff)

```
src 31  (limit 200)
tests 219 (limit 250)
budget_violations []
```

`resolve_workspace` uses `path.parent()` (never process cwd), takes absolute
paths as-is, and returns `MissingWorkspace: ...` for a non-directory. The copy
happens after the directories are created and before the worker/sandbox is
built; `copy_workspace` retains its existing public signature and rules (no
`.git`, `target`, or `.omo`). No `unwrap`/`expect`/`#[allow]`/`#[ignore]` was
added in `src`.

## PolyForge chain (on main at e47311a)

```
evidence_append(ModelClaim) task=V2/packet-05b   -> seq 387 hash a024f0db...
evidence_verify(tool="cargo test", args=[-p stellwerk-cli --test run])
                                                 -> seq 388 Verified, exit_code 0,
                                                    stdout_hash 1ee0e101...
gate_evaluate(task=V2/packet-05b, required=[Verified])
                                                 -> passed=true verified=1 missing=[]
chain_tail_hash: 142f43c7e9e6bc1fe157d64dea0763afed3e76abf709ebe458852f01613abf0a
```
