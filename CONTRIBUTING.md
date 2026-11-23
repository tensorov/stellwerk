# Contributing

Stellwerk is pre-1.0 and developed as a set of task packets. Read `AGENTS.md` before opening a change. It is the authority on invariants and edit boundaries.

## Test-driven development

Every change follows RED, then GREEN, then PROOF.

1. **RED.** Write a failing test that names the behavior in Given/When/Then form. Run it and show the failure. A test that fails for a typo or a missing import is not RED.
2. **GREEN.** Write the minimum code that makes the test pass. Add the next case only after the first one is green.
3. **PROOF.** Run the exact acceptance command, append a `ModelClaim` to the evidence ledger, and verify it with the tool that ran the test. A gate result is a tool attestation, not a claim in prose.

A commit without a new test is rejected. Do not use `#[ignore]`, skip tests, or replace an implementation with a mock unless the packet explicitly allows it.

## Packet size

One task packet, at most 3 files. The authoritative line budget is C2 as counted by `scripts/plan-lint.py` in `added_counts`: source lines are counted logically, without blank lines and lines starting with `//`; test lines count in full under a separate budget. `wc -l` is not the metric. When C2 requires it, return `SPLIT_REQUIRED` instead of raising a ceiling. A lint-policy change needs a `contract-change` packet and an ADR in `docs/adr/`.

## Forbidden

- `#[allow(...)]` in code, without exception.
- `cargo add`. Dependencies come only from `[workspace.dependencies]` in the root `Cargo.toml`, included with `workspace = true`. A new dependency means `BLOCKED`.
- Editing `modules.lock`, `.github/workflows/`, `crates/stellwerk-core/src/contract.rs`, or the root `Cargo.toml` outside a `contract-change` packet.
- Treating missing data as a default value. Absence is its own state, as defined in `AGENTS.md`.

## Verification tiers

- `just check` is the strict gate. It fails on any error. Today it runs `check-gate`, `check-risk-tables`, and `test-status`.
- `just status` is the planned informational RED report for the crates in active milestones. It does not exist yet. Task 17 adds it and removes `test-status` from `just check`. Do not describe that split as already live.

Run `just check` on every change. `docs/quickstart.md` describes the full-workspace tier.

## Worktree order

Work sequentially in one worktree. Run the test suite on `main` first, then create the worktree, implement, and commit. After the work is committed, merge back to `main` with `merge --no-ff`, attest on `main`, evaluate the gate, and make a separate evidence commit carrying the chain tail. In short: main, then worktree, then merge, then attest.

Attesting a stub on `main` while the implementation lives only in a worktree is forbidden. So is declaring a gate passed without `gate_evaluate`.
