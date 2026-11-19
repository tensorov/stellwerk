# F3 — Real manual QA (stellwerk-production-v2)

- Timestamp: `2026-11-19T01:28:08+01:00`
- Repo: `/home/izyabretatel/gits/stellwerk`
- Branch / HEAD: `main` / `5d7e50b chore(evidence): attest V2/packet-14 after merge`
- Reviewer role: read-only QA. No source, test, helper, plan, packet, ledger, boulder, or notepad file was modified. The only file written is this evidence file. No commit was made. PolyForge was not called.

## Pi binary

```
$ /home/izyabretatel/.pi/agent/bin/pi --version
1.0.2
```

The Pi-backed test was run with `PI_BIN=/home/izyabretatel/.pi/agent/bin/pi`, i.e. the canonical managed path (not `/home/izyabretatel/.local/bin/pi`, whose lexical parent lacks a sibling `install/`). `/home/izyabretatel/.local/bin/pi` is a symlink to `../../.pi/agent/bin/pi`.

FINDING `F-2026-11-19-01` was **not re-triggered**: the canonical path was used from the first attempt, so no `bwrap: Can't find source path .../install` failure occurred. The finding is not re-opened here and is not treated as a product regression.

## Non-removing helper proof

`crates/stellwerk-cli/tests/run.rs` selects the real Pi through `real_pi()`, which honors `PI_BIN`:

```rust
fn real_pi() -> PathBuf {
    if let Some(value) = std::env::var_os("PI_BIN") {
        let path = PathBuf::from(value);
        assert!(path.is_file(), "real Pi missing: {}", path.display());
        return path;
    }
```

The test then builds a sandbox layout with `pi_layout()` instead of mutating the installed Pi. The line that proves the helper is non-removing:

```rust
std::os::unix::fs::symlink(agent.join("install"), layout.join("install"))
    .expect("install link");
```

Why this is non-removing: `pi_layout` copies the launcher into a fresh `scratch_dir` (`std::fs::copy(&managed, &launcher)`), and *links* the real `agent/install` into that scratch layout. It never moves, rewrites, or deletes the installed Pi tree under `~/.pi/agent`; the real `install/` is only read through the symlink. The only cleanup is the test's own scratch directory:

```rust
let _ = std::fs::remove_file(&result.store_path);
let _ = std::fs::remove_file(&result.ledger_path);
let _ = std::fs::remove_dir_all(&dir);
```

`dir` is the per-test directory returned by `scratch_dir` (pid+seq under `std::env::temp_dir()`), not the repository workspace and not the installed Pi. `ws` under test is a scratch fixture created by the test itself, so deleting `dir` removes only test-owned artifacts. The helper therefore does not remove the workspace or lock under test.

## Command 1 — `just check`

Command (exact):

```
just check
```

Raw tail:

```
test r2_requires_maintainer ... ok
test r1_requires_contributor ... ok
test r3_requires_owner ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

cargo test --locked -p stellwerk-gateway --test permission --test gate --test gate_ask
   Compiling stellwerk-gateway v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-gateway)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.84s
     Running tests/gate.rs (target/debug/deps/gate-3fcdfcaec0412607)

running 15 tests
test personal_data_to_third_party_is_deny_even_at_r0_l5 ... ok
test content_digest_is_sha256_of_the_bytes ... ok
test a_claim_that_disagrees_with_the_bytes_is_refused ... ok
test public_data_to_third_party_is_not_denied_by_data ... ok
test content_replaced_after_authorization_is_refused ... ok
test r0_at_l2_with_public_data_and_first_party_is_allow ... ok
test r4_at_l4_is_deny ... ok
test r5_is_deny_at_l5_owner_too ... ok
test r2_at_l3_is_ask ... ok
test r3_at_l4_is_deny_after_adr_0002 ... ok
test r2_at_l2_is_deny_not_ask ... ok
test r4_at_l5_is_ask_on_one_action_not_deny ... ok
test same_tool_path_different_content_gets_different_decision ... ok
test r3_at_l5_is_ask_human_decides_secret_access ... ok
test r1_at_l0_is_deny_for_lacking_write_rights ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/gate_ask.rs (target/debug/deps/gate_ask-06c893c58a6f21db)

running 7 tests
test ask_in_noninteractive_run_is_suspended_not_deny ... ok
test every_deny_carries_a_non_empty_reason ... ok
test suspended_and_deny_are_distinguishable_states ... ok
test unclassified_in_noninteractive_run_is_deny ... ok
test suspended_settles_into_deny_when_the_ask_window_expires ... ok
test unclassified_interactive_at_owner_is_ask ... ok
test unclassified_interactive_below_owner_is_deny_not_ask ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/permission.rs (target/debug/deps/permission-1799aaff5e03d2e9)

running 9 tests
test r0_is_allow ... ok
test r2_is_ask ... ok
test unclassified_never_allows ... ok
test all_risk_classes_covered ... ok
test r3_is_ask ... ok
test r5_is_deny ... ok
test r1_is_allow ... ok
test r4_is_ask ... ok
test unclassified_is_deny ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

stellwerk-task: 338/338 (0 RED, expected — активный milestone)
stellwerk-gateway: 80/80 (0 RED, expected — активный milestone)
EXIT=0
```

`just check` composes `fmt lint test-registry test-core test-store check-risk-tables test-status`. The log begins with:

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.10s
```

No fmt or clippy diagnostics were emitted. The log also contains a line `error: unrecognized subcommand 'badcmd'`; that line is inside a registry test that deliberately exercises CLI error handling, and the overall command exited 0 with all registry tests green, so it is not a gate failure.

Exit code: **0**.

## Command 2 — real Pi 1.0.2 run suite

Command (exact):

```
PI_BIN=/home/izyabretatel/.pi/agent/bin/pi cargo test -p stellwerk-cli --test run
```

Raw output (complete):

```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.12s
     Running tests/run.rs (target/debug/deps/run-2ce137b7fdece2c7)

running 13 tests
test failed_run_is_not_reported_as_success ... ok
test run_task_without_model_config_refuses_by_name ... ok
test missing_task_file_exits_nonzero ... ok
test run_without_git_head_refuses ... ok
test missing_workspace_is_named_refusal ... ok
test run_against_workspace_copy_sees_files ... ok
test run_attestation_reads_linked_worktree_commit ... ok
test missing_engine_is_named_refusal ... ok
test run_attestation_carries_bindings ... ok
test run_writes_receipt_to_sqlite ... ok
test both_entrypoints_share_one_authorization ... ok
test run_task_with_drives_one_model_turn_and_one_gate_request ... ok
test run_task_with_real_pi ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s

EXIT=0
```

`run_task_with_real_pi` asserts the version of the same binary:

```rust
assert_eq!(
    version,
    "1.0.2",
    "real Pi must be 1.0.2 at {}: {version:?}",
    real.display()
);
```

Exit code: **0**.

## Command 3 — plan suite

Command (exact):

```
cargo test -p stellwerk-cli --test plan
```

Raw tail:

```
running 41 tests
test a_draft_has_a_text_digest ... ok
test a_changed_draft_after_approval_requires_a_new_approval ... ok
test a_missing_session_is_absent_not_a_default ... ok
test a_matching_snapshot_leaves_approval_intact ... ok
test a_model_claim_of_approval_does_not_approve_anything ... ok
test a_plan_approved_for_another_snapshot_is_refused ... ok
test a_plan_reply_is_recognized ... ok
test a_plan_wrapped_in_prose_is_still_extracted ... ok
test a_plan_with_object_entries_names_the_file ... ok
test a_question_reply_is_recognized ... ok
test a_recorded_mode_is_not_overwritten_by_a_different_flag ... ok
test a_scope_entry_without_a_file_is_not_guessed ... ok
test a_snapshot_drift_after_approval_forces_reapproval ... ok
test a_text_edit_without_a_version_bump_is_caught_by_the_digest ... ok
test absent_summary_and_empty_summary_are_different_texts ... ok
test an_edit_after_approval_invalidates_the_request ... ok
test an_approved_host_plan_becomes_a_request ... ok
test an_empty_session_render_has_no_dialogue_or_live_text ... ok
test an_old_session_without_the_field_is_not_treated_as_host ... ok
test an_unparseable_reply_is_a_distinct_state_not_a_default ... ok
test a_session_round_trips_through_disk ... ok
test an_unparsed_reply_is_written_down ... ok
test an_unreadable_path_is_an_error_not_an_absent_session ... ok
test an_unrecorded_planner_is_not_host ... ok
test build_prompt_carries_goal_snapshot_and_json_contract ... ok
test approving_marks_the_current_version ... ok
test build_requires_an_approved_plan ... ok
test build_status_names_every_decision ... ok
test extract_json_rejects_reversed_braces ... ok
test changing_plan_text_without_bumping_the_version_still_requires_approval ... ok
test opencode_is_named_as_bypassing_the_transport ... ok
test planner_config_grants_no_mutating_execution_or_network_tool ... ok
test render_labels_the_current_stage_and_the_answering_prompt ... ok
test render_names_the_mode_and_calls_opencode_external ... ok
test render_shows_approval_once_the_human_approved ... ok
test render_shows_draft_fields_and_pending_approval ... ok
test render_shows_goal_input_before_anything_else ... ok
test set_goal_and_set_draft_bump_the_version ... ok
test the_build_names_the_missing_token ... ok
test the_digest_is_stable_for_identical_text ... ok
test the_session_records_the_planner_mode ... ok

test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

EXIT=0
```

Exit code: **0**.

## Command 4 — stellwerk-task four targets

Command (exact):

```
cargo test -p stellwerk-task --test evidence --test human_validation --test suspension --test lifecycle
```

Raw output (complete):

```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running tests/evidence.rs (target/debug/deps/evidence-1b4973198c5071a0)

running 22 tests
test chain_tail_distinguishes_entry_kinds ... ok
test chain_tail_is_reproducible_and_follows_the_chain ... ok
test absent_and_empty_binding_have_different_tails ... ok
test empty_chain_tail_is_a_real_digest_not_a_placeholder ... ok
test chain_without_validation_waits_for_a_human_and_is_not_done ... ok
test correct_order_closes_the_gate ... ok
test constructors_set_every_field ... ok
test fixture_validation_is_labelled_and_not_a_human_decision ... ok
test human_validation_accepts_the_chain ... ok
test lone_model_claim_is_not_proof ... ok
test refutation_entry_records_the_basis_and_the_hash_of_refuting_data ... ok
test refutation_entry_rejects_a_whitespace_only_basis ... ok
test present_entry_without_digest_is_refused ... ok
test only_the_constructors_mint_entries ... ok
test refutation_entry_rejects_an_empty_basis ... ok
test refutation_entry_rejects_empty_refuting_data ... ok
test tool_attestation_records_the_factual_verification ... ok
test tool_attestation_without_command_is_refused ... ok
test two_absent_conditions_give_two_reasons ... ok
test validation_without_tool_attestation_does_not_pass ... ok
test wrong_order_is_refused_and_names_the_expected_order ... ok
test tool_attestation_bindings_are_required_and_tamper_sensitive ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/human_validation.rs (target/debug/deps/human_validation-10355ea5a8391964)

running 4 tests
test resume_only_on_own_task_validation ... ok
test transition_requires_explicit_record ... ok
test resume_outcome_reports_own_validation_without_running_worker ... ok
test resume_outcome_foreign_validation_is_not_already_validated ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/lifecycle.rs (target/debug/deps/lifecycle-e96df14fe4df2bb6)

running 10 tests
test contract_allows_draft_to_planned ... ok
test contract_allows_verified_to_validated_but_not_back_to_running ... ok
test contract_forbids_done_to_running ... ok
test contract_allows_running_suspended_roundtrip ... ok
test contract_forbids_draft_to_verified ... ok
test direct_running_to_awaiting_is_illegal ... ok
test forbidden_advance_leaves_status_untouched ... ok
test advance_writes_new_status_into_store ... ok
test suspended_from_running_stays_suspended ... ok
test ask_from_running_reaches_awaiting_human_validation ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/suspension.rs (target/debug/deps/suspension-73f95944314b9e42)

running 2 tests
test suspension_status_distinguishes_question_from_silence ... ok
test non_interactive_run_does_not_hang_and_does_not_allow ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

EXIT=0
```

Exit code: **0**.

## Summary

| Command | Result | Exit |
|---|---|---|
| `just check` | fmt/clippy clean; registry/core/store suites green; task 338/338, gateway 80/80 | 0 |
| `PI_BIN=/home/izyabretatel/.pi/agent/bin/pi cargo test -p stellwerk-cli --test run` | 13 passed, 0 failed | 0 |
| `cargo test -p stellwerk-cli --test plan` | 41 passed, 0 failed | 0 |
| `cargo test -p stellwerk-task --test evidence --test human_validation --test suspension --test lifecycle` | 22 + 4 + 10 + 2 passed, 0 failed | 0 |

All four required groups pass. Real Pi 1.0.2 was exercised through the non-removing `pi_layout` helper via the canonical `PI_BIN`. No product regression was observed. FINDING `F-2026-11-19-01` is not re-opened.

## Verdict

APPROVE — every F3 command exited 0 with zero failures; real Pi 1.0.2 passed through the non-removing helper; no source, test, helper, plan, packet, ledger, boulder, notepad, or Pi installation was modified.
