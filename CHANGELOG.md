# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). This project is pre-1.0, so contracts, APIs, and the CLI surface can change between revisions. Pin a commit when you depend on specific behavior.

## [Unreleased]

### Added

- Public `README.md` and a two-tier `docs/quickstart.md` covering prerequisites, the strict gate, and full-workspace tests.
- `SECURITY.md` with the supported-branch policy, private vulnerability reporting, identity, history composition, and the IP attestation.
- `CONTRIBUTING.md` with the RED/GREEN/PROOF loop, packet limits, and the worktree-to-attest order.
- Executable publication checks for workspace metadata, the public tree, and the commit history.

### Changed

- Internal run artifacts and non-plan OMO files are no longer tracked; `HEAD` is clean of them.
- Live development endpoints were redacted from the public plan files. The commit history was left as is.

## [0.0.1] — 2026-11-19

Initial public checkpoint of the Stellwerk workspace.

### Added

- `stellwerk-core`: contracts, events, risk and data-class policy, evidence types.
- `stellwerk-registry`: module registry and module commands.
- `stellwerk-store`: SQLite event and task storage.
- `stellwerk-task`: task runner, sandbox, evidence ledger, and model transport.
- `stellwerk-gateway`: permission gate, risk tables, and the ask/approval channel.
- `stellwerk-cli`: the `stellwerk` binary with `run`, `module`, `tui`, and `plan` commands.
- The `stellwerk-task` binary with `task apply/get/watch/cancel/logs`.
- `xtask` build and release tooling, reached through the `cargo xtask` alias.
- Dual MIT and Apache-2.0 licensing.

### Notes

- No maintained release branches. Fixes land on `main`.
- The history contains internal artifacts from development. See `SECURITY.md` before redistributing a checkout.
