# m0-registry - Work Plan

## TL;DR (For humans)

**What you'll get:** A ready-to-run CLI (`stellwerk`) that can register external tool modules, measure how much context each one costs, check that every advertised tool actually works, and open a safe update pull request — delivered as 12 small, self-contained task briefs the user hands to parallel workers.

**Why this approach:** TDD one-command-at-a-time with a hard 12-packet cap keeps every change reviewable under the repo's ≤3-file/≤200-line rule; a single real-world calibration (813-token reference on polyforge-mcp) replaces an unavailable tokenizer; update is wired through a swappable `gh` binary so tests never touch the network.

**What it will NOT do:** No skill-module auditing, no exact qwen tokenizer, no new dependencies beyond the existing workspace allowlist, no edits to contract.rs / root Cargo.toml / hand-edited lockfile, no merging of PRs.

**Effort:** Medium
**Risk:** Medium - calibration and the rmcp client path depend on one live MCP server (polyforge-mcp) being available at acceptance time.
**Decisions to sanity-check:** (1) calibration lives in modules.toml and is renamed bytes_per_token→tokens_per_byte in packet 07b; (2) the only Cargo.toml edit in M0 is `rmcp = { workspace = true }` in packet 08; (3) without `gh`, update writes a patch to `.stellwerk/pending/` and stops (exit 0) instead of failing.

Your next move: run `just plan-lint`, then Momus + Metis on Nemotron, then hand lanes to `stellwerk-worker-pi` workers. Full execution detail follows below.

---

> TL;DR (machine): Medium/12 TDD packets for stellwerk-registry add|measure|audit|update; Medium risk (live polyforge-mcp calibration + rmcp); sole Cargo.toml touch = rmcp in p08; gh-swappable update.

## Scope
### Must have
- 12 TDD-пакетов в `.sisyphus/plans/M0/packet-{01..11}.md` (07a+07b) по формату `docs/PROMETHEUS_PROMPT.md` (plan_id: M0; goal; files_allowed ≤3; max_diff_lines 200; test; acceptance; forbidden; depends_on).
- Индекс `.sisyphus/plans/M0-index.md` (top-level для `just plan-lint`): таблица packet→lane→depends_on, 12 заголовков `packet`, команды `cargo test`, раздел DEFERRED, слово «калибровка», НЕ содержит строк «cargo add».
- `.omo/plans/m0-registry.md` — этот план, TL;DR последним.
- CRUD-команды: `module add|measure|audit|update` (docs/M0.md:3-11).
- Памятка решений Q1-Q6 и поправок A-D (зафиксированы в `.omo/drafts/m0-registry.md:46-57`).

### Must NOT have (guardrails, anti-slop, scope boundaries)
- Ноль правок: `crates/stellwerk-core/src/contract.rs`, `modules.lock` руками, `.github/workflows/`, корневой `Cargo.toml` (кроме отдельного packet contract-change, в M0 его нет).
- `cargo add`. Единственная правка Cargo.toml в M0: `rmcp = { workspace = true }` в `crates/stellwerk-registry/Cargo.toml`, packet 08.
- Скоуп вне M0 (kanban, hermes, gateway, nag, crm, obsidian, sync, nats, casbin, authentik) — в DEFERRED.
- Исполнение кода во время планирования: я (Prometheus) только пишу плановые артефакты `.omo/` и `.sisyphus/plans/`. Воркеры `stellwerk-worker-pi` запускает пользователь.
- «Рефакторинг заодно», `#[allow(...)]`, `#[ignore]`, skip-тесты, исправление работником RED-теста (поправка A).

## Verification strategy
> Zero human intervention - all verification is agent-executed.
- Test decision: TDD, каждый packet = сначала тест, потом код; RED-пакеты (01,03,06,08,10) только падающий тест + компилируемая заглушка.
- Evidence: `.omo/evidence/m0-registry/`; RED → показать вывод падающего теста; GREEN → показать вывод проходящего; PROOF → evidence_append(ModelClaim) + evidence_verify(cmd=<точная команда>); GATE → gate_evaluate (fail → не отчитываться об успехе).
- Приёмка плана: `just plan-lint` (scripts/plan-lint.sh), затем ревью Momus и Metis (модель nemotron-3-ultra-free).

## Execution strategy
> Воркеры (stellwerk-worker-pi) видят ТОЛЬКО свой packet. Lane = одна ветка (worktree). RED и GREEN одного lane — последовательно в одной ветке; merge в main только после GREEN (для lane-update — после packet 11). Параллельность между lanes, не между соседними packets одного lane.

### Parallel execution waves
> Waves = lanes (поправка B). Каждый packet — свой todo (TDD: тест внутри packet).
- Wave 1: lane-model — p01 (RED) → p02 (GREEN)
- Wave 2: lane-cli — p03 (RED) → p04 (GREEN)
- Wave 3 (3 параллельные ветки, до 3 воркеров): lane-add — p05 | lane-measure — p06 → p07a → p07b | lane-audit — p08 → p09
- Wave 4: lane-update — p10 (RED) → p11 (GREEN), после слияния 07b и 09
- F-wave (после всех): plan-lint + Momus/Metis + ручная M0-приёмка

### Dependency matrix
| Todo | Depends on | Blocks | Can parallelize with |
| --- | --- | --- | --- |
| p01 model RED | — | p02 | — |
| p02 model GREEN | p01 | p03 | — |
| p03 cli RED | p02 | p04 | — |
| p04 cli GREEN | p03 | p05,p06,p08 | — |
| p05 add (RED+GREEN) | p04 | p10 | p06, p08 |
| p06 measure RED | p04 | p07a | p05, p08 |
| p07a measure GREEN | p06 | p07b | — |
| p07b calibration acceptance | p07a | p10 | — |
| p08 audit RED (+rmcp) | p04 | p09 | p05, p06 |
| p09 audit GREEN | p08 | p10 | — |
| p10 update RED | p07b, p09 | p11 | — |
| p11 update GREEN | p10 | — (merge после 11) | — |

## Todos
> Implementation + Test = ONE todo. Каждый packet — todo. Пакеты самодостаточны (воркер не читает план).
- [ ] 1. p01 — model RED: типы Module/Source/Calibration + формула tokens=round(bytes×tokens_per_byte); заглушка компилируется, тесты падают на assert
- [ ] 2. p02 — model GREEN: поведение реализовано, `cargo test -p stellwerk-registry model` зелёный
- [ ] 3. p03 — cli RED: clap-скелет (module add|measure|audit|update), тесты разбора падают
- [ ] 4. p04 — cli GREEN: разбор + диспетчеризация на stubs команд, тесты зелёные
- [ ] 5. p05 — add: append-блок в modules.toml, валидация source, дубликат → ошибка; RED→GREEN в одном packet
- [ ] 6. p06 — measure RED: тесты падают (fake MCP-сервер, bytes→tokens, пиннинг ≈813±10%)
- [ ] 7. p07a — measure GREEN: tools/list → байты схем → tokens → запись в modules.lock (tempdir)
- [ ] 8. p07b — calibration acceptance: реальный polyforge-mcp → 813±10%, rename bytes_per_token→tokens_per_byte, measured_at, modules.lock
- [ ] 9. p08 — audit RED: Cargo.toml crate + rmcp (единственная Cargo-правка M0), тесты audit падают
- [ ] 10. p09 — audit GREEN: spawn kind=mcp, tools/list, probe каждого тула (invalid args, 10s), сверка с manifest
- [ ] 11. p10 — update RED: git fetch→self build→audit→measure→gh pr create через STELLWERK_GH_BIN; no-gh → .stellwerk/pending/
- [ ] 12. p11 — update GREEN: полный flow, тесты зелёные (реальный gh только в ручной приёмке)

## Final verification wave
> Runs in parallel after ALL todos. ALL must APPROVE.
- [ ] F1. `just plan-lint` (scripts/plan-lint.sh) — OK: план прошёл lint
- [ ] F2. Momus (nemotron-3-ultra-free) — APPROVE; учесть замечания в пакетах
- [ ] F3. Metis (nemotron-3-ultra-free) — APPROVE; учесть замечания в пакетах
- [ ] F4. Ручная M0-приёмка (реальный polyforge-mcp, 813±10%; реальный `gh` НЕ вызывается в автотестах)

## Commit strategy
- Ветки по lanes: `lane-model`, `lane-cli`, `lane-add`, `lane-measure`, `lane-audit`, `lane-update`.
- RED и GREEN одного lane — одна ветка; commit после GREEN каждого packet; merge в main после GREEN (update-lane — после p11). RED без пары не мержится (иначе ломает CI: `cargo test --workspace`).
- Правки только внутри `files_allowed` каждого packet; `modules.lock` руками запрещён (исключение: 07b — калибровочная запись).

## Success criteria
- `just plan-lint` → "OK: план прошёл lint"; ровно 12 packets, формат строго по docs/PROMETHEUS_PROMPT.md (≤3 файла, ≤200 строк, ≤4k токенов, точный фильтр test).
- Momus + Metis → APPROVE.
- Пакеты самодостаточны: воркер выполняет packet без чтения полного плана; Telegram-канал ролей не нужен.
- Lanes готовы к раздаче воркерам stellwerk-worker-pi.
