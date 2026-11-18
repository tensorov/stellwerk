# Repo Hygiene Findings (H1/08)

## Owner-WIP
Records: 8 owner-WIP paths; they remain untouched:
- `.gitignore`
- `.omo/omo.jsonc`
- `crates/stellwerk-gateway/src/shaping.rs`
- `crates/stellwerk-gateway/tests/mount.rs`
- `crates/stellwerk-gateway/tests/session_failure.rs`
- `crates/stellwerk-gateway/tests/shaping.rs`
- `crates/stellwerk-registry/src/cmd_add.rs`
- `crates/stellwerk-task/src/cli.rs`

## Formatting
`just fmt` 12 hunks / 4 files incl. `http_wire.rs:14`, `model_backend.rs:64` (plus diffs in `crates/stellwerk-gateway/tests/shaping.rs`).

## Linting
`just lint` `session.rs:613` `manual_noop_waker`.

## Audit live
`audit_live` 6/7 then 7/7 isolated (all 7 tests pass).

## Remaining untracked canonical packets
Records exact 33 remaining untracked canonical packets: N1 packet-01..10 (10), N2 00b,00c,00d,00e,00f,00g1,00g2,00g3,00g3b,00h1,00h2a,00h2b,00h3a,00h3b,00h3c,00h3d,00h3e,00i1 (18), TUI packet-00..04 (5); plus untracked non-packet N1/index.md, N1/REVIEW.md.

### N1 packet-01..10 (10)
- `.sisyphus/plans/N1/packet-01.md`
- `.sisyphus/plans/N1/packet-02.md`
- `.sisyphus/plans/N1/packet-03.md`
- `.sisyphus/plans/N1/packet-04.md`
- `.sisyphus/plans/N1/packet-05.md`
- `.sisyphus/plans/N1/packet-06.md`
- `.sisyphus/plans/N1/packet-07.md`
- `.sisyphus/plans/N1/packet-08.md`
- `.sisyphus/plans/N1/packet-09.md`
- `.sisyphus/plans/N1/packet-10.md`

### N2 00b,00c,00d,00e,00f,00g1,00g2,00g3,00g3b,00h1,00h2a,00h2b,00h3a,00h3b,00h3c,00h3d,00h3e,00i1 (18)
- `.sisyphus/plans/N2/packet-00b.md`
- `.sisyphus/plans/N2/packet-00c.md`
- `.sisyphus/plans/N2/packet-00d.md`
- `.sisyphus/plans/N2/packet-00e.md`
- `.sisyphus/plans/N2/packet-00f.md`
- `.sisyphus/plans/N2/packet-00g1.md`
- `.sisyphus/plans/N2/packet-00g2.md`
- `.sisyphus/plans/N2/packet-00g3.md`
- `.sisyphus/plans/N2/packet-00g3b.md`
- `.sisyphus/plans/N2/packet-00h1.md`
- `.sisyphus/plans/N2/packet-00h2a.md`
- `.sisyphus/plans/N2/packet-00h2b.md`
- `.sisyphus/plans/N2/packet-00h3a.md`
- `.sisyphus/plans/N2/packet-00h3b.md`
- `.sisyphus/plans/N2/packet-00h3c.md`
- `.sisyphus/plans/N2/packet-00h3d.md`
- `.sisyphus/plans/N2/packet-00h3e.md`
- `.sisyphus/plans/N2/packet-00i1.md`

### TUI packet-00..04 (5)
- `.sisyphus/plans/TUI/packet-00.md`
- `.sisyphus/plans/TUI/packet-01.md`
- `.sisyphus/plans/TUI/packet-02.md`
- `.sisyphus/plans/TUI/packet-03.md`
- `.sisyphus/plans/TUI/packet-04.md`

### Untracked non-packet
- `.sisyphus/plans/N1/index.md`
- `.sisyphus/plans/N1/REVIEW.md`

## Plan lint after H1/01-H1/03

`just plan-lint` exits 1, which is allowed for recorded real findings:
H1 has 8 packets and 0 violations; C7 risk tables match `contract.rs`.
The run still reports pre-existing C1 intra-milestone overlaps and 23 C9
cross-milestone overlaps in N1/N2/TUI (plus 2 real C1 TUI overlaps). Those
packet overlaps are outside H1 scope and are not normalized or fixed here.

Total: 33 canonical packets remain untracked. `00j7a` is tracked at HEAD in commit `4f117a6`.
