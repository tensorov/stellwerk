# Evidence — todo 10 / V2-07b (F-08): Refuted constructor

Packet: `.sisyphus/plans/V2/packet-07b.md`
Commit: `26843882607923dde604ec26e5f79db6b64d1314` — `feat(task): add the Refuted constructor`
Files (2): `crates/stellwerk-task/src/evidence.rs`, `crates/stellwerk-task/tests/evidence.rs`

## RED — before the constructor existed

Command: `cargo test -p stellwerk-task --test evidence`

```
   Compiling stellwerk-task v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-task)
error[E0432]: unresolved import `stellwerk_task::evidence::refutation_entry`
  --> crates/stellwerk-task/tests/evidence.rs:19:45
   |
19 |     evaluate, evidence_status, model_claim, refutation_entry, tool_attestation, validation,
   |                                             ^^^^^^^^^^^^^^^^ no `refutation_entry` in `evidence`

For more information about this error, try `rustc --explain E0432`.
error: could not compile `stellwerk-task` (test "evidence") due to 1 previous error
```

Why this RED is the asked-for failure: the new tests import `refutation_entry`; while the
constructor is absent the test target cannot compile, so the count test (3 -> 4) and the
constructor tests all fail at once.

## GREEN — after the constructor

Command: `cargo test -p stellwerk-task --test evidence`

```
running 22 tests
...
test refutation_entry_rejects_a_whitespace_only_basis ... ok
test refutation_entry_records_the_basis_and_the_hash_of_refuting_data ... ok
test refutation_entry_rejects_an_empty_basis ... ok
test refutation_entry_rejects_empty_refuting_data ... ok
test only_the_constructors_mint_entries ... ok
...
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full package: `cargo test -p stellwerk-task` — all targets green, including
`tests/refutation.rs` (2 passed, untouched).

## Gates

- `cargo clippy --workspace --all-targets -- -D warnings` → exit 0.
- `cargo fmt --all -- --check` → exit 0.
- C2 on commit `2684388` via `scripts/plan-lint.py::added_counts`:
  `(src, tests) = (21, 80)` (limits 200 / 250).

## What the constructor does

- `pub fn refutation_entry(task_id, id, basis, refuting_data, ts) -> Result<EvidenceEntry, RefutationError>`
  delegates validation to the existing `refutation(basis, refuting_data)?` — no duplicate
  trim/empty checks.
- `kind: EvidenceKind::Refuted`; `command = refutation.basis()` (trimmed basis);
  `result_hash = sha256_hex(refutation.refuting_data().as_bytes())` (private helper reused,
  not made public); `ts` as given.
- All four bindings stay `None`: a human refutation is not a recipe/policy/model/commit
  attestation, so absence is explicit "not applicable" (law 6).
- Constructor is not authority (mirrors `tool_attestation`/`validation`): it records the fact,
  the human decides whether the refutation is justified.

## Count test

`only_the_constructors_mint_entries` now asserts 4 `kind: EvidenceKind::` literal sites and
exactly one per kind, `Refuted` added to the loop.

## PolyForge attestation (packet V2/packet-07b, after merge)

- `evidence_append(ModelClaim)` seq 393, entry hash `e9556dccde27deaa6a5e5e1cab08608449290ddc856cf594d7e32df781ae110b`,
  commit `2684388`, diff_hash `450077b4777371220e14c5e12c10082b84d5ece17fa8713d8cbe9fda9722ead2`.
- `evidence_verify(cargo test -p stellwerk-task --test evidence)` seq 394 → `Verified`, exit 0,
  stdout_hash `8e597670f71f509a220f107482b2ceab39cf5fb2da031f084cfdfc7d4104e6de`.
- `gate_evaluate(required=["Verified"])` → passed, claimed 1 / verified 1 / validated 0,
  chain tail `c06f9fac0f1ed01f708f0bc94a2afb16bd2f4411838cc3dc15c6102aa75b27b0`.

## Orchestrator re-verification (independent of the worker)

- `git show --numstat 2684388` → exactly two files: `src/evidence.rs` 35/0, `tests/evidence.rs` 80/6.
- Constructor re-read: delegates to `refutation()`, `kind: Refuted`, `command = basis`,
  `result_hash = sha256_hex(refuting_data)` (existing private helper reused), four bindings `None`.
- `cargo test -p stellwerk-task --test evidence` → 22 passed; `cargo test -p stellwerk-task` → 43
  result lines all `ok`; clippy workspace exit 0; fmt exit 0.
- C2 recomputed by the orchestrator: `(21, 80)`.
