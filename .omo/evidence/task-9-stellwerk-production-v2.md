# Task 9 — V2-07a F-08: add the Refuted evidence kind (contract-change)

Commit: `096f3d0fa7484a58cf74281c9f9102e55a8c2e2a` — `feat(contract): add Refuted evidence kind`
Packet: `.sisyphus/plans/V2/packet-07a.md`
Files (3): `crates/stellwerk-core/src/contract.rs`,
`crates/stellwerk-task/src/evidence.rs`,
`docs/adr/0024-refutation-evidence-kind.md`.

## Authorization to touch the contract

`crates/stellwerk-core/src/contract.rs` is otherwise forbidden by AGENTS.md.
The packet carries the `contract-change` label:

```
$ rg -n "contract-change: true" .sisyphus/plans/V2/packet-07a.md
11:contract-change: true
```

ADR required by the label: `docs/adr/0024-refutation-evidence-kind.md` (110 lines).

## RED — serde round-trip test written before the variant existed

The test module was appended to `contract.rs` first, then:

```
$ cargo test -p stellwerk-core
   Compiling stellwerk-core v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-core)
error[E0599]: no variant, associated function, or constant named `Refuted` found for enum `EvidenceKind` in the current scope
   --> crates/stellwerk-core/src/contract.rs:682:57
    |
479 | pub enum EvidenceKind {
    | --------------------- variant, associated function, or constant `Refuted` not found for this enum
...
682 |         let json = serde_json::to_string(&EvidenceKind::Refuted)?;
    |                                                         ^^^^^^^ variant, associated function, or constant not found in `EvidenceKind`

error[E0599]: no variant, associated function, or constant named `Refuted` found for enum `EvidenceKind` in the current scope
   --> crates/stellwerk-core/src/contract.rs:685:44
    |
479 | pub enum EvidenceKind {
    | --------------------- variant, associated function, or constant `Refuted` not found for this enum
...
685 |         assert_eq!(restored, EvidenceKind::Refuted);
    |                                            ^^^^^^^ variant, associated function, or constant not found in `EvidenceKind`

For more information about this error, try `rustc --explain E0599`.
error: could not compile `stellwerk-core` (lib test) due to 2 previous errors
EXIT=101
```

## GREEN — after adding `EvidenceKind::Refuted`

```
$ cargo test -p stellwerk-core
   Compiling stellwerk-core v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-core)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.76s
     Running unittests src/lib.rs (target/debug/deps/stellwerk_core-9454c52d4993d4f3)

running 5 tests
test contract::tests::refuted_kind_round_trips ... ok
test contract::tests::unknown_kind_is_rejected ... ok
test hexfmt::tests::matches_standard_vectors ... ok
test hexfmt::tests::multibyte_utf8_bytes_matter ... ok
test hexfmt::tests::order_and_length_matter ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
...
test result: ok. 13 passed; 0 failed; ...  (tests/task_lifecycle.rs)
...
EXIT=0
```

Full `cargo test -p stellwerk-core` output is green for every integration test
suite (`data_class_routing`, `event_risk`, `event_serde`, `rbac_level`,
`risk_class`, `risk_level_policy`, `task_lifecycle`, `workspace_policy`).

## Exhaustive matches updated (`crates/stellwerk-task/src/evidence.rs`)

- `kind_name`: `EvidenceKind::Refuted => "Refuted"` (chain tail distinguishes it).
- `evaluate`'s `match entry.kind`: documented empty `Refuted => {}` arm — the
  semantics land in V2-07c (todo 11); an unconsumed `Refuted` entry must not
  silently become a pass.
- Comment at `evidence.rs:160-165` ("not a new `EvidenceKind`") updated to point
  at ADR-0024, so the code no longer contradicts the ADR.
- No other exhaustive `match` on `EvidenceKind` exists in the workspace
  (`rg -n "match .*kind|EvidenceKind::" crates/ --type rust`); the workspace
  check below is the real exhaustiveness gate.

## Gates

```
$ cargo check --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.67s
CHECK_EXIT=0

$ cargo test -p stellwerk-task --test evidence
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
EVID_EXIT=0
   (`only_the_constructors_mint_entries` still counts 3: no constructor added.)

$ cargo test -p stellwerk-task --test refutation
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
REF_EXIT=0

$ cargo test -p stellwerk-task
TASK_EXIT=0   (all suites green)

$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.54s
CLIPPY_EXIT=0

$ cargo fmt --all -- --check
FMT_EXIT=0
```

## C2 budget (after commit, `scripts/plan-lint.py::added_counts`)

```
$ python3 -c "... added_counts(git show 096f3d0 ...)"
C2 src,tests = (102, 0)
budget_violations = []
```

src 102 <= 200, tests 0 <= 250; raw added diff lines 150 <= `max_diff_lines: 200`.
`docs/adr/*.md` is counted as `src`; the ADR is 110 lines and the Rust changes
are tight.

## ADR

`docs/adr/0024-refutation-evidence-kind.md` — declares `EvidenceKind::Refuted`
as its own chain kind, why it is neither `Validation` nor a `Failed` run,
supersedes the old `evidence.rs:160-165` comment, and reconciles with
ADR-0022's `TaskStatus::Refuted` (task status and chain entry are different
layers, both named `Refuted` on purpose).

## PolyForge chain (on main at 096f3d0)

```
evidence_append(ModelClaim) task=V2/packet-07a   -> seq 391 hash d792dcb3...
evidence_verify(tool="cargo test", args=[-p stellwerk-core])
                                                 -> seq 392 Verified, exit_code 0,
                                                    stdout_hash b310e14b...
gate_evaluate(task=V2/packet-07a, required=[Verified])
                                                 -> passed=true verified=1 missing=[]
chain_tail_hash: 99a443a910364db2ddcb3ec9bd41d91bd4fc31bbcb4308bdbca3154b35235f6c
```

## Orchestrator re-verification (independent of the worker)

- `git show --numstat 096f3d0` → exactly 3 files: `contract.rs 30/0`,
  `evidence.rs 10/4`, `docs/adr/0024-refutation-evidence-kind.md 110/0`.
- `cargo test -p stellwerk-core` → 13 passed (includes
  `refuted_kind_round_trips` and `unknown_kind_is_rejected`).
- `cargo test -p stellwerk-task` → every target green, `--test evidence` 18 passed
  (`only_the_constructors_mint_entries` still counts 3 — no constructor added).
- `cargo check --workspace --all-targets` exit 0;
  `cargo clippy --workspace --all-targets -- -D warnings` exit 0;
  `cargo fmt --all -- --check` exit 0.
- `added_counts 096f3d0` → (102, 0); raw 150 lines ≤ `max_diff_lines: 200`.
- `rg -n "contract-change: true" .sisyphus/plans/V2/packet-07a.md` → match at line 11
  (the label that authorizes touching `contract.rs`).
- `rg -n "EvidenceKind::Refuted"` → `contract.rs:688/691`, `evidence.rs:164/341/451`;
  no wildcard arm was introduced.
- `git status --porcelain` (tracked) → only the accepted baseline.
