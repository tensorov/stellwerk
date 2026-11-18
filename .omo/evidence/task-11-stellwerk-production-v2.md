# Task 11 — V2/07c (F-08): carry refutation into the evidence chain

- Packet: `.sisyphus/plans/V2/packet-07c.md`
- Commit: `9798e80c9ecff3eb11d19fea424fef01304fe7cf`
- Message: `feat(task): carry refutation into the evidence chain`
- Files: `crates/stellwerk-task/src/evidence.rs`, `crates/stellwerk-task/src/runner.rs`,
  `crates/stellwerk-task/tests/refutation.rs`

## RED — tests written first, variants did not exist yet

Command:

```
cargo test -p stellwerk-task --test refutation
```

Output (verbatim):

```
   Compiling stellwerk-task v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-task)
error[E0599]: no variant, associated function, or constant named `Refuted` found for enum `EvidenceStatus` in the current scope
   --> crates/stellwerk-task/tests/refutation.rs:197:41
    |
197 |     assert_eq!(refuted, EvidenceStatus::Refuted);
    |                                         ^^^^^^^ variant, associated function, or constant not found in `EvidenceStatus`

error[E0599]: no variant, associated function, or constant named `Refuted` found for enum `EvidenceStatus` in the current scope
   --> crates/stellwerk-task/tests/refutation.rs:199:33
    |
199 |     let label = EvidenceStatus::Refuted.label();
    |                                 ^^^^^^^ variant, associated function, or constant not found in `EvidenceStatus`

error[E0599]: no variant, associated function, or constant named `Refuted` found for enum `EvidenceStatus` in the current scope
   --> crates/stellwerk-task/tests/refutation.rs:213:57
    |
213 |     assert_eq!(evidence_status(&mixed), EvidenceStatus::Refuted);
    |                                                         ^^^^^^^ variant, associated function, or constant not found in `EvidenceStatus`

error[E0599]: no variant, associated function, or constant named `Refuted` found for enum `EvidenceStatus` in the current scope
   --> crates/stellwerk-task/tests/refutation.rs:317:25
    |
317 |         EvidenceStatus::Refuted,
    |                         ^^^^^^^ variant, associated function, or constant not found in `EvidenceStatus`

error[E0599]: no variant, associated function, or constant named `Refuted` found for enum `ResumeOutcome` in the current scope
   --> crates/stellwerk-task/tests/refutation.rs:327:40
    |
327 |     assert_eq!(outcome, ResumeOutcome::Refuted);
    |                                        ^^^^^^^ variant, associated function, or constant not found in `ResumeOutcome`

For more information about this error, try `rustc --explain E0599`.
error: could not compile `stellwerk-task` (test "refutation") due to 5 previous errors
```

## GREEN — after the implementation

Command (packet acceptance):

```
cargo test -p stellwerk-task --test evidence --test refutation
```

Output (tail, verbatim):

```
test only_the_constructors_mint_entries ... ok
test empty_chain_tail_is_a_real_digest_not_a_placeholder ... ok
...
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/refutation.rs (target/debug/deps/refutation-4d9b0c9ab31a919f)

running 7 tests
test done_after_refuted_is_rejected ... ok
test fixture_validation_is_not_a_human_decision ... ok
test evaluate_reports_a_refutation_and_does_not_pass ... ok
test refuted_requires_stated_basis ... ok
test refuted_chain_is_refuted_not_unattested ... ok
test validation_without_a_refutation_is_still_human_validated ... ok
test run_once_resuming_on_a_refuted_ledger_never_calls_the_worker ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Whole crate:

```
cargo test -p stellwerk-task
# EXIT=0, 43 test targets "test result: ok", 0 failed
```

Note: `error: not found: t-1` appears in the log from a passing fixture that
prints expected stderr; no `FAILED`/`failures:` lines and exit status is 0.

## Gates

```
cargo check --workspace --all-targets        # EXIT=0
cargo clippy --workspace --all-targets -- -D warnings  # EXIT=0
cargo fmt --all -- --check                   # EXIT=0
```

`crates/stellwerk-task/tests/evidence.rs` was NOT edited; its
`only_the_constructors_mint_entries` still sees exactly 4 `kind: EvidenceKind::`
sites in `src/evidence.rs` (green above).

## C2 (scripts/plan-lint.py::added_counts on the commit)

```
git show --format= 9798e80c9ecff3eb11d19fea424fef01304fe7cf | added_counts
=> (22, 216)
```

src 22 <= 200, tests 216 <= 250.

## PolyForge attestation (packet V2/packet-07c, after merge)

- `evidence_append(ModelClaim)` seq 395, entry hash `9105cd2fbf4c2a674606d2190566041a28337564a5941264614a61817d4a9fdd`,
  commit `9798e80`, diff_hash `69fa7992aaa988b2c74aa19a12b8a3b284d4d7bb7d6224cb7da87f3f9a694b44`.
- `evidence_verify(cargo test -p stellwerk-task --test evidence --test refutation)` seq 396 →
  `Verified`, exit 0, stdout_hash `0e248d2960ad949e34aa6f1e3c877ac93237969a179b54e44bbaa0331cf0e2b1`.
- `gate_evaluate(required=["Verified"])` → passed, claimed 1 / verified 1 / validated 0,
  chain tail `d5668c3b7365e28bafa139167bb40401348bbd3cfe638d2ad459cdee1d803407`.

## Orchestrator re-verification (independent of the worker)

- `git show --numstat 9798e80` → exactly three files: `src/evidence.rs` 36/4,
  `src/runner.rs` 8/3, `tests/refutation.rs` 216/3 (`tests/evidence.rs` untouched).
- `evidence_status` checks `Refuted` BEFORE the ToolAttestation guard; `evaluate` early-returns
  `passed: false` with `missing == ["refuted: <basis>"]` before every other check;
  `run_once_resuming` has `EvidenceStatus::Refuted => Ok(ResumeOutcome::Refuted)` before the
  `Unattested` arm, so the worker is never called.
- `cargo test -p stellwerk-task --test evidence --test refutation` → 22 + 7 passed;
  `cargo test -p stellwerk-task` → 43 `ok` result lines; `cargo check --workspace --all-targets`
  exit 0; clippy workspace exit 0; fmt exit 0.
- C2 recomputed by the orchestrator: `(22, 216)`.
