# repo-hygiene - Work Plan

## TL;DR (For humans)

**What you'll get:** Stellwerk's plan tools stop reporting "no packets" and start reading the packet formats the repository actually uses. The missing C7 source documents are restored, the completed `00j7a` packet is tracked, and the remaining red gates are written down as findings instead of being hidden.

**Why this approach:** We teach the tools the current N2/N1/TUI formats instead of rewriting 42 canonical packets. C7 is restored from history because the documents already matched `contract.rs` before they went missing. `00j7a` is the only canonical packet with a completed, verified implementation, so it is the only canonical packet committed.

**What it will NOT do:** No product code, no owner-WIP changes, no `just check` green claim, no packet normalization, no `#[allow]`, no `#[ignore]`, no dependency changes.

**Effort:** Medium
**Risk:** Medium - parser changes touch shared Python tools; C9 may expose real cross-milestone overlaps.
**Decisions to sanity-check:** C7 documents are restored verbatim from `4e17bb4^`; all 33 other untracked canonical packets stay untracked; `plan-lint` may exit 1 on real C1/C2/C3/C9 findings.

Your next move: start execution in a separate worker session with `$start-work repo-hygiene`. Full execution detail follows below.

---

> TL;DR (machine): 8 implementation todos, 4 final verifiers; teach plan-lint/packet_replay/accept.sh current formats, restore 3 C7 docs, track 00j7a, record findings; no product code.

## Scope

### Must have

- `plan-lint` parses canonical N2 YAML packets: frontmatter `depends_on`, multiline `allowed_changes`, `## Зачем`, `## RED`, `## Приёмка`.
- `plan-lint` parses N1/TUI packets: bold `**depends_on:**`, `**limit_waiver:**`, `## files_allowed` bullet list, `## Цель`, `## Приёмка`, optional `## RED`.
- C9 sees all canonical milestones, including N1/N2/TUI/H1; `_deps_of` understands `N2/00j7`, `N2-00j7`, `TUI-00`, `H1/01`.
- C7 self-test is genuinely green at 47/47, with the fixture root cause reproduced and fixed; 47/47 is not claimed before that evidence exists.
- `packet_replay` reads current formats, extracts the first shell command from the selected section (preferring `cargo test`), and reports explicit `NO_COMMAND` / `UNSUPPORTED_COMMAND` states.
- `accept.sh` reads current `allowed_changes` / `## files_allowed` and RED/GREEN sections; no `cargo test` is `NO_TEST_COMMAND`, never pass.
- Three C7 documents restored verbatim from `4e17bb4^`: `.sisyphus/plans/M1/packet-05.md`, `.sisyphus/plans/M1/packet-06.md`, `.sisyphus/plans/M3/REVIEW.md`.
- Only `.sisyphus/plans/N2/packet-00j7a.md` is added to canonical tracking.
- `.sisyphus/plans/H1/FINDINGS.md` records owner-WIP, fmt/lint, `audit_live`, and the exact 33 remaining untracked canonical packets.
- TDD for H1/01-H1/05: a failing fixture/test exists before the adapter.

### Must NOT have (guardrails, anti-slop, scope boundaries)

- No product-code edits; no `session.rs:613` fix; no `manual_noop_waker` change.
- No staging, reverting, formatting, or editing the 8 owner-WIP files: `.gitignore`, `.omo/omo.jsonc`, `crates/stellwerk-gateway/src/shaping.rs`, `crates/stellwerk-gateway/tests/{mount,session_failure,shaping}.rs`, `crates/stellwerk-registry/src/cmd_add.rs`, `crates/stellwerk-task/src/cli.rs`.
- No `just fmt` / `just check` repair by touching owner-WIP; no `#[allow(...)]`, `#[ignore]`, no test weakening.
- No normalization or bulk commit of the 42 original canonical packets.
- No edits to `.omo/plans/pi-tools-channel.md`, `.omo/evidence/`, `.omo/start-work/`, `.playwright-mcp/`, `mutants.out*`.
- No new dependencies, no `cargo add`, no root `Cargo.toml`, `contract.rs`, `modules.lock`, or workflow changes.
- No `audit_live` flake fix; no claim that `just check` is green.
- No implementation during planning; execution belongs to the worker session.

## Verification strategy

> Zero human intervention - all verification is agent-executed.

- **Test decision: TDD** for H1/01-H1/05. Each parser/adapter gets a failing fixture or shell test first; no adapter lands without a RED observation.
- **Python:** stdlib `unittest` only; no new dependency. Test files live under `scripts/tests/` and `scripts/integrator/tests/`.
- **Evidence root:** `.omo/evidence/repo-hygiene/H1-01/` ... `H1-08/`. Each task writes command output, exit code, and `git diff --stat` there.
- **Gate honesty:** `plan-lint` exit 1 is allowed when C1/C2/C3/C9 report real violations; the output is recorded. Exit 0 is asserted only if the actual run has no violations.
- **No `just check` green:** `just fmt` remains red on owner-WIP + two pre-existing hunks, `just lint` remains red on `session.rs:613`; this is a finding, not a fix in this plan.
- **Structural self-check before handoff:** every task row is column-zero and matches `- [ ] N.`; every final verifier matches `- [ ] F<n>.`; no prose bullet is used as a task.

## Execution strategy

### Parallel execution waves

> Target 5-8 todos per wave where possible. The plan has 8 implementation tasks; H1/01-H1/03 are deliberately serial because they share `scripts/plan-lint.py` and its test file.

- **Wave 1 - independent tools and artifacts (6 tasks):** H1/01 (plan-lint N2), H1/04 (packet_replay), H1/05 (accept.sh), H1/06 (C7 docs), H1/07 (00j7a), H1/08 (findings).
- **Wave 2 - plan-lint N1/TUI (1 task):** H1/02, blocked by H1/01.
- **Wave 3 - plan-lint C9/C7 (1 task):** H1/03, blocked by H1/02.
- **Wave 4 - final verification (4 tasks):** F1-F4 in parallel after all implementation tasks.

H1/06-H1/08 do not share files with the Python work; they may run in Wave 1. H1/07 only stages `00j7a`; it does not touch owner-WIP.

### H1 packet contract

Before Todo 1, the worker materializes eight canonical packets under `.sisyphus/plans/H1/` and mirrors them byte-for-byte to `.omo/plans/H1/`. These are plan artifacts, not product code.

Use this legacy schema so the current tools can parse them before the adapter lands; after H1/01-H1/03 they remain backward-compatibility fixtures:

```markdown
---
id: H1/01
milestone: H1
title: plan-lint parses N2 YAML packets
depends_on: []
allowed_changes:
  - scripts/plan-lint.py
  - scripts/tests/test_plan_lint_formats.py
max_files: 2
max_diff_lines: 450
---
```

Packet-specific fields:

| packet | depends_on | allowed_changes | test |
|---|---|---|---|
| H1/01 | [] | `scripts/plan-lint.py`, `scripts/tests/test_plan_lint_formats.py` | `python3 -m unittest discover -s scripts/tests -p 'test_plan_lint_formats.py' -v` |
| H1/02 | [H1/01] | same two files | same |
| H1/03 | [H1/02] | same two files | same |
| H1/04 | [] | `scripts/packet_replay.py`, `scripts/tests/test_packet_replay_formats.py` | `python3 -m unittest discover -s scripts/tests -p 'test_packet_replay_formats.py' -v` |
| H1/05 | [] | `scripts/integrator/accept.sh`, `scripts/integrator/tests/accept_schema.sh` | `bash scripts/integrator/tests/accept_schema.sh` |
| H1/06 | [] | the three C7 docs | `python3 scripts/plan-lint.py --gating` |
| H1/07 | [] | `.sisyphus/plans/N2/packet-00j7a.md` | `git ls-files --error-unmatch .sisyphus/plans/N2/packet-00j7a.md` |
| H1/08 | [] | `.sisyphus/plans/H1/FINDINGS.md` | `test -s .sisyphus/plans/H1/FINDINGS.md` |

Each packet must include `## RED` for its failing test name and `## Приёмка` with the exact command above. `max_diff_lines` is raw and non-authoritative; C2 `src <= 200` / `tests <= 250` decides. No `limit_waiver` is needed; H1/06 has exactly 3 files.

### Dependency matrix

| Todo | Depends on | Blocks | Can parallelize with |
| --- | --- | --- | --- |
| 1. H1/01 | - | 2 | 4, 5, 6, 7, 8 |
| 2. H1/02 | 1 | 3 | - |
| 3. H1/03 | 2 | F1-F4 | - |
| 4. H1/04 | - | F1-F4 | 1, 5, 6, 7, 8 |
| 5. H1/05 | - | F1-F4 | 1, 4, 6, 7, 8 |
| 6. H1/06 | - | F1-F4 | 1, 4, 5, 7, 8 |
| 7. H1/07 | - | F1-F4 | 1, 4, 5, 6, 8 |
| 8. H1/08 | - | F1-F4 | 1, 4, 5, 6, 7 |

## Todos

- [x] 1. H1/01 - teach plan-lint the canonical N2 YAML format
  What to do / Must NOT do: In `scripts/plan-lint.py`, extend `parse()` and the C1/C2/C3 field readers to accept N2's `---` frontmatter, multiline `depends_on` / `allowed_changes` lists, and body sections `## Зачем`, `## RED`, `## Приёмка`. Use stdlib only. `goal` = `## Зачем` first non-empty line, fallback title. `acceptance` = `## RED` when the section exists, else `## Приёмка`. `phase` is RED iff `## RED` exists. Preserve `_folded` detection for C4. Do NOT touch N1/TUI parsing yet, do NOT suppress C3, do NOT normalize any packet.
  Parallelization: Wave 1 | Blocked by: none | Blocks: 2
  References (executor has NO interview context - be exhaustive): `scripts/plan-lint.py:71-113` (`parse`), `:124-140` (`split_list`/RED), `:334-380` (`_collect_packets`/`_deps_of`), `:527-565` (`check` field reads), `.sisyphus/plans/N2/packet-00j1.md:1-110` (canonical shape), `.sisyphus/plans/N2/packet-00i1.md` (no `## Приёмка`), `scripts/tests/plan-lint-parse-fixture.md`.
  Acceptance criteria (agent-executable): `python3 -m unittest discover -s scripts/tests -p 'test_plan_lint_formats.py' -v` is green; the new N2 fixture asserts `parse(.sisyphus/plans/N2/packet-00j1.md)` returns `depends_on=[]`, exactly two `allowed_changes`, `phase != RED`, and acceptance containing `16 passed`; `python3 scripts/plan-lint.py .sisyphus/plans/N2` prints packet counts for N2 instead of `пакетов нет`.
  QA scenarios (name the exact tool + invocation): happy - `python3 scripts/plan-lint.py .sisyphus/plans/N2`; failure - temporary packet with a folded `acceptance: >` still yields C4 and a packet with `## RED` yields C3 when acceptance lacks an inversion marker. Evidence `.omo/evidence/repo-hygiene/H1-01/`.
  Commit: Y | test(plan-lint): parse canonical N2 packets

- [x] 2. H1/02 - teach plan-lint the N1/TUI heading format
  What to do / Must NOT do: Extend the same parser so no-frontmatter packets read bold `**depends_on:**`, bold `**limit_waiver:**`, `## files_allowed` bullets, `## Цель`, `## Приёмка`, and optional `## RED`. `goal` = `## Цель`; files = bullet paths under `## files_allowed`; phase RED only if `## RED` exists or `goal`/`acceptance` starts with `RED`. Keep N2 behavior unchanged. Do NOT rewrite N1/TUI packets.
  Parallelization: Wave 2 | Blocked by: 1 | Blocks: 3
  References: `scripts/plan-lint.py:71-113`, `.sisyphus/plans/N1/packet-01.md:1-95`, `.sisyphus/plans/TUI/packet-01.md:1-100`, `.sisyphus/plans/N1/packet-10.md` (final N1 shape), `scripts/tests/test_plan_lint_formats.py` (from Todo 1).
  Acceptance criteria (agent-executable): `python3 -m unittest discover -s scripts/tests -p 'test_plan_lint_formats.py' -v` green; tests assert N1/01 has 5 files, TUI/01 has 4 files, `depends_on` contains `TUI-00`, and neither is RED; `python3 scripts/plan-lint.py .sisyphus/plans/N1 .sisyphus/plans/TUI` prints counts, not `пакетов нет`.
  QA scenarios: happy - parse N1/01 and TUI/01; failure - a fixture with only a prose `depends_on` sentence must not become a dependency, and a missing `## files_allowed` must yield 0 files, not invent files. Evidence `.omo/evidence/repo-hygiene/H1-02/`.
  Commit: Y | test(plan-lint): parse N1 and TUI packets

- [x] 3. H1/03 - make C9 see current milestones and fix the C7 self-test fixture
  What to do / Must NOT do: Change C9 collection to include every canonical milestone directory except `_archive` (at minimum `M*`, `N1`, `N2`, `TUI`, `H1`), and make `_deps_of` normalize `N2/00j7`, `N2-00j7`, `TUI-00`, `H1/01`, and bare same-milestone ids. Parse files from both `allowed_changes` and `files_allowed` for C9. For C7, reproduce the exact 46/47 failure with a focused test, identify whether the fixture root, `TABLE_DOCS`, or assertion harness is wrong, and fix that root cause; do not simply change 47 to 46 or point the test at real docs. Keep C1/C2/C3 semantics.
  Parallelization: Wave 3 | Blocked by: 2 | Blocks: F1-F4
  References: `scripts/plan-lint.py:262-324` (`check_risk_tables`), `:334-380` (`_collect_packets`/`_deps_of`), `:445-527` (`check_cross_milestone_overlap`), `:612-945` (`self_test`), `:922-943` (C7 fixture), `scripts/plan-lint.py:1040-1065` (C7 main path), `docs/adr/0003-line-budget.md`, `contract.rs` R0-R5.
  Acceptance criteria (agent-executable): `python3 scripts/plan-lint.py --self-test` prints `47/47` and exits 0; a new C9 fixture with `N2` and `H1` both claiming `crates/x/src/tool.py` and no `depends_on` yields C9, and adding `H1/01` to `depends_on: [H1/01]` clears it; `python3 scripts/plan-lint.py` no longer prints `ПРОВЕРЕНО ПУСТЫХ КАТАЛОГОВ` for N1/N2/TUI.
  QA scenarios: happy - self-test 47/47; failure - a fixture where `H1` claims the same `crates/` file as `N2` without order must fail C9; a fixture with C7 docs absent must still fail, not silently pass. Evidence `.omo/evidence/repo-hygiene/H1-03/`.
  Commit: Y | fix(plan-lint): include current milestones and repair C7 self-test

- [x] 4. H1/04 - teach packet_replay current packet formats
  What to do / Must NOT do: Extend `scripts/packet_replay.py` with section-aware parsing: N2 frontmatter `allowed_changes`; N1/TUI bold fields and `## files_allowed`; selected section is `## RED` when present, else `## Приёмка`; command is the first shell command in that section, preferring the first `cargo test`. `inventory` marks `NO_COMMAND` only when no command exists; `validate` returns `UNSUPPORTED_COMMAND` for non-`cargo test` commands; neither is a parse error. Do NOT execute commands, do NOT read `.omo/plans`, do NOT rewrite packets.
  Parallelization: Wave 1 | Blocked by: none | Blocks: F1-F4
  References: `scripts/packet_replay.py:1-170` (`read_fields`, `collect_rows`), `scripts/packet_replay_validate.py`, `.sisyphus/plans/N2/packet-00j1.md`, `.sisyphus/plans/N2/packet-00i1.md`, `.sisyphus/plans/N1/packet-01.md`, `.sisyphus/plans/TUI/packet-01.md`.
  Acceptance criteria (agent-executable): `python3 -m unittest discover -s scripts/tests -p 'test_packet_replay_formats.py' -v` green; `python3 scripts/packet_replay.py inventory` reports `total=50`, `PARSED=49`, `NO_COMMAND=1` (`N2/00i1`), `PARSE_ERROR=0`; `validate` on TUI/01 reports `UNSUPPORTED_COMMAND` and does not crash.
  QA scenarios: happy - inventory over all 50 packets; failure - a packet with `test:` in a fenced example must not become a command; `00i1` must stay `NO_COMMAND`. Evidence `.omo/evidence/repo-hygiene/H1-04/`.
  Commit: Y | feat(packet-replay): read current canonical formats

- [x] 5. H1/05 - make accept.sh read current schema and RED/GREEN honestly
  What to do / Must NOT do: Update `scripts/integrator/accept.sh` to read N2 `allowed_changes` and N1/TUI `## files_allowed`; determine RED by `## RED` presence or legacy `goal`/`acceptance` starting with `RED`; run the first `cargo test` from `## RED` for RED and from `## Приёмка` for GREEN; emit `NO_TEST_COMMAND` when no `cargo test` exists. Keep the existing assert-count and file-limit checks. Add `scripts/integrator/tests/accept_schema.sh` with temporary N2 RED, N2 GREEN, N1, and TUI fixtures. Do NOT execute real product tests, do NOT weaken assertions, do NOT use `eval` on untrusted packet prose.
  Parallelization: Wave 1 | Blocked by: none | Blocks: F1-F4
  References: `scripts/integrator/accept.sh:1-155`, `scripts/integrator/tests.sh`, `.sisyphus/plans/N2/packet-00j1.md` (`## RED`), `.sisyphus/plans/N2/packet-00j7.md` (`## Приёмка`), `.sisyphus/plans/N1/packet-01.md`, `.sisyphus/plans/TUI/packet-01.md`.
  Acceptance criteria (agent-executable): `bash scripts/integrator/tests.sh` passes; `bash scripts/integrator/tests/accept_schema.sh` prints `ACCEPT_SCHEMA_OK` and exits 0; N2 RED fixture is classified RED, N2 GREEN as GREEN, TUI as `NO_TEST_COMMAND`, and a packet with no command never returns 0 as accepted.
  QA scenarios: happy - four fixtures; failure - a RED packet whose command exits 0 must be rejected, and a GREEN packet that prints no `test result: ok.` must be rejected. Evidence `.omo/evidence/repo-hygiene/H1-05/`.
  Commit: Y | fix(integrator): accept current packet schema and explicit no-test state

- [x] 6. H1/06 - restore the three C7 source documents verbatim
  What to do / Must NOT do: Restore `.sisyphus/plans/M1/packet-05.md`, `.sisyphus/plans/M1/packet-06.md`, `.sisyphus/plans/M3/REVIEW.md` from `4e17bb4^` with `git show 4e17bb4^:<path> > <path>`. Do not edit a byte after restore. Do NOT change `contract.rs`, `TABLE_DOCS`, or policy levels.
  Parallelization: Wave 1 | Blocked by: none | Blocks: F1-F4
  References: `scripts/plan-lint.py:164-178` (`TABLE_DOCS`), `:262-324` (`check_risk_tables`), `4e17bb4^` history; `crates/stellwerk-core/src/contract.rs` R0-R5.
  Acceptance criteria (agent-executable): `git show 4e17bb4^:.sisyphus/plans/M1/packet-05.md | cmp - .sisyphus/plans/M1/packet-05.md`; same for packet-06 and M3/REVIEW; `python3 scripts/plan-lint.py --gating` prints `ГЕЙТ: сверка risk_class с contract.rs — совпадает` and exits 0.
  QA scenarios: happy - all three `cmp` commands exit 0; failure - a one-byte edit makes `cmp` fail and C7 must report the mismatch. Evidence `.omo/evidence/repo-hygiene/H1-06/`.
  Commit: Y | docs(plans): restore C7 risk tables from 4e17bb4

- [x] 7. H1/07 - track the completed canonical 00j7a packet
  What to do / Must NOT do: `git add .sisyphus/plans/N2/packet-00j7a.md` only, then commit it. Do NOT add the other 33 untracked canonical packets, N1/index.md, N1/REVIEW.md, or any owner-WIP file. Do NOT modify packet content.
  Parallelization: Wave 1 | Blocked by: none | Blocks: F1-F4
  References: `git ls-files '.sisyphus/plans/**'`, `git log --oneline -5`, `.sisyphus/plans/N2/packet-00j7a.md`, `AGENTS.md` commit rules.
  Acceptance criteria (agent-executable): `git ls-files --error-unmatch .sisyphus/plans/N2/packet-00j7a.md` exits 0; `git status --porcelain` shows no other canonical packet newly staged; `git show --stat --oneline HEAD` contains only `00j7a.md`.
  QA scenarios: happy - the single file is tracked; failure - staging any additional path fails the guard `test "$(git diff --cached --name-only | wc -l)" -eq 1`. Evidence `.omo/evidence/repo-hygiene/H1-07/`.
  Commit: Y | chore(plans): track completed N2/00j7a packet

- [x] 8. H1/08 - record remaining blockers and the exact untracked inventory
  What to do / Must NOT do: Write `.sisyphus/plans/H1/FINDINGS.md` with: (a) the 8 owner-WIP paths and why they are untouched; (b) `just fmt` 12 hunks / 4 files including `http_wire.rs:14` and `model_backend.rs:64`; (c) `just lint` `session.rs:613` `manual_noop_waker`; (d) `audit_live` 6/7 then 7/7 isolated; (e) the exact 33 remaining untracked canonical packets: N1 `packet-01`..`packet-10` (10), N2 `00b,00c,00d,00e,00f,00g1,00g2,00g3,00g3b,00h1,00h2a,00h2b,00h3a,00h3b,00h3c,00h3d,00h3e,00i1` (18), TUI `packet-00`..`packet-04` (5); plus untracked non-packet `N1/index.md`, `N1/REVIEW.md`. Do NOT clean, stage, or edit these paths.
  Parallelization: Wave 1 | Blocked by: none | Blocks: F1-F4
  References: `justfile` fmt/lint/check recipes, `crates/stellwerk-gateway/src/session.rs:613`, `crates/stellwerk-gateway/src/http_wire.rs:14`, `crates/stellwerk-task/tests/model_backend.rs:64`, `crates/stellwerk-cli/tests/audit_live.rs`, `git ls-files --others --exclude-standard '.sisyphus/plans/**'`.
  Acceptance criteria (agent-executable): `test -s .sisyphus/plans/H1/FINDINGS.md`; the file contains exact counts 8, 12, 4, 33 and the paths above; `git status --porcelain` still shows the owner-WIP files unchanged and the 33 packets untracked.
  QA scenarios: happy - findings file exists and counts match; failure - a wrong count or a staged owner-WIP file fails the audit `grep -c '^- \`'` / `git diff --cached`. Evidence `.omo/evidence/repo-hygiene/H1-08/`.
  Commit: Y | docs(plans): record repo-hygiene findings

## Final verification wave

> Runs in parallel after ALL todos. ALL must APPROVE. Surface results and wait for the user's explicit okay before declaring complete.

- [x] F1. Plan compliance audit
  Run `python3 scripts/plan-lint.py --self-test` (expect 47/47), `just plan-lint` (expect 50 packets; C7 0; exit 1 allowed only for recorded C1/C2/C3/C9 findings), `python3 scripts/packet_replay.py inventory` (expect 50 total, 49 PARSED, 1 NO_COMMAND `N2/00i1`), and `python3 scripts/packet_replay.py validate` (expect explicit `UNSUPPORTED_COMMAND` only). Reject any claim based on grep or self-report. Evidence `.omo/evidence/repo-hygiene/F1/`.

- [x] F2. Code quality review
  Run `git diff --check`; `python3 -m unittest discover -s scripts/tests -p 'test_plan_lint_formats.py' -v`; `python3 -m unittest discover -s scripts/tests -p 'test_packet_replay_formats.py' -v`; `bash scripts/integrator/tests.sh`; `bash scripts/integrator/tests/accept_schema.sh`. Reject `#[allow]`, `#[ignore]`, test weakening, or any new dependency. Evidence `.omo/evidence/repo-hygiene/F2/`.

- [x] F3. Real manual QA
  Run `bash scripts/integrator/tests.sh` and `bash scripts/integrator/tests/accept_schema.sh`; execute the accept adapter against a temporary N2 RED fixture and a TUI fixture; verify RED rejection, GREEN acceptance, and `NO_TEST_COMMAND` for TUI. Restore/verify C7 with `python3 scripts/plan-lint.py --gating`; verify `00j7a` with `git ls-files --error-unmatch`. Evidence `.omo/evidence/repo-hygiene/F3/`.

- [x] F4. Scope fidelity
  Compare `git status --porcelain` and `git diff --name-only` against the allowed file list. Confirm no owner-WIP file changed, no product code changed, no root `Cargo.toml` / `contract.rs` / workflows changed, no new dependency, and only `00j7a` was newly tracked. Confirm `.omo` evidence remains untracked. Evidence `.omo/evidence/repo-hygiene/F4/`.

## Commit strategy

- One commit per implementation todo: H1/01, H1/02, H1/03, H1/04, H1/05.
- H1/06: one docs commit with exactly the three restored C7 files.
- H1/07: one chore commit with exactly `.sisyphus/plans/N2/packet-00j7a.md`.
- H1/08: one docs commit with exactly `.sisyphus/plans/H1/FINDINGS.md`.
- Never `git add -A`; never stage owner-WIP, `.omo` evidence, or the other 33 canonical packets.
- Each commit message follows repo style: `test(plan-lint): ...`, `fix(plan-lint): ...`, `feat(packet-replay): ...`, `fix(integrator): ...`, `docs(plans): ...`, `chore(plans): ...`.
- C2 is checked per commit with `scripts/plan-lint.py --budget <commit>`; if src > 200 or tests > 250, stop with `SPLIT_REQUIRED`; do not raise limits.

## Success criteria

- `just doctor` stays green.
- `python3 scripts/plan-lint.py --self-test` is 47/47, exit 0.
- `just plan-lint` parses 50 packets (42 original + 8 H1), emits no `пакетов нет`, C7 0; exit 1 only with a recorded real C1/C2/C3/C9 finding.
- `python3 scripts/packet_replay.py inventory` is 50 total / 49 PARSED / 1 NO_COMMAND (`N2/00i1`) / 0 PARSE_ERROR.
- `python3 scripts/packet_replay.py validate` reports explicit `UNSUPPORTED_COMMAND` only; no silent pass.
- `bash scripts/integrator/tests.sh` and `bash scripts/integrator/tests/accept_schema.sh` pass; TUI yields `NO_TEST_COMMAND`.
- C7 documents are byte-identical to `4e17bb4^`; `--gating` exits 0.
- `00j7a` is tracked; the other 33 canonical packets remain untracked.
- `.sisyphus/plans/H1/FINDINGS.md` records owner-WIP, fmt/lint, `audit_live`, and the exact 33 inventory.
- `git status` shows no new changes to the 8 owner-WIP files; no product code, no `Cargo.toml`, no `contract.rs`, no workflows.
- `just fmt` / `just lint` remain red with the documented blockers; no claim of `just check` green.
