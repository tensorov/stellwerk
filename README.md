# Stellwerk

Attested agent orchestration: a control plane where task execution is gated by policy and every decision leaves evidence.

> **Pre-1.0.** Stellwerk is under active development. Contracts, APIs, and the CLI surface can change between revisions. Pin a commit if you depend on a specific behavior.

## What it is

Stellwerk runs agent tasks behind an explicit permission gate. The core stays deliberately boring and auditable: events, RBAC, sync, evidence, and a module registry. Agents are replaceable, memory is partitioned, and human approval is part of the loop rather than an afterthought.

## Workspace layout

Six product crates:

| Crate | Role |
| --- | --- |
| `stellwerk-core` | Contracts, events, risk and data-class policy |
| `stellwerk-registry` | Module registry and module commands |
| `stellwerk-store` | SQLite event and task storage |
| `stellwerk-task` | Task runner, sandbox, evidence ledger, model transport |
| `stellwerk-gateway` | Permission gate, risk tables, ask and approval channel |
| `stellwerk-cli` | `stellwerk` command-line entry point and terminal UI |

`xtask` is internal build and release tooling, not a seventh product crate. It is reached through the `cargo xtask` alias defined in `.cargo/config.toml`; there is no `cargo-xtask` binary to install.

## Binaries

The workspace builds exactly two product binaries:

- `stellwerk` (from `stellwerk-cli`): the `run`, `module`, `tui`, and `plan` commands.
- `stellwerk-task` (from `stellwerk-task`): the task runner.

There is no `stellwerk task` subcommand. The task runner is `stellwerk-task task ...`, for example `stellwerk-task task apply --file task.toml`.

## Getting started

See [docs/quickstart.md](docs/quickstart.md) for prerequisites, the two verification tiers, and a worked example. Build Stellwerk from a checkout; a fresh clone still needs the prerequisites and local configuration listed there.

## License

Licensed under either of

- [LICENSE-MIT](LICENSE-MIT) (MIT license)
- [LICENSE-APACHE](LICENSE-APACHE) (Apache License, Version 2.0)

at your option.
