# Quickstart

This page covers a source checkout of Stellwerk. A fresh clone is not ready to run on its own: it needs the prerequisites and the local configuration below. Stellwerk is pre-1.0, so pin a commit when you need stable behavior.

## Toolchain

`rust-toolchain.toml` pins the stable channel `1.99.0` with the `rustfmt` and `clippy` components. The workspace `rust-version` in `Cargo.toml` is `1.88`; that is the minimum supported compiler, not the one rustup will select here. Let `rust-toolchain.toml` choose the active toolchain.

## Prerequisites

Install these before building or running anything:

- **rustup**, plus `rustfmt` and `clippy` for the pinned toolchain. `rustup component add rustfmt clippy` if a component is missing.
- **`nightly-2026-10-01`**, only if you are running mutation testing. Nothing in the normal build or test flow needs it.
- **`python3`**, used by the plan-lint script (`scripts/plan-lint.py`).
- **`bwrap`** (bubblewrap), the sandbox backend used by the task runner.
- **`gh`**, the GitHub CLI, for release checks.
- **Pi 1.0.2**, the model engine. The code looks for `PI_BIN` first, then falls back to `$HOME/.pi/agent/bin/pi`. If Pi lives at the default path you can leave `PI_BIN` unset:

  ```sh
  export PI_BIN="$HOME/.pi/agent/bin/pi"   # optional at the default path
  ```

- **`STELLWERK_PI_EXTENSION`**, the absolute path to the Pi transport extension. Without it the CLI refuses to start rather than launching an engine with no model channel:

  ```sh
  export STELLWERK_PI_EXTENSION="$PWD/crates/stellwerk-task/pi/stellwerk_transport.ts"
  ```

- **A model token file** at `$XDG_CONFIG_HOME/stellwerk/neuraldeep.token`, or at `$HOME/.config/stellwerk/neuraldeep.token` when `XDG_CONFIG_HOME` is unset. Keep it readable only by you (`0600`):

  ```sh
  TOKEN_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/stellwerk"
  mkdir -p "$TOKEN_DIR"
  printf '%s' 'YOUR_TOKEN_VALUE' > "$TOKEN_DIR/neuraldeep.token"
  chmod 600 "$TOKEN_DIR/neuraldeep.token"
  ```

## Build

```sh
cargo build --workspace --exclude xtask
```

This produces the two product binaries, `stellwerk` and `stellwerk-task`. `cargo xtask` is a Cargo alias from `.cargo/config.toml` (`run --package xtask --`), not a third binary and not an external `cargo-xtask` install.

## Verification tiers

There are two tiers. Run tier 1 on every change. Run tier 2 before a release or when you need whole-workspace coverage.

### tier-1: strict gate (`just check`)

`just check` is the strict gate. It fails on any error. Today it runs exactly three recipes:

- `check-gate`, which runs `fmt` (`cargo fmt --all -- --check`), `lint` (`cargo clippy --workspace --all-targets -- -D warnings`), and the tests for the closed milestones: `test-registry`, `test-core`, `test-store`.
- `check-risk-tables`, which checks that the documented risk tables match the contract matrix.
- `test-status`, which prints GREEN/RED counters for the crates in active milestones. It is informational and does not fail the gate.

```sh
just check
```

`just check` is **not** the full workspace test suite. It deliberately covers only the crates whose milestones are closed, plus the risk-table checks and the status counters. There is no `history` recipe in the current justfile; a history scan is a separate release check, described in [SECURITY.md](../SECURITY.md).

### tier-2: full workspace tests

Run both commands. The first covers every product crate; the second covers the `xtask` test that is excluded from the first:

```sh
cargo test --workspace --exclude xtask
cargo test -p xtask --test ledger_encoding
```

Expect these to take longer than tier 1. Some suites in active milestones are RED by design; tier 2 is a full picture, not a clean pass guarantee.

## Run a task

The repository ships a real example at `crates/stellwerk-cli/examples/n1-smoke.toml`. With the prerequisites above configured, run it through the `stellwerk` binary:

```sh
cargo run -p stellwerk-cli --bin stellwerk -- run crates/stellwerk-cli/examples/n1-smoke.toml
```

A successful run prints a line with the verdict, receipt id, store path, risk, gate, and ledger path, for example:

```text
verdict=... receipt=... store=... risk=... gate=... ledger=...
```

The task runner is a separate binary. Use `stellwerk-task task ...` for task operations:

```sh
cargo run -p stellwerk-task --bin stellwerk-task -- task apply --file crates/stellwerk-cli/examples/n1-smoke.toml
cargo run -p stellwerk-task --bin stellwerk-task -- task get n1-smoke
```

There is no `stellwerk task` subcommand. The `stellwerk` binary has `run`, `module`, `tui`, and `plan`; task operations belong to `stellwerk-task task ...`.

## Repository hygiene note

A working tree can look clean because untracked and ignored files are not part of `HEAD`. That is not the same as a clean history: the commit history still contains internal artifacts. Review the history before publishing, and see [SECURITY.md](../SECURITY.md) for the release-security checks. That document is added in a later task of the publication-readiness work; the link becomes valid then.
