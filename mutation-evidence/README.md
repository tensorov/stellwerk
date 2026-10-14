# Mutation evidence — N1/F8 (authoritative tool risk resolution)

Self-contained, auditable bundle for the mutation run that backs the N1/F8 fix.
This directory is intentionally KEPT (not cleaned up). `mutants.out/` is also left
in place at the repository root.

## Provenance

- Commit under test: `57f11e6` (HEAD at run time; not reverted, not re-done)
- Date of run: 2026-10-14 (start 2026-10-14T06:28:54Z, end 2026-10-14T06:36:28Z)
- Tool: `cargo-mutants 27.1.0` (`cargo mutants --version`)
- Scope: `crates/stellwerk-task/src/runner.rs`, `crates/stellwerk-task/src/orchestrator.rs`
  (see `scope.txt` for why)

## Exact command

```
cargo mutants -p stellwerk-task -t 300 --in-place -f runner.rs -f orchestrator.rs
```

`-t 300` is the per-mutant timeout in seconds. `-- <name>` would be a cargo TEST
FILTER, not a file scope, and is deliberately not used.

## Result

| metric   | value |
|----------|-------|
| caught   | 57    |
| missed   | 9     |
| unviable | 29    |
| timeout  | 0     |
| total    | 95    |

caught / (caught + missed) = 57 / 66 = **86.4%** (threshold >= 75% -> PASS).
`unviable` is NOT in the denominator.

## Recompute

From this directory:

```
python3 -c "import json;d=json.load(open('outcomes.json'));c=d['caught'];m=d['missed'];print(f'{c}/{c+m} = {100*c/(c+m):.1f}%')"
```

Expected output: `57/66 = 86.4%`

## Bundle contents

- `outcomes.json` — cargo-mutants outcome file, copied verbatim from `mutants.out/outcomes.json`
- `argv.txt` — the exact command line used
- `scope.txt` — which files the run was scoped to, and why
- `counts.txt` — caught / missed / unviable / timeout counts and the computed percentage
- `snapshots/` — the `*.json` listing/snapshot files cargo-mutants produced
  (`lock.json`, `mutants.json`, `outcomes.json`)
- `SHA256SUMS` — sha256 of every file in this bundle
- `README.md` — this file

## Verify integrity

```
sha256sum -c SHA256SUMS
```
