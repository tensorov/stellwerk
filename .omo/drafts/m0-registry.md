---
slug: m0-registry
status: planned
intent: clear
review_required: false
pending-action: none — plan + 12 packets written, just plan-lint OK, Momus APPROVE, Metis fixes incorporated; ready for lane handoff to stellwerk-worker-pi
approach: TDD packet set (≤12) for stellwerk-registry CLI: add|measure|audit|update; each command opens with a failing test; deliverable = self-contained packets under .sisyphus/plans/M0/, handed by user to Pi workers (no /start-work).
---

# Draft: m0-registry

## Components (topology ledger)
<!-- id | outcome (one line) | status: active|deferred | evidence path -->

1. cli-shell | clap subcommands module add|measure|audit|update wired in main.rs | active | crates/stellwerk-registry/src/main.rs:1-5
2. cmd-add | append validated module block to modules.toml, no build | active | docs/M0.md:5
3. cmd-measure | schema bytes × calibration -> context_cost_tokens in modules.lock; polyforge acceptance 813 ±10% | active | docs/M0.md:6-7, docs/PROMETHEUS_PROMPT.md:37-39
4. cmd-audit | spawn kind=mcp server, tools/list + one tools/call per declared tool, diff vs manifest | active | docs/M0.md:8-9
5. cmd-update | fetch -> build -> smoke(audit) -> measure -> gh pr create; no gh -> .stellwerk/pending/ stop | active | docs/M0.md:10-11
6. model-types | where Module/Source/Lock types live (contract.rs freeze vs M0 fill) | active | AGENTS.md vs crates/stellwerk-core/src/contract.rs:1-2

## Open assumptions (announced defaults)
<!-- assumption | adopted default | rationale | reversible? -->
- MCP client for audit/measure = rmcp (workspace allowlist), added to stellwerk-registry/Cargo.toml as `rmcp = { workspace = true }` (crate manifest allowed; root Cargo.toml untouched) | pending user veto at gate
- unit tests use tempdir modules.toml/modules.lock fixtures + a tiny fixture MCP server binary for audit; real polyforge-mcp only for the calibration/acceptance path | pending veto
- audit invalid-args probe = ADOPTED (Q3) | user picked recommended 2026-09-22 | reversible

## Findings (cited - path:lines)

- workspace members = stellwerk-core, stellwerk-registry; allowlist deps incl. rmcp client+transport-child-process | Cargo.toml:3,10-20
- registry crate: bin `stellwerk`, deps have clap/serde/toml/tokio/sha2 but NOT rmcp yet | crates/stellwerk-registry/Cargo.toml:8-22
- contract.rs intentionally empty; header says "первый packet — падающий тест, не структура" and lib.rs says only contract-change packet may change it | crates/stellwerk-core/src/contract.rs:1-2, crates/stellwerk-core/src/lib.rs:5
- AGENTS.md forbids touching contract.rs AND modules.lock AND root Cargo.toml | AGENTS.md "Границы правки"
- modules.toml: calibration bytes_per_token=0.0 PIN_ME; comment says coeff = Δtokens/bytes measured once by hand; polyforge module block with tools list of 4 | modules.toml:8-21
- modules.lock: generated file, version=1, one block per module sorted by name | modules.lock:1-3
- plan-lint: scans ONLY .sisyphus/plans/*.md (non-recursive); requires ≤12 packet headings, cargo test, DEFERRED, calibration mention, forbids cargo add and scope-leak words | scripts/plan-lint.sh:5-20
- measure calibration point: polyforge-mcp 4 tool schemas = 813 tokens qwen | docs/PROMETHEUS_PROMPT.md:37-39
- execution model: packets only; worker = Pi v0.87.0 / stellwerk-worker-pi, sees only packet-NN.md; PolyForge via polyforge-cli bash, not MCP | docs/PROMETHEUS_PROMPT.md:14-18,43-46
- packet invariants: ≤3 files, ≤200 lines, ≤4k tokens, ≤12 packets, up to 3 parallel | docs/PROMETHEUS_PROMPT.md:23-35, docs/CONTEXT_BUDGET.md:22-25

## Decisions (with rationale)

- intent: clear (explicit "Проинтервьюируй меня" override) | interview on, adopt-default filter OFF
- review_required: false (no high-accuracy modifier stated)
- M0.md pre-decided items are LOCKED: crate allowlist, no qwen tokenizer (bytes×calibration), skill-audit DEFERRED, lock TOML sorted by name | docs/M0.md:13-17
- Q1 (2026-09-22, user): types live in stellwerk-registry (src/model.rs); contract.rs stays frozen all M0; no contract-change packet
- Q2 (2026-09-22, user): tokens = round(bytes × tokens_per_byte); calibration packet renames bytes_per_token → tokens_per_byte in modules.toml
- Q3 (2026-09-22, user): audit probes every tool with invalid/empty args; any well-formed JSON-RPC reply (result|error) = invocable; manifest check = tools/list names exact-match + every declared tool answered
- Q4 (2026-09-22, user): Source = three-variant enum — git = { url, rev }, npm = { package, version? }, cargo = { crate, version? }; add validates, refuses unknown keys, never fetches
- Q5 (2026-09-22, user): update fetch = git fetch/checkout of source.rev into temp dir (modules external); build = cargo build -p stellwerk-registry (self); PR touches modules.lock (+modules.toml rev bump on channel=tag), base = repo default branch via plain `gh pr create`
- Q6 (2026-09-22, user): measure binary writes real repo modules.lock; cargo tests mutate only tempdir fixtures; final acceptance runs the binary once to refresh committed modules.lock
- APPROVED 2026-09-22 with mandatory amendments A-D (scope unchanged, ≤12 packets):
  - A (RED semantics): RED-only packets = 01, 03, 06, 08, 10. Acceptance inverted: `cargo build -p stellwerk-registry` OK AND `cargo test -p stellwerk-registry <filter>` exits non-zero AND failure is an assert (not compile error). evidence_verify targets an explicit wrapper returning 0 iff (build passes AND test fails): `bash -c 'cargo build -p stellwerk-registry && ! cargo test -p stellwerk-registry <filter>'`. Packet body must state: DO NOT fix/red-green the test inside a RED packet.
  - B (branch discipline): lane = one branch/worktree. RED+GREEN of a lane run sequentially in the SAME branch; merge to main only after GREEN (update-lane: after packet 11). Different lanes parallelize as branches, max 3 concurrent; never parallel packets inside one lane. Order: model (01-02) → cli (03-04) → three parallel lanes add/measure/audit → update (10-11).
  - C (split 07): 07a = GREEN measure + lock write (tempdir tests); 07b = calibration acceptance with REAL polyforge-mcp (rename bytes_per_token→tokens_per_byte, value, measured_at in modules.toml). Total 12 packets (07a/07b naming), exactly at cap, still plan-lint-OK. 06/07 note (anti-circularity, for Momus): 813 ±10% is the OpenCode context_input DELTA for polyforge-mcp including tool-formatting overhead, NOT pure schema bytes; test 06 is a PINNING round-trip (bytes × tokens_per_byte ≈ 813), not a correctness proof of the coefficient — state this verbatim in packets 06/07a/07b.
  - D (gh swappable): update reads binary from env STELLWERK_GH_BIN (default `gh`); tests inject fixture script; missing-binary scenario → .stellwerk/pending/ + stop covered by test with nonexistent bin; real gh only in manual final M0 acceptance. Pending dir is `.stellwerk/pending/` (NOT legacy `.nexus/pending/` — repo-wide grep confirms no nexus leftovers; packets must spell `.stellwerk/pending/`).
- Minor (silent): rmcp = { workspace = true } added to crate Cargo.toml ONLY in packet 08 (audit RED) — sole Cargo.toml edit in M0, called out for Momus; audit probe timeout = 10s per tool (tokio::time::timeout); M0-index.md carries packet→lane→depends_on table for Atlas distribution.

## Scope IN

- crate stellwerk-registry only: module add|measure|audit|update + supporting types/tests
- deliverable: .sisyphus/plans/M0/packet-NN.md self-contained packets (+ plan-lint-visible top-level artifact)
- TDD, PolyForge evidence chain, workspace-deps-only

## Scope OUT (Must NOT have)

- everything in docs/DEFERRED.md; kanban/hermes/gateway/nag/crm/obsidian/sync mentions planning-forbidden
- scripts/ablation/, docs/experiments/, docs/CONTEXT_BUDGET.md modifications
- /start-work, Atlas/Sisyphus delegation, OMO in workers
- cargo add, root Cargo.toml edits, hand-editing modules.lock, contract.rs edits (pending Q1 resolution)

## Open questions

- none blocking — interview complete (Q1-Q6 answered)

## Approval gate
status: approved (2026-09-22, user: «С поправками A–D — ок, фиксируй план и все packets»)
approach: 12 TDD packets (01,02,03,04,05,06,07a,07b,08,09,10,11 — ровно лимит) under .sisyphus/plans/M0/packet-NN.md + top-level .sisyphus/plans/M0-index.md (packet→lane→depends_on table) for just plan-lint; lanes: model (01→02) → cli (03→04) → three parallel branches add (05) | measure (06→07a→07b) | audit (08→09) → update (10→11, after 07b+09); RED packets = 01,03,06,08,10 (inverted acceptance + explicit wrapper); types in stellwerk-registry/src/model.rs; calibration tokens=bytes×tokens_per_byte with rename bytes_per_token→tokens_per_byte; 813±10% = context_input delta (pin, not correctness); audit invalid-args probes, probe timeout 10s/tool, rmcp added in packet 08 (only Cargo.toml edit in M0); Source three-variant enum; update = STELLWERK_GH_BIN (default gh) + external git fetch + self-build + audit + measure + gh pr (no gh → .stellwerk/pending/ stop); tests tempdir-only, binary refreshes real modules.lock at final acceptance; real gh only in manual final M0 acceptance.
pending_action: DONE — .omo/plans/m0-registry.md + .sisyphus/plans/M0/packet-*.md (12 files) + .sisyphus/plans/M0-index.md; just plan-lint OK (12 packets); Momus APPROVE; Metis BLOCKED→fixes applied (merge anchors, fixtures, rmcp decision, typos); hand lanes to stellwerk-worker-pi.
next: user distributes lane packets to workers; no further planning work.
