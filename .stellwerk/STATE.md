# Stellwerk Phase 0 state

audited: 2026-09-24

## Repository

| item | observed |
|---|---|
| branch | `main` @ `f2e2c57` |
| status | modified gitlinks `.worktrees/m0/audit`, `.worktrees/m0/measure`; untracked `.ablation-solpi/`, `.omo/plans/M0/*.result.*`, one continuation JSON |
| main log | `f2e2c57 style: fmt after packet-05 merge`; `8840646 feat(registry): module add command (M0 packet-05)`; `4fc58f5 feat(registry): cmd_add GREEN (packet-05)`; `b8e887e style: format merged registry sources`; `63172ef Merge branch 'm0/cli'`; `fbcee79 merge: resolve lib.rs (add model, keep cli)`; `205b530 feat(registry): CLI wiring GREEN (packet-04)`; `0a7df64 feat(registry): model types RED+GREEN (packet-01/02)`; `1f252f3 test(registry): add RED CLI parsing tests (packet-03)`; `5a3bd26 chore: M0 plan (12 packets), Pi ablation results, docs`; `8d27f13 docs: freeze context budget from measurement (worker=9415, OMO=+47622)`; `71c295c chore(deps): pin allowlist to current versions, fix resolver/toml req`; `b7ddb4b fix(just): recipe bodies on indented lines`; `b9eaa6f chore: level-0 boilerplate (scope M0: nexus-registry)` |
| indexed artifacts | gitlinks: `.worktrees/m0/{audit,cli,measure,model}`; tracked result files: `.sisyphus/plans/M0/packet-{01..06,08}.result.{jsonl,md,stderr}` |
| worktrees | main; `.worktrees/.ablation/baseline-r2`; prunable detached `.worktrees/.ablation/test-wt`; `m0/{audit,cli,measure,model}` |
| branches | `main`, `m0/audit`, `m0/cli`, `m0/measure`, `m0/model`, `ablation-baseline-r1`, `ablation-baseline-r2` |

## Lane checks

| lane | last commit | dirty paths | filtered test result |
|---|---|---|---|
| `m0/audit` | `f461b39 test(registry): audit RED + rmcp dep (packet-08)` | `src/cli.rs`, `src/cmd_audit.rs` | `audit`: exit 0, 14 passed, 20 filtered |
| `m0/measure` | `2d572f1 test(registry): measure RED (packet-06)` | `src/cli.rs`, `src/cmd_measure.rs` | `measure`: exit 101, 16 passed, 1 failed (`measure_module_fake_server`), 18 filtered |
| `m0/cli` | `205b530 feat(registry): CLI wiring GREEN (packet-04)` | none | `cli`: exit 0, 12 passed |
| `m0/model` | `0a7df64 feat(registry): model types RED+GREEN (packet-01/02)` | none | `model`: exit 0, 8 passed |

`just check` on main: exit 0; workspace tests reported 34 passed, 0 failed.

## PolyForge transport

`polyforge-cli` is installed at `/home/izyabretatel/.cargo/bin/polyforge-cli`; it rejects `--help` and prints usage only with no command. Real shell syntax: `pf init`; `pf append <kind> <payload> [--task <id>] [--commit <sha>] [--diff <hash>] ...`; `pf ledger tail`; `pf ledger summary`; `pf ledger export --otel`; `pf gate <task_id> [--required verified,validated] [--commit <sha>] [--diff <hash>]`; `pf coverage-check --report <llvm-cov.json>`. No CLI `verify` command was found; verification is exposed through the PolyForge MCP tool, not `polyforge-cli evidence/verify`. `polyforge-mcp --help` exits 1 with `ConnectionClosed("initialize request")` and is an MCP server, not a help CLI.

## Phase 0 decision

No STOP condition: main is green; history and known lane state are explainable. Proceed to repository hygiene.
