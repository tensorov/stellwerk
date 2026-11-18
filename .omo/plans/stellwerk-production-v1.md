# stellwerk-production-v1 - Work Plan

## TL;DR (For humans)

**What you'll get:** Stellwerk перестаёт быть системой, которая «гейтит агента по имени инструмента», и становится системой, которая исполняет авторизованный вызов сама: типизированный запрос с digest содержимого, решение гейта, применение хостом, кандидат коммита, цепочка evidence, которую нельзя собрать из своих же слов. Плюс приведённый в зелёное тулчейн и закрытый долг линта плана.

**Why this approach:** Две несущие решения. Первое — эффект инструмента переезжает в хост (extension-инструменты Pi вместо встроенных `edit`/`write`), потому что иначе «аргументы не изменились после авторизации» недоказуемо в принципе: Pi исполняет эффект сам и хост его не видит. Второе — новые состояния (`AwaitingHumanValidation`, `HumanValidated`, `Refuted`, `Cancelled`) входят в `contract.rs` явными вариантами, а не выводятся из порядка evidence, потому что закон 6 запрещает неизвестному превращаться в значение.

**What it will NOT do:** Не добавляет зависимостей, не использует `git worktree`/`gix`, не вводит второй профиль исполнения, не прячет 77 нарушений линта за `--gating`, не ослабляет гейты.

**Effort:** XL
**Risk:** High - три внешних стопа (Pi 1.0.2, `registerTool`-семантика, схема PolyForge) и один незакоммиченный owner-WIP блокируют часть приёмок.
**Decisions to sanity-check:** (1) хост исполняет эффект — меняет контракт воркера; (2) `contract.rs` расширяется пакетом `contract-change` + ADR; (3) P01 не чинит линт, линт уходит в отдельный L01.

Your next move: approve and run `$start-work`. Full execution detail follows below.

---

> TL;DR (machine): XL / High; 29 canonical packets + 1 materialization step; host-executed authorized calls, contract extension, toolchain green, lint debt closed.

## Scope

### Must have

- Измеренная база P00, зафиксированная документом с тестом-стражем.
- Зелёный тулчейн: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test` по шести крейтам.
- Один профиль исполнения `pi`; ADR, отменяющий ADR-0015.
- `contract.rs`: 4 новых варианта `TaskStatus` и типизированные bindings evidence — пакетом `contract-change` + ADR.
- Типизированный `RequestedCall` с digest содержимого; `AuthorizedCall`, который исполняет только хост.
- Extension-инструменты Pi вместо встроенных `edit`/`write`; встроенные не предлагаются.
- Удалённый fixture-путь из `stellwerk run` и TUI.
- Кандидат коммита из workspace по digest; отказ при грязном дереве.
- Порядок evidence: `ModelClaim` → `ToolAttestation` → `Validation`; ни один шаг не выводится из собственного текста модели.
- Вертикальные доказательства V01–V04, CI-гейт I01–I02, состояния S01–S03, квартир Q01–Q02.
- `just plan-lint` зелёный на всём каноне.

### Must NOT have (guardrails, anti-slop, scope boundaries)

- `git worktree`, `gix`, любая новая зависимость, `cargo add`.
- opencode как воркер; второй execution-профиль; `fixture-engine` в `ALLOWED_PROFILES`.
- Правка `crates/stellwerk-core/src/contract.rs` вне пакета `03` (метка `contract-change` + ADR).
- `.omo/plans/` как источник пакетов: канон — `.sisyphus/plans/V1/`.
- `--gating`, `#[allow(...)]`, `#[ignore]`, `continue-on-error`, `unwrap_or(R0)`, `else { Allow }`, `Ask` без подтверждения.
- Пакет без нового теста; приёмка командой, не относящейся к пакету.
- Объявление гейта пройденным без `gate_evaluate`.

## Verification strategy

- Test decision: **TDD** — каждый пакет добавляет падающий тест, затем минимальный код, затем зелёный прогон. Rust `cargo test`, шесть крейтов workspace.
- Приёмка пакета: `scripts/integrator/accept.sh V1 packet-<id> <sandbox>` (ждёт `cargo test` в `## Приёмка`; RED-пакет — в `## RED`; `limit_waiver` снимает лимит файлов).
- Бюджет пакета: C2 считает **добавленные** строки — src ≤ 200 логических (без пустых и `//`), tests ≤ 250 полных. Не влез — вернуть `SPLIT_REQUIRED` и разделить; потолки не поднимать (только `contract-change` + ADR).
- Приёмка плана: `python3 scripts/plan-lint.py .sisyphus/plans/V1` проверяет C1–C6 по V1, но C7, C8 и C9 считаются глобально. Поэтому «exit 0» достижим только после L01. До L01 критерий — строка `V1: 32 пакетов, нарушений нет` **и** `ПЕРЕСЕЧЕНИЯ МЕЖДУ MILESTONE: порядок объявлен везде`. Счёт 33, а не 29, потому что владелец 2026-11-02 добавил `V1/packet-04.md` (остаточный fmt после R01/A01/A03), `V1/packet-a01b.md` (типизированный `RequestedCall` с digest) и `V1/packet-a02b.md` (хост исполняет авторизованный edit), а оркестратор — `V1/packet-v02b.md` (Pi регистрирует удерживаемый `bash`, не предлагая его модели); пакет 01 остаётся с шестью путями.
- Приёмка канона: `python3 scripts/packet_replay.py validate` → exit 0.
- Evidence: `.omo/evidence/ulw/<session>/<goalId>/a<attempt>/task-<N>-stellwerk-production-v1.<ext>`; для пакетов — PolyForge `evidence_append(ModelClaim)` + `evidence_verify` ПОСЛЕ merge в main (вариант B из AGENTS.md).
- Zero human intervention: все проверки исполняет агент.

## Execution strategy

### Parallel execution waves

> Волны здесь — **ранг зависимости, а не параллельность**. AGENTS.md требует строго
> последовательного исполнения в одном worktree, иначе `overlap_waiver: C9` недействителен.
> Исполнять строго по возрастанию номера todo.

- Wave 1: 00
- Wave 2: 01, 02, l01
- Wave 3: 03, r01, r02
- Wave 4: a01
- Wave 5: a03
- Wave 6: r03
- Wave 7: a02, r04
- Wave 8: c01, v01
- Wave 9: c02
- Wave 10: v02, o01
- Wave 11: o02, v03
- Wave 12: o03, i01
- Wave 13: o04, i02
- Wave 14: v04
- Wave 15: s01
- Wave 16: s02
- Wave 17: s03
- Wave 18: q01
- Wave 19: q02

### Dependency matrix

| Todo | Depends on | Blocks | Can parallelize with |
| --- | --- | --- | --- |
| 00 | — | 01, 02, l01 | — |
| 01 | 00 | r01 | 02, l01 |
| 02 | 00 | 03, r02 | 01, l01 |
| 03 | 02 | a01, c01, o01 | r01 |
| l01 | 00 | — | 01, 02 |
| r01 | 01 | r02, r03 | 03 |
| r02 | 02, 01 | r03 | — |
| a01 | 03 | a03, a02, r03 | — |
| a03 | a01 | r03 | — |
| r03 | r01, r02, a01, a03 | a02, r04 | — |
| a02 | a01, r03 | c01, v01 | r04 |
| r04 | r03 | v01 | a02 |
| c01 | 03, a02 | c02 | v01 |
| c02 | c01 | v02, o01 | — |
| v01 | a02, r04 | — | c01 |
| v02 | c02 | v03 | o01 |
| o01 | 03, c02 | o02, v03 | v02 |
| o02 | o01 | o03, v04 | v03 |
| o03 | o02 | o04, v04 | i01 |
| o04 | o03 | v04 | i02 |
| v03 | o01, v02 | v04, i01 | o02 |
| v04 | o02, o03, o04, v03 | s01 | — |
| i01 | v03 | i02 | o03 |
| i02 | i01 | s01 | o04 |
| s01 | v04, i02 | s02 | — |
| s02 | s01 | s03 | — |
| s03 | s02 | q01 | — |
| q01 | s03 | q02 | — |
| q02 | q01 | — | — |

## Canonical packet field table

> Источник для todo 1. `depends_on` — короткие id; `plan-lint.norm_dep` разворачивает их
> в `packet-<id>`. Файлы — ровно те, что вправе менять пакет.

| Packet | depends_on | files_allowed | acceptance | waiver |
| --- | --- | --- | --- | --- |
| 00 | [] | `docs/p00-baseline.md`, `crates/stellwerk-cli/tests/p00_baseline.rs` | `cargo test -p stellwerk-cli --test p00_baseline` | — |
| 01 | [00] | `crates/stellwerk-gateway/src/session.rs`, `crates/stellwerk-gateway/tests/session.rs`, `crates/stellwerk-gateway/tests/session_failure.rs`, `crates/stellwerk-gateway/tests/shaping.rs`, `crates/stellwerk-task/tests/http_wire.rs`, `crates/stellwerk-task/tests/model_backend.rs` | `cargo test -p stellwerk-gateway --test session` | fmt/clippy — механические правки в 6 файлах |
| 02 | [00] | `docs/adr/0021-single-execution-profile.md`, `crates/stellwerk-task/src/execution.rs`, `crates/stellwerk-task/tests/execution.rs` | `cargo test -p stellwerk-task --test execution` | — |
| 03 | [02] | `crates/stellwerk-core/src/contract.rs`, `docs/adr/0022-task-status-human-and-refutation.md`, `crates/stellwerk-core/tests/task_lifecycle.rs` | `cargo test -p stellwerk-core --test task_lifecycle` | — |
| l01 | [00] | 34 пути — точный список раскрыт в todo 6 (без сокращений `-02`) | `cargo test -p stellwerk-cli --test plan_lint` | 34 файла, правка только плана: 0 src / 0 test строк по C2 |
| r01 | [01] | `crates/stellwerk-cli/src/lib.rs`, `crates/stellwerk-cli/src/main.rs`, `crates/stellwerk-cli/src/plan.rs`, `crates/stellwerk-cli/src/build_runner.rs`, `crates/stellwerk-cli/tests/run.rs` | `cargo test -p stellwerk-cli --test run` | 5 файлов: один путь исполнения; `run_build` больше не зовёт `worker.exec` напрямую |
| r02 | [02, 01] | `crates/stellwerk-task/src/taskfile.rs`, `crates/stellwerk-task/src/execution.rs`, `crates/stellwerk-task/tests/taskfile.rs` | `cargo test -p stellwerk-task --test taskfile` | — |
| a01 | [03] | `crates/stellwerk-gateway/src/gate.rs`, `crates/stellwerk-task/src/m5/gate_channel.rs`, `crates/stellwerk-gateway/tests/gate.rs` | `cargo test -p stellwerk-gateway --test gate` | — |
| a03 | [a01] | `crates/stellwerk-task/src/orchestrator.rs`, `crates/stellwerk-task/tests/authorization.rs` | `cargo test -p stellwerk-task --test authorization` | — |
| r03 | [r01, r02, a01, a03] | `crates/stellwerk-task/pi/stellwerk_transport.ts`, `crates/stellwerk-task/src/orchestrator.rs`, `crates/stellwerk-task/tests/pi_transport.rs` | `cargo test -p stellwerk-task --test pi_transport` | — |
| a02 | [a01, r03] | `crates/stellwerk-task/src/m5/gate_channel.rs`, `crates/stellwerk-cli/src/build_runner.rs`, `crates/stellwerk-cli/tests/host_tools.rs` | `cargo test -p stellwerk-cli --test host_tools` | — |
| r04 | [r03] | `crates/stellwerk-cli/src/lib.rs`, `crates/stellwerk-cli/src/tui.rs`, `crates/stellwerk-cli/tests/run.rs` | `cargo test -p stellwerk-cli --test run` | — |
| c01 | [03, a02] | `crates/stellwerk-cli/src/build_runner.rs`, `crates/stellwerk-task/src/m5/sandbox_bwrap.rs`, `crates/stellwerk-cli/tests/build_runner.rs` | `cargo test -p stellwerk-cli --test build_runner` | — |
| c02 | [c01] | `crates/stellwerk-cli/src/build_runner.rs`, `crates/stellwerk-cli/tests/candidate.rs` | `cargo test -p stellwerk-cli --test candidate` | — |
| v01 | [a02, r04] | `crates/stellwerk-cli/tests/vertical_v1.rs` | `cargo test -p stellwerk-cli --test vertical_v1` | — |
| v02 | [c02] | `crates/stellwerk-cli/tests/vertical_v2.rs` | `cargo test -p stellwerk-cli --test vertical_v2` | — |
| o01 | [03, c02] | `crates/stellwerk-task/src/evidence.rs`, `crates/stellwerk-task/tests/evidence.rs` | `cargo test -p stellwerk-task --test evidence` | — |
| o02 | [o01] | `crates/stellwerk-task/src/runner.rs`, `crates/stellwerk-task/tests/runner.rs` | `cargo test -p stellwerk-task --test runner` | — |
| o03 | [o02] | `crates/stellwerk-task/src/evidence.rs`, `crates/stellwerk-task/tests/human_validation.rs` | `cargo test -p stellwerk-task --test human_validation` | — |
| o04 | [o03] | `crates/stellwerk-task/src/evidence.rs`, `crates/stellwerk-task/tests/refutation.rs` | `cargo test -p stellwerk-task --test refutation` | — |
| v03 | [o01, v02] | `crates/stellwerk-cli/tests/vertical_v3.rs` | `cargo test -p stellwerk-cli --test vertical_v3` | — |
| v04 | [o02, o03, o04, v03] | `crates/stellwerk-cli/tests/vertical_v4.rs` | `cargo test -p stellwerk-cli --test vertical_v4` | — |
| i01 | [v03] | `.github/workflows/polyforge.yml`, `scripts/integrator/gate.sh`, `crates/stellwerk-cli/tests/ci_gate.rs` | `cargo test -p stellwerk-cli --test ci_gate` | — |
| i02 | [i01] | `.github/workflows/polyforge.yml`, `crates/stellwerk-cli/tests/ci_gate.rs` | `cargo test -p stellwerk-cli --test ci_gate` | — |
| s01 | [v04, i02] | `crates/stellwerk-task/src/runner.rs`, `crates/stellwerk-task/tests/suspension.rs` | `cargo test -p stellwerk-task --test suspension` | — |
| s02 | [s01] | `crates/stellwerk-task/src/runner.rs`, `crates/stellwerk-task/tests/human_validation.rs` | `cargo test -p stellwerk-task --test human_validation` | — |
| s03 | [s02] | `crates/stellwerk-task/src/runner.rs`, `crates/stellwerk-task/tests/refutation.rs` | `cargo test -p stellwerk-task --test refutation` | — |
| q01 | [s03] | `crates/stellwerk-task/src/runner.rs`, `crates/stellwerk-task/tests/quarantine.rs` | `cargo test -p stellwerk-task --test quarantine` | — |
| q02 | [q01] | `crates/stellwerk-task/src/runner.rs`, `crates/stellwerk-task/tests/quarantine.rs` | `cargo test -p stellwerk-task --test quarantine` | — |

### Cross-milestone waivers (C9)

> 21 пакет V1 заново берёт `crates/`-файлы, которыми владеют закрытые milestone
> N1, N2, TUI и M1. `check_cross_milestone_overlap` иначе печатает C9 и валит
> `just plan-lint`. Waiver разрешён AGENTS.md при двух условиях одновременно:
> (1) пакет — рефакторинг или усиление тестов, (2) исполнение строго
> последовательно в одном worktree. Оба условия держит этот план (см.
> «Parallel execution waves»). Строка в пакете:
> `overlap_waiver: C9 — <пересечение>, последовательное исполнение`.

| Packet | overlap_waiver |
| --- | --- |
| 01 | N2/00g3, N2/00g3b, N2/00j4, N2/00j5, N2/00j7a — общие тестовые файлы |
| 02 | N1/02 — `execution.rs`, `tests/execution.rs` |
| 03 | M1/05, M1/06 — `contract.rs` |
| r01 | N1/01, N1/10, N2/00h1, N2/00h2a, N2/00h2b, N2/00h3b, TUI/02, TUI/03 |
| r02 | N1/02 — `execution.rs` |
| a01 | N2/00e, N2/00i1 — `gate_channel.rs` |
| a03 | N1/03, N1/05, N1/06, N1/09, N2/00c, N2/00f — `orchestrator.rs`, `tests/authorization.rs` |
| r03 | N1/03, N1/05, N1/06, N1/09, N2/00c, N2/00f, N2/00i1, N2/00j6 |
| a02 | N2/00e, N2/00i1, N2/00h3b, N2/00h3c, N2/00h3d, N2/00j1 |
| r04 | N1/01, N2/00h1, N2/00h2a, N2/00h3b, TUI/02, TUI/03 |
| c01 | N2/00c, N2/00h3b, N2/00h3c, N2/00h3d, N2/00j1, N2/00j6 |
| c02 | N2/00h3b, N2/00h3c, N2/00h3d, N2/00j1 |
| o01 | N1/07 — `evidence.rs` |
| o02 | N1/04, N2/00b — `runner.rs` |
| o03 | N1/07 — `evidence.rs` |
| o04 | N1/07 — `evidence.rs` |
| s01 | N1/04, N2/00b — `runner.rs` |
| s02 | N1/04, N2/00b — `runner.rs` |
| s03 | N1/04, N2/00b — `runner.rs` |
| q01 | N1/04, N2/00b — `runner.rs` |
| q02 | N1/04, N2/00b — `runner.rs` |

> Проверка списка при материализации: `python3 scripts/plan-lint.py` печатает
> `ПЕРЕСЕЧЕНИЯ МЕЖДУ MILESTONE: 23` (только исторические N1↔N2↔TUI, их закрывает
> L01). Любая строка `[C9]` с `V1/` — недописанный waiver, а не «шум линта».

## Canonical packet template

> Todo 1 пишет эти 29 файлов в `.sisyphus/plans/V1/packet-<id>.md` ДОСЛОВНО по этому
> шаблону. Прометеус не вправе писать в `.sisyphus/` (политика `[prometheus-md-only]`),
> поэтому материализация канона — первый шаг воркера, а не артефакт планировщика.
> Поле `status` — `GREEN`; секции `## RED` НЕТ (иначе `accept.sh` ждёт падения).
> Строка `cargo test` в `## Приёмка` обязана начинаться с первого столбца.

```
---
id: <id>
milestone: V1
status: GREEN
depends_on: [<deps>]
files_allowed: [<files>]
limit_waiver: <только для 01, l01, r01>
overlap_waiver: <только для 21 пакета из таблицы «Cross-milestone waivers (C9)»>
acceptance: <acceptance>
---

# <id> — <title>

## Цель
<одна фраза: какое поведение появляется и почему его нельзя подделать>

## Обоснование (измерено)
- <path:line> — <факт>

## Шаг 1 — падающий тест
<файл теста>, <имя теста>, что он утверждает и почему падает до правки>

## Приёмка
cargo test <...>
Ожидается PASS.

## Запрещено
- <2–3 запрета, специфичных пакету>

> В `## Запрещено` писать только запреты этого пакета, формулируя их через
> «нельзя/запрещено». НЕ копировать сюда формулировки из «Must NOT have» этого
> плана: литералы отозванных решений (`R4`→Deny при любом уровне, `R3`=`L4Admin`,
> `risk.unwrap_or(R0)`, `Option<RiskClass>`, `Ask` без подтверждения = Deny)
> C5 ловит как живое указание, даже когда строка задумана как запрет.
```

## Todos

> Implementation + Test = ONE todo. Never separate.
> Исполнять строго последовательно по номеру: один worktree, `overlap_waiver: C9`
> действителен только при последовательном исполнении.

- [x] 1. Материализовать 29 канонических пакетов в `.sisyphus/plans/V1/`
  What to do / Must NOT do: создать 29 пакетов `packet-{00,01,02,03,l01,r01,r02,a01,a03,r03,a02,r04,c01,c02,v01,v02,o01,o02,o03,o04,v03,v04,i01,i02,s01,s02,s03,q01,q02}.md` по таблице «Canonical packet field table», таблице «Cross-milestone waivers (C9)» и шаблону ниже; в каждом — цель, измеренное обоснование, шаг падающего теста, `## Приёмка` с командой из таблицы, запреты. `overlap_waiver` пишется ровно 21 пакету из таблицы C9, дословно `overlap_waiver: C9 — <пересечение>, последовательное исполнение`. НЕ создавать `.omo/plans/V1/`; НЕ писать секцию `## RED`; НЕ добавлять поля сверх шаблона; НЕ переносить в `## Запрещено` литералы отозванных решений (C5).
  Parallelization: Wave 0 | Blocked by: — | Blocks: все
  References (executor has NO interview context - be exhaustive): `scripts/plan-lint.py:273-300` (C5-литералы), `scripts/plan-lint.py:465` (`WAIVER_RE`), `scripts/plan-lint.py:523-598` (C9 и waiver), `scripts/integrator/accept.sh:32-53,95-135` (поля и секции), `AGENTS.md` (канон пакетов, C9-waiver), `/home/izyabretatel/gits/stellwerk/.omo/plans/stellwerk-production-v1.md` (таблицы полей и C9).
  Acceptance criteria (agent-executable): `python3 scripts/plan-lint.py .sisyphus/plans/V1` содержит строку `V1: 29 пакетов, нарушений нет`; в том же выводе `ПЕРЕСЕЧЕНИЯ МЕЖДУ MILESTONE: 23` (ни одного нового; все пары с `V1/` закрыты waiver); `python3 scripts/packet_replay.py inventory` показывает 29 новых PARSED и 0 PARSE_ERROR в V1. Exit-код линта на этом шаге — всё ещё 1 из-за 23 исторических C9: это ожидаемо до L01, зелёным его делает L01. Не выдавать этот exit 1 за успех и не выдавать его за провал.
  QA scenarios (name the exact tool + invocation): happy — `python3 scripts/packet_replay.py validate` exit 0; failure — удалить `overlap_waiver` у `packet-01`, ожидать строку `[C9] ... V1/packet-01`, вернуть. Evidence `.omo/evidence/task-1-stellwerk-production-v1.txt`
  Commit: N | план-артефакт, коммитится вместе с первым исполняемым пакетом

- [x] 2. P00: зафиксировать измеренную базу документом и тестом-стражем
  What to do / Must NOT do: создать `docs/p00-baseline.md` с шестью измеренными командами и их exit-кодами (`fmt` 1, `clippy` 101, `cargo test` 0/650 (в плане ранее стояло 0/99 — измеренная сумма по шести крейтам 650, зафиксировано в документе как расхождение), `plan-lint` 1/77, `replay inventory` 0/50-49-1, `doctor` 0), перечнем шести крейтов и версией Pi 0.87.0; тест `p00_baseline` требует по строке на каждую команду. НЕ вписывать числа, которых нет в пакете; НЕ объявлять линт зелёным.
  Parallelization: Wave 1 | Blocked by: todo 1 | Blocks: 01, 02, l01
  References: `/tmp/opencode/p00/` (fmt.txt, clippy.txt, planlint-full.txt, replay-inventory.txt, test-exits.txt), `justfile`, `docs/PI_1_0_2_PROBE.md`.
  Acceptance criteria: `cargo test -p stellwerk-cli --test p00_baseline` PASS.
  QA scenarios: happy — тест зелёный; failure — убрать строку про `plan-lint` из документа, ожидать падение теста. Evidence `.omo/evidence/task-2-stellwerk-production-v1.txt`
  Commit: Y | `docs(p00): record measured baseline with guard test`

- [x] 3. P01: привести fmt и clippy в зелёное — closed: clippy attested (claim 310 / attestation 311); fmt green on clean checkout of 33e6ea6 and attested via V1/packet-04 (claim 313 / attestation 314, gate_evaluate passed:true, tail a0b1ad32…)
  What to do / Must NOT do: отформатировать 4 файла и заменить ручной no-op waker на `Waker::noop()` в `crates/stellwerk-gateway/src/session.rs:613`; добавить тест в `crates/stellwerk-gateway/tests/session.rs`, что пробуждённая задача продолжает исполнение после замены waker. Стоп-условие: `git status --porcelain -- crates/stellwerk-gateway/tests/session_failure.rs crates/stellwerk-gateway/tests/shaping.rs` печатает хотя бы одну строку — остановиться и вернуть `BLOCKED_BY_OWNER_WIP`; эти два файла не перезаписывать, `git checkout --` по ним запрещён. НЕ трогать `contract.rs`; НЕ добавлять `#[allow]`; НЕ менять lint-политику.
  Parallelization: Wave 2 | Blocked by: 00 | Blocks: r01
  References: `crates/stellwerk-gateway/src/session.rs:613`, `crates/stellwerk-task/tests/http_wire.rs`, `crates/stellwerk-task/tests/model_backend.rs`, `/tmp/opencode/p00/fmt.txt`, `/tmp/opencode/p00/clippy.txt`.
  Acceptance criteria: `cargo fmt --all -- --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0; `cargo test -p stellwerk-gateway --test session` PASS.
  QA scenarios: happy — обе команды exit 0; failure — вернуть ручной waker, ожидать clippy error. Evidence `.omo/evidence/task-3-stellwerk-production-v1.txt`
  Commit: Y | `fix(gateway): replace manual noop waker and format tree`
  Прогресс (2026-11-02, измерено): **clippy-half СДАН** — `drive()` в `gateway/src/session.rs` держит `Unpark(std::thread::Thread)` + `park()` вместо пустого waker + `yield_now`; тест `woken_task_continues_after_waker_replacement` (канонное имя из `.sisyphus/plans/V1/packet-01.md:22`) в `gateway/tests/session.rs` с 5-секундным стражем и границей ≤100 опросов. Закоммичено — `2b35575` (parent `fea4fa7`, 3 файла, +238/−7). Проверено на коммите: `cargo clippy --workspace --all-targets -- -D warnings` exit 0; `--test session` 5 passed; `--test session_failure` 18 passed (0.07s, без зависания); `cargo fmt --all -- --check` по двум файлам — чисто. **fmt-half: 4 из 4 файлов, которые пакет вправе трогать, теперь чисты** — `http_wire.rs` и `model_backend.rs` отформатированы (разбита одна цепочка `accept_within`, +6/−2) и закоммичены `882e3b5` (parent `2b35575`); проверено на коммите: `cargo fmt --all -- --check` по этим двум файлам молчит, `--test http_wire` 5 passed, `--test model_backend` 18 passed, workspace clippy exit 0, 8 хешей файлов владельца совпали. **Todo 3 остаётся `[~]`, потому что acceptance недостижим в рамках пакета**: общий `fmt` exit 1 на 7 файлах, которых нет в `files_allowed` — 2 файла владельца (`session_failure.rs`, `shaping.rs`) и 5 файлов уже влитых пакетов (`cli/tests/run.rs` R01; `gateway/src/gate.rs`, `gateway/tests/gate.rs`, `task/src/m5/gate_channel.rs` A01; `task/tests/authorization.rs` A03). Это не недоделка реализации, а дефект области пакета: нужен либо owner-коммит двух файлов плюс расширение `files_allowed`/`limit_waiver`, либо отдельный style-пакет на 5 файлов с `overlap_waiver: C9`. FINDING записан в `.omo/notepads/stellwerk-production-v1/issues.md`; самовольно те 5 файлов не форматировал — это выход за `limit_waiver`. Отклонение от буквы пакета записано как FINDING: `Waker::noop()` не планирует ничего, поэтому с `park()` даёт вечный сон, а с `yield_now()` — ровно тот busy-poll, который пакет и называет дефектом; буква пакета несовместима с его же шагом 1. Ловушка зафиксирована в evidence: per-file `rustfmt --check` ≠ `cargo fmt` (дал false green на переупорядоченных импортах).

  - [x] 3a. P01-fmt: остаточный fmt после R01/A01/A03 — `V1/packet-04` (решение владельца 2026-11-02) — merged 33e6ea6; accept.sh V1 packet-04 ПРИНЯТО (8 passed); attestation 314 Verified; gate_evaluate passed:true. Это подпункт задачи 3, не отдельная нумерованная задача.
  Why: `packet-01` вправе трогать только 6 своих путей, а `cargo fmt --all -- --check` остаётся exit 1 на 5 файлах уже влитых пакетов. Расширение `packet-01` до 11 файлов владелец **отклонил**; вместо этого создан отдельный механический пакет.
  Packet: `.sisyphus/plans/V1/packet-04.md` (канон) = `.omo/plans/V1/packet-04.md` (зеркало, байт-в-байт). `files_allowed` ровно 5: `cli/tests/run.rs` (R01), `gateway/src/gate.rs` + `gateway/tests/gate.rs` + `task/src/m5/gate_channel.rs` (A01), `task/tests/authorization.rs` (A03). `depends_on: [r01, r04, a01, a02, a03, v02, o02]` — это и есть связь для C1 внутри V1; `overlap_waiver: C9` покрывает N1/packet-01, N1/packet-09, N2/packet-00e, N2/packet-00i1, TUI/packet-02; `limit_waiver` на 5 файлов.
  Acceptance: `cargo test -p stellwerk-task --test authorization` (8 passed: 7 существующих + `workspace_is_rustfmt_clean`), внутри теста — `cargo fmt --all -- --check`; плюс `cargo clippy -p stellwerk-task --all-targets -- -D warnings`. Приёмка не может быть самим `cargo fmt`: `accept.sh:166-169` требует `cargo test` в `## Приёмка`, а PolyForge не пропускает fmt в allowlist (проба claim 312). Цель `authorization.rs`, а не `run.rs`, потому что бинарь `stellwerk-cli` на `882e3b5` не собирается (TUI-02 влит без `struct TuiArgs`), и `--test run` был бы красным не из-за формата. Никаких изменений логики, `rustfmt.toml`, `#[rustfmt::skip]`, `#[allow]`.
  Стоп-условие: `git status --porcelain` по 8 путям владельца непуст → `BLOCKED_BY_OWNER_WIP`, файлы владельца не трогать. Порядок владельца: 3a закрывается ДО R02, после подтверждения чистого дерева.
  Состояние линта на 2026-11-02 (измерено, до исполнения пакета): `plan-lint.py` exit 0; scoped `V1: 33 пакетов, нарушений нет`; `[C1]`/`[C2]`/`[C9]` 0; глобально `ПЕРЕСЕЧЕНИЯ МЕЖДУ MILESTONE: порядок объявлен везде`; остались только предсуществующие `[C8]` по N2. `ПЕРЕЗАПУСК F1 обязателен` из-за счёта 29→30.
  Прогресс (2026-11-02, измерено): пакет исполнен **в изолированном worktree** `.worktrees/p01-fmt-04` (ветка `v1-p04-fmt` от `882e3b5`), потому что в главном дереве лежит незакоммиченная работа владельца; в worktree её нет, поэтому fmt-краснота атрибутируется пяти файлам пакета. Коммит `eb8f9a1` (parent `882e3b5`, 5 файлов, +41/−14): 4 файла — только rustfmt (порядок импортов в `run.rs`, разворот `Refused`-ветки в `gateway/src/gate.rs`, склейка цепочки в `gateway/tests/gate.rs`, разбиение let-else и `format!` в `gate_channel.rs`); `authorization.rs` — импорт `std::process::Command`, новый тест `workspace_is_rustfmt_clean()` и один rustfmt-перенос уже существующего let-паттерна (это и был измеренный hunk `:321`). Проверено оркестратором на коммите: `cargo fmt --all -- --check` exit 0 (в worktree), `cargo test -p stellwerk-task --test authorization` → `8 passed`, `cargo clippy -p stellwerk-task --all-targets -- -D warnings` exit 0; главное дерево не тронуто (HEAD `882e3b5`, 8 файлов владельца ` M`). **Todo 3a остаётся `[~]`: merge и аттестация невозможны, пока база не собирается** — на `882e3b5` `crates/stellwerk-cli/src/main.rs:11` ссылается на `stellwerk_task::cli::TuiArgs`, а `struct TuiArgs` нет ни в одном коммите (есть только в незакоммиченном `crates/stellwerk-task/src/cli.rs:71` владельца); второй дефект базы — `clippy -D warnings` на `registry/src/cmd_add.rs:102` (`unnecessary_map_or`), тоже чинится только WIP владельца. FINDING записан в `.omo/notepads/stellwerk-production-v1/issues.md`, доказательства — `.omo/evidence/task-3a-p04-worktree.txt` и `.omo/evidence/task-3a-p04-blocked-base.txt`.

- [x] 4. P02: один профиль исполнения `pi`, ADR отменяет ADR-0015
  What to do / Must NOT do: создать `docs/adr/0021-single-execution-profile.md`, отменяющий ADR-0015 и фиксирующий, что единственный профиль — `pi`, а `task.toml` несёт только `profile`; привести `execution.rs` к одному разрешённому профилю, убрав `fixture-engine` из `ALLOWED_PROFILES`; тест `execution` требует отказа на любой профиль кроме `pi`; существующее ожидание `crates/stellwerk-task/tests/execution.rs:174` (`resolve(Some(&spec("/bin/true", "fixture-engine")), &REGISTRY).is_ok()`) инвертировать на `.is_err()` — иначе старый тест закрепляет ровно отменённое поведение. НЕ оставлять `ExecutionSpec { program, engine }` живым API без ADR-пометки; НЕ добавлять зависимостей.
  Parallelization: Wave 2 | Blocked by: 00 | Blocks: 03, r02
  References: `crates/stellwerk-task/src/execution.rs:25-32,95-101,188-208`, `docs/adr/0015-execution-spec-not-task-spec.md`, `crates/stellwerk-task/tests/execution.rs`.
  Acceptance criteria: `cargo test -p stellwerk-task --test execution` PASS.
  QA scenarios: happy — профиль `pi` разрешён; failure — `fixture-engine` отвергнут с названной причиной. Evidence `.omo/evidence/task-4-stellwerk-production-v1.txt`
  Commit: Y | `feat(execution): single pi profile, supersede ADR-0015`

- [x] 5. P03: contract-change — 4 варианта `TaskStatus` и bindings evidence
  What to do / Must NOT do: в `crates/stellwerk-core/src/contract.rs` добавить `AwaitingHumanValidation`, `HumanValidated`, `Refuted`, `Cancelled` в `TaskStatus:185` и типизированные поля bindings в `EvidenceEntry:449` (`recipe_digest`, `policy_digest`, `model_fingerprint`, `commit_sha`); обновить все места сопоставления; ADR `docs/adr/0022-task-status-human-and-refutation.md`; тест в `task_lifecycle.rs` требует переходов `Suspended -> AwaitingHumanValidation -> HumanValidated` и `EvidenceSubmitted -> Refuted`. Пакет несёт метку `contract-change`. НЕ менять lint-политику; НЕ добавлять `#[allow]`; НЕ трогать другие файлы.
  Parallelization: Wave 3 | Blocked by: 02 | Blocks: a01, c01, o01
  References: `crates/stellwerk-core/src/contract.rs:185,437,449`, `crates/stellwerk-core/tests/task_lifecycle.rs`, `AGENTS.md` (contract-change + ADR).
  Acceptance criteria: `cargo test -p stellwerk-core --test task_lifecycle` PASS и `cargo test -p stellwerk-core` PASS.
  QA scenarios: happy — новые переходы разрешены; failure — переход `Running -> HumanValidated` отвергнут. Evidence `.omo/evidence/task-5-stellwerk-production-v1.txt`
  Commit: Y | `feat(contract): human validation and refutation states`
  BLOCKED (SPLIT_REQUIRED, measured 2026-11-01): пакет в 3 файла не оставляет workspace собираемым. `TaskStatus` — 12 вариантов без `#[non_exhaustive]`; `crates/stellwerk-store/src/sqlite.rs:29-44` держит исчерпывающий `match` без catch-all → жёсткая ошибка компиляции, и **ни один пакет V1 этим файлом не владеет**. `EvidenceEntry` — 6 обязательных полей без `Default`; литералы в `crates/stellwerk-task/src/evidence.rs:62,88,118` и `crates/stellwerk-task/tests/vertical.rs:410` ломаются. Плюс `sqlite.rs:47-70` (`str_to_status`) компилируется, но новый статус, записанный в БД, станет нечитаемым — дефект закона 6. Всего нужно 6 файлов, src ≈ +20, tests ≈ +4. Нужно решение владельца: расширить пакет `limit_waiver`-ом или дробить 03a/03b (второй вариант даёт красный workspace между коммитами). Приёмка пакета компилирует только `stellwerk-core` и этого не поймает.

- [x] 6. L01: закрыть 77 нарушений plan-lint на каноне
  What to do / Must NOT do: ровно 33 правки в существующих пакетах по таблице ниже (`depends_on` — дописать id к существующему списку; `overlap_waiver` — новая строка frontmatter) и новый тест `crates/stellwerk-cli/tests/plan_lint.rs`, который запускает `python3 scripts/plan-lint.py`, требует exit 0 и отсутствия подстроки `НАРУШЕНИЙ`. НЕ менять `files_allowed`, `goal`, `acceptance` и тело пакетов; НЕ трогать `.omo/plans/` (C8 печатается, но в код возврата не входит); НЕ использовать `--gating`; НЕ менять ни одного `.rs` вне нового теста.
  Files allowed (34 пути, полностью): `.sisyphus/plans/N1/packet-04.md`, `.sisyphus/plans/N1/packet-05.md`, `.sisyphus/plans/N1/packet-06.md`, `.sisyphus/plans/N1/packet-08.md`, `.sisyphus/plans/N1/packet-09.md`, `.sisyphus/plans/N1/packet-10.md`, `.sisyphus/plans/N2/packet-00b.md`, `.sisyphus/plans/N2/packet-00c.md`, `.sisyphus/plans/N2/packet-00d.md`, `.sisyphus/plans/N2/packet-00e.md`, `.sisyphus/plans/N2/packet-00f.md`, `.sisyphus/plans/N2/packet-00g1.md`, `.sisyphus/plans/N2/packet-00g3.md`, `.sisyphus/plans/N2/packet-00h2a.md`, `.sisyphus/plans/N2/packet-00h2b.md`, `.sisyphus/plans/N2/packet-00h3a.md`, `.sisyphus/plans/N2/packet-00h3b.md`, `.sisyphus/plans/N2/packet-00h3c.md`, `.sisyphus/plans/N2/packet-00h3d.md`, `.sisyphus/plans/N2/packet-00h3e.md`, `.sisyphus/plans/N2/packet-00i1.md`, `.sisyphus/plans/N2/packet-00j1.md`, `.sisyphus/plans/N2/packet-00j2.md`, `.sisyphus/plans/N2/packet-00j3.md`, `.sisyphus/plans/N2/packet-00j3a.md`, `.sisyphus/plans/N2/packet-00j4.md`, `.sisyphus/plans/N2/packet-00j5.md`, `.sisyphus/plans/N2/packet-00j6.md`, `.sisyphus/plans/N2/packet-00j7.md`, `.sisyphus/plans/N2/packet-00j7a.md`, `.sisyphus/plans/TUI/packet-01.md`, `.sisyphus/plans/TUI/packet-02.md`, `.sisyphus/plans/TUI/packet-03.md`, `crates/stellwerk-cli/tests/plan_lint.rs`.
  Exact edits (33 строки, ничего кроме этого):
  | пакет | правка |
  | --- | --- |
  | N1/packet-04 | depends_on + `02` |
  | N1/packet-05 | depends_on + `03` |
  | N1/packet-06 | depends_on + `05` |
  | N1/packet-08 | depends_on + `04` |
  | N1/packet-09 | depends_on + `06` |
  | N1/packet-10 | depends_on + `01` |
  | N2/packet-00b | overlap_waiver: C9 |
  | N2/packet-00c | overlap_waiver: C9 |
  | N2/packet-00d | overlap_waiver: C9 |
  | N2/packet-00e | depends_on + `00d`; overlap_waiver: C9 |
  | N2/packet-00f | depends_on + `00c`; overlap_waiver: C9 |
  | N2/packet-00g1 | overlap_waiver: C9 |
  | N2/packet-00g3 | depends_on + `00e`; overlap_waiver: C9 |
  | N2/packet-00h2a | depends_on + `00h1` |
  | N2/packet-00h2b | overlap_waiver: C9 |
  | N2/packet-00h3a | depends_on + `00h2b` |
  | N2/packet-00h3b | depends_on + `00h2a` |
  | N2/packet-00h3c | depends_on + `00h3b` |
  | N2/packet-00h3d | depends_on + `00h3a`, `00h3c` |
  | N2/packet-00h3e | depends_on + `00h3d` |
  | N2/packet-00i1 | depends_on + `00e`, `00f` |
  | N2/packet-00j1 | depends_on + `00h3c`, `00h3d` |
  | N2/packet-00j2 | depends_on + `00d`, `00i1` |
  | N2/packet-00j3 | depends_on + `00j2` |
  | N2/packet-00j3a | depends_on + `00h2a` |
  | N2/packet-00j4 | depends_on + `00g3b` |
  | N2/packet-00j5 | depends_on + `00j3`, `00j4` |
  | N2/packet-00j6 | depends_on + `00f`, `00i1`, `00j1` |
  | N2/packet-00j7 | depends_on + `00h1`, `00j3`, `00j5` |
  | N2/packet-00j7a | depends_on + `00j5` |
  | TUI/packet-01 | overlap_waiver: C9 |
  | TUI/packet-02 | overlap_waiver: C9 |
  | TUI/packet-03 | depends_on + `02`; overlap_waiver: C9 |
  Parallelization: Wave 2 | Blocked by: 00 | Blocks: —
  References: `/tmp/opencode/p00/planlint-full.txt` (исходные 77), `scripts/plan-lint.py:381-390` (`related`, транзитивно в обе стороны), `scripts/plan-lint.py:465` (`WAIVER_RE`), `scripts/plan-lint.py:523-598` (C9 и waiver), `.sisyphus/plans/N1`, `N2`, `TUI`, `AGENTS.md` (прецедент M5/packet-21,22,23; два условия waiver).
  Acceptance criteria: `python3 scripts/plan-lint.py` exit 0 и вывод содержит `ПЕРЕСЕЧЕНИЯ МЕЖДУ MILESTONE: порядок объявлен везде`; `cargo test -p stellwerk-cli --test plan_lint` PASS.
  QA scenarios: happy — линт зелёный; failure — убрать `overlap_waiver` у `N2/packet-00b`, ожидать C9, вернуть. Evidence `.omo/evidence/task-6-stellwerk-production-v1.txt`
  Commit: Y | `docs(plans): close plan-lint debt with declared order`

- [x] 7. R01: один путь исполнения вместо plan-Build и run
  What to do / Must NOT do: свести `crates/stellwerk-cli/src/plan.rs` Build-путь и `lib.rs` run-путь к одной функции исполнения в `lib.rs`; `main.rs` диспетчеризует оба в неё; `run_build` перестаёт вызывать `worker.exec` напрямую и идёт через `run_once`/`authorize`. Тест в `tests/run.rs` требует, что оба входа вызывают одну и ту же авторизацию. НЕ менять поведение гейта; НЕ заводить второй модуль-дубликат.
  Parallelization: Wave 3 | Blocked by: 01 | Blocks: r02, r03
  References: `crates/stellwerk-cli/src/lib.rs:53-188`, `crates/stellwerk-cli/src/main.rs:54-82`, `crates/stellwerk-cli/src/build_runner.rs:355-477` (exec на 455-457), `crates/stellwerk-task/src/runner.rs:296,394`.
  Acceptance criteria: `cargo test -p stellwerk-cli --test run` PASS.
  QA scenarios: happy — оба пути логируют один `authorize`; failure — прямой `worker.exec` без авторизации отвергается тестом. Evidence `.omo/evidence/task-7-stellwerk-production-v1.txt`
  Commit: Y | `refactor(cli): single authorized execution path`

- [x] 8. R02: `task.toml` несёт только `profile = "pi"` — merged 435c134 + 4488e99 (7 файлов, limit_waiver); RED 3 compile errors → GREEN 5/5; cli_watch 3/3, cli run 4/4; accept.sh ПРИНЯТО; attestation 318 Verified; gate passed:true (tail 63873847…). FINDING: `pi_transport::pi_executes_an_edit_the_model_asked_for` красный уже на базе 76fa801 — предсуществующий, вне пакета
  What to do / Must NOT do: добавить поле `profile` в `TaskFileSpec` (`deny_unknown_fields` уже стоит) и отвергать любое значение кроме `pi`; `resolve` вызывается из продакшн-пути, а не только из тестов. Тест в `tests/taskfile.rs`: `profile = "pi"` принимается, `profile = "opencode"` и отсутствие поля дают разные названные отказы. НЕ добавлять дефолт «если нет — значит pi».
  Parallelization: Wave 4 | Blocked by: 02, 01 | Blocks: r03
  References: `crates/stellwerk-task/src/taskfile.rs:32-44,118-137`, `crates/stellwerk-task/src/execution.rs:188`, `crates/stellwerk-core/src/contract.rs:294-311` (`deny_unknown_fields`).
  Acceptance criteria: `cargo test -p stellwerk-task --test taskfile` PASS.
  QA scenarios: happy — `pi` принят; failure — отсутствие `profile` даёт `MissingProfile`, не `pi`. Evidence `.omo/evidence/task-8-stellwerk-production-v1.txt`
  Commit: Y | `feat(taskfile): require explicit pi profile`
  BLOCKED_BY_OWNER_WIP (measured 2026-11-01): профиль реализован и приёмка пакета проходит (`cargo test -p stellwerk-task --test taskfile` 5 passed), но `cargo test -p stellwerk-task` краснеет на `cli::tests::cli_apply_creates_task_and_reports_conflict` и `cli_get_prints_spec_and_reports_absence`: генератор `task.toml` живёт в `crates/stellwerk-task/src/cli.rs`, а этот файл держит незакоммиченные правки владельца (+12 строк, `TuiArgs`) и трогать его запрещено. Плюс `crates/stellwerk-cli/examples/{n1-smoke,pi-smoke}.toml` без `profile` валят `crates/stellwerk-cli --test run` (2 из 4). Нужно 2 файла вне пакета (`cli.rs` — после коммита владельца; 2 примера), затем R02 переиспользует готовый дифф из `git stash r02-parked-profile` / `/tmp/opencode/r02-taskfile-profile.patch`. Дерево возвращено в зелёное, чтобы не тащить красный workspace дальше.

- [x] 9. A01: типизированный `RequestedCall` с digest содержимого — SPLIT → `V1/packet-a01b`: merged 1f98ecf; RED `risk_table_allow_of_write_requires_content_digest` → GREEN 17/17; accept.sh ПРИНЯТО; attestation 316 Verified; gate_evaluate passed:true (tail 1d266460…)
  What to do / Must NOT do: ввести в `crates/stellwerk-gateway/src/gate.rs` тип вызова, несущий полный аргумент и `content_digest` (SHA-256 содержимого для пишущих инструментов), и `AuthorizedCall`, побайтово связанный с digest; `GateRequest` в `gate_channel.rs` расширяется полями аргумента и digest. Тест в `tests/gate.rs`: два вызова с одинаковым `(tool, path)`, но разным содержимым дают разные решения и разные `scope`. НЕ принимать `path` без digest для пишущего инструмента; НЕ выводить digest из текста модели.
  Parallelization: Wave 4 | Blocked by: 03 | Blocks: a03, a02, r03
  References: `crates/stellwerk-gateway/src/gate.rs:16-31`, `crates/stellwerk-task/src/m5/gate_channel.rs:26-38,103-140`, `crates/stellwerk-gateway/tests/gate.rs`.
  Acceptance criteria: `cargo test -p stellwerk-gateway --test gate` PASS.
  QA scenarios: happy — digest совпал, `Authorized` с названным `scope`; failure — подмена содержимого после авторизации даёт отказ. Evidence `.omo/evidence/task-9-stellwerk-production-v1.txt`
  Commit: Y | `feat(gateway): typed requested call with content digest`
  PARTIAL (measured 2026-11-01, landed as `099881a` под фактическим сообщением `feat(gate): bind authorized writes to content digest`): приёмка пакета проходит (`cargo test -p stellwerk-gateway --test gate` 15 passed; `-p stellwerk-gateway`, `-p stellwerk-task`, `-p stellwerk-cli` зелёные; `cargo build --workspace` OK). Ветка `AllowBasis::ApprovedPlan` больше не выдаёт разрешение по паре `(tool, path)` — только `AuthorizedCall`, побайтово связанный с digest, а `verify_content` ловит подмену после авторизации. Но п. «Запрещено» №1 выполнен не полностью: `gate_channel.rs:264` (`AllowBasis::RiskTable => GateAnswer::Allow`) по-прежнему разрешает пишущий инструмент, пришедший с путём и без digest, а production-политика `crates/stellwerk-cli/src/build_runner.rs:149` возвращает именно `RiskTable`. Закрытие требует правки `build_runner.rs` и/или `crates/stellwerk-cli/tests/build_runner.rs:386-437` — вне `files_allowed` пакета a01 — и уронит живой build-прогон `an_edit_reaches_the_copy_and_the_gate_authorizes_it`, пока R03 не пришлёт аргумент встроенного `edit`. Поэтому `[~]`, а не `[x]`: закрыть вместе с R03 (todo 11) или отдельным пакетом с меткой `contract-change`. FINDING записан в `.omo/notepads/stellwerk-production-v1/issues.md`.

- [x] 10. A03: авторитет классификации риска принадлежит хосту
  What to do / Must NOT do: `risk_for` в `orchestrator.rs` перестаёт принимать риск от вызывающего; риск берётся только из `StaticRegistry`/`ToolEntry`; неизвестный инструмент даёт `Unclassified`, а не ноль. Тест в `tests/authorization.rs`: агент, назвавший `bash` как `R0`, получает `R5`-решение; неизвестный инструмент даёт `Unclassified`, отличимый от `Deny`. НЕ менять матрицу уровней; НЕ добавлять дефолт риска.
  Parallelization: Wave 5 | Blocked by: a01 | Blocks: r03
  References: `crates/stellwerk-task/src/orchestrator.rs:70-121`, `docs/adr/0017-risk-classification-authority.md`, `crates/stellwerk-task/tests/authorization.rs`.
  Acceptance criteria: `cargo test -p stellwerk-task --test authorization` PASS.
  QA scenarios: happy — `bash` отклонён как R5; failure — неизвестный инструмент даёт `Unclassified`, не `Allow`. Evidence `.omo/evidence/task-10-stellwerk-production-v1.txt`
  Commit: Y | `fix(orchestrator): host owns risk classification`
  DONE (measured 2026-11-01, landed as `fea4fa7` под фактическим сообщением `test(orchestrator): lock host authority over risk classification`): приёмка `cargo test -p stellwerk-task --test authorization` — 7 passed; `cargo test -p stellwerk-task` — 39 targets ok, 0 failed; `cargo build --workspace` exit 0. Премиса пакета устарела: на `099881a` `risk_for`/`preflight` уже читают риск ТОЛЬКО из реестра (`orchestrator.rs:103-121,435-447`), пути «риск из аргумента вызывающего» нет, поэтому `orchestrator.rs` не менялся, а пакет лёг стражем: `host_classifies_risk_not_caller` подаёт `StaticRegistry` с `bash`=`R5`, вызывающий заявляет `R0`, решение обязано взять `R5` и отказать с `risk_denied`; неизвестный инструмент даёт `Unclassified` с `unclassified_tool`, и обе причины `assert_ne`. Страж доказан мутацией `authorize` на доверие `call.risk` (красный: `Classified(R0)` vs `Classified(R5)`), затем откат — `git diff -- orchestrator.rs` пуст. `bash` в `TOOL_REGISTRY` не добавлен, `RISK_PERMISSION` не тронута.

- [x] 11. R03: Pi получает extension-инструменты вместо встроенных — merged 5bd77cf (3 файла; C2 src 133 / tests 114); RED `tool_call_goes_to_socket_with_args` (path-only frame) → GREEN 13/13, включая ранее красный `pi_executes_an_edit_the_model_asked_for`; Pi 1.0.2 `--no-builtin-tools` подтверждён; fmt exit 0, clippy -D warnings exit 0; attestation 322 Verified; gate_evaluate passed:true (tail 2975d577…)
  What to do / Must NOT do: в `crates/stellwerk-task/pi/stellwerk_transport.ts` зарегистрировать `read`/`edit`/`write` через `pi.registerTool` с `execute`, который шлёт полный типизированный вызов в `gate.sock` и возвращает результат хоста; встроенные инструменты не предлагаются (`BUILD_OFFERED`/`BUILD_HELD` в `build_runner.rs` не трогаются — их правит a02); `pi.on("tool_call")` остаётся вторым блокирующим поясом. `orchestrator.rs`: `pi_transport_args` больше не передаёт встроенные имена. Тест в `tests/pi_transport.rs`: вызов инструмента уходит в сокет с аргументами, а не исполняется внутри Pi. Стоп-условие: `grep -c "registerTool" <pi-install>/dist/core/extensions/types.d.ts` равно 0 либо в типе `ToolDefinition` нет поля `execute` — вернуть `BLOCKED_BY_PI_API`. Сначала `pi --version`: при версии ниже `1.0.2` вернуть `BLOCKED_BY_PI_VERSION`. НЕ оставлять путь, где Pi пишет файл сам.
  Parallelization: Wave 6 | Blocked by: r01, r02, a01, a03 | Blocks: a02, r04
  References: `crates/stellwerk-task/pi/stellwerk_transport.ts:259-277`, `crates/stellwerk-task/src/orchestrator.rs:243-257`, `~/.pi/agent/install/releases/1.0.2/node_modules/@earendil-works/pi-coding-agent/dist/core/extensions/types.d.ts:1192,492`, `crates/stellwerk-task/tests/pi_transport.rs`.
  Acceptance criteria: `cargo test -p stellwerk-task --test pi_transport` PASS.
  QA scenarios: happy — вызов доходит до сокета с аргументами; failure — прямой файл-эффект внутри Pi не наблюдается. Evidence `.omo/evidence/task-11-stellwerk-production-v1.txt`
  Commit: Y | `feat(pi): host-executed tools via registerTool`
  BLOCKED_BY_PI_VERSION (measured 2026-11-01, HEAD `fea4fa7`): `pi --version` → `0.87.0`, а стоп-условие пакета требует не ниже `1.0.2`. Установлен только `0.87.0`: `~/.pi/agent/install/releases/` содержит единственный каталог `0.87.0`, локального архива `1.0.2` нет. Бинарь — `/home/izyabretatel/.local/bin/pi` (user-local, без sudo). API-стоп-условие при этом НЕ срабатывает: `grep -c "registerTool" …/0.87.0/…/dist/core/extensions/types.d.ts` → 2, то есть имя в 0.87.0 уже есть; блокирует именно версия. На реальном пакете `1.0.2` (скачан в `/tmp/opencode/pi-probe`, sha256 `eda5ae78…ceeda2`, НЕ установлен, система не менялась) `registerTool` — строка 1192, `ToolDefinition.execute(toolCallId, params, signal, onUpdate, ctx)` — строка 492, то есть `BLOCKED_BY_PI_API` не заявляется; полная проба — `.omo/evidence/task-11-pi-api-probe.txt`. Сеть доступна (`registry.npmjs.org` → 200), в реестре есть `1.0.2` (и `1.0.0`, `1.0.1`, `1.0.3`, `1.0.4`), `latest` = `1.4.0`. Второй объявленный блокер — `r02` (todo 8, `[~] BLOCKED_BY_OWNER_WIP`).
  РАЗБЛОКИРОВАНО ПО ВЕРСИИ (2026-11-02, измерено): Pi `1.0.2` установлен в managed-layout из кэшированного архива (sha256 `eda5ae7875343bd902ffe55718fb65b2406d7b03abecc5cb89d8e4bb09ceeda2`, совпадает с пином владельца), БЕЗ `pi update self`. Гейт исполнен: `which pi` → `/home/izyabretatel/.local/bin/pi` → launcher → `~/.pi/agent/install/releases/1.0.2`, `pi --version` → `1.0.2`. `releases/0.87.0` не тронут и исполняется; откат — одна строка в `current-version`. Provenance: `metadata.json` собран из `package-lock.json` (116 пакетов, resolved+integrity) плюс source-путь и sha256; `sourceCommit`/`publishedAt` не выдумывались. Evidence `.omo/evidence/task-11-pi-install-102.txt`. `BLOCKED_BY_PI_VERSION` снят; todo 11 по-прежнему `[~]` из-за `r02` — версия была не единственным блокером.
  ЦЕПОЧКА: todo 12–30 зависят от `r03` транзитивно (12/13 ← r03; 14 ← a02; 15 ← c01; 16 ← a02,r04; 17 ← c02; 18 ← c02; 19–23 ← o01/v02/o02…; 24–30 ← v03/i01/i02/s01…). Пока Pi `1.0.2` нет, ни один оставшийся todo не разблокирован; поэтому 12–30 помечены `[~]` (правка 2026-11-01: недостижимы до снятия блокера `r03`); они не «отказали», они ждут одного внешнего действия.
  Нужно решение владельца: (а) поставить ровно `1.0.2` по пину плана (npm в user-local prefix), (б) поднять пин до `1.4.0` отдельным пакетом с меткой `contract-change` и ADR, либо (в) отложить R03 и всю цепочку.

- [x] 12. A02: хост исполняет только `AuthorizedCall` — merged 011ce23 + c1faf7b (3 файла; C2 src 194 / tests 181); RED 3 compile errors → GREEN host_tools 3/3, gate_channel 17/17, pi_transport 13/13; fmt/clippy exit 0 на чистом target; accept.sh V1 packet-a02 ПРИНЯТО; attestation 324 Verified; gate_evaluate passed:true (tail 5d1c9613…); evidence a7f78a3. FINDING: host executor подключён как библиотечная функция (`decide_request` + `apply_gate_decision`), но не вызван из `run_build`; сквозное исполнение и возврат результата в сокет переносятся в V01, т.к. требуют `transport.ts` вне `files_allowed` a02 и иначе дают двойную запись
  What to do / Must NOT do: в `crates/stellwerk-cli/src/build_runner.rs` применить авторизованный вызов к workspace (read/edit/write), сверяя digest до и после записи, и вернуть результат в сокет; `gate_channel.rs` отдаёт `AuthorizedCall`, который нельзя сконструировать вне гейта. Тест `tests/host_tools.rs`: запись с несовпавшим digest отвергается и файл не меняется; успешная запись оставляет digest, равный авторизованному. НЕ позволять правку вне workspace; НЕ писать при отказе.
  Parallelization: Wave 7 | Blocked by: a01, r03 | Blocks: c01, v01
  References: `crates/stellwerk-cli/src/build_runner.rs:82-113,355-477`, `crates/stellwerk-task/src/m5/gate_channel.rs:103-140`, `crates/stellwerk-task/src/m5/sandbox_bwrap.rs:184-206` (границы workspace).
  Acceptance criteria: `cargo test -p stellwerk-cli --test host_tools` PASS.
  QA scenarios: happy — файл записан, digest совпал; failure — путь вне workspace отвергнут, файл не создан. Evidence `.omo/evidence/task-12-stellwerk-production-v1.txt`
  Commit: Y | `feat(build): host applies authorized calls only`

- [x] 13. R04: убрать fixture-путь из `run` и TUI — merged edc827e (3 файла; C2 src 72 / tests 124); RED `missing_engine_is_named_refusal` на базе a7f78a3 (fixture-not-found вместо `MissingEngine`) → GREEN run 5/5, tui_start 7/7, host_tools 3/3; fmt exit 0, clippy -D warnings exit 0; accept.sh V1 packet-r04 ПРИНЯТО; attestation 326 Verified; gate_evaluate passed:true (tail feb795ab…); evidence cd84881
  What to do / Must NOT do: удалить поиск и копирование `pi-fixture.sh` из `lib.rs:59-102` и `tui.rs:143-148`; `stellwerk run` исполняет Pi по профилю `pi`; при отсутствии Pi — названный отказ `MissingEngine`, а не тихий переход на фикстуру. Тест в `tests/run.rs`: отсутствие Pi даёт `MissingEngine`; наличие — запускает Pi, а не `sh`. НЕ оставлять `pi-fixture.sh` в продакшн-путях; НЕ менять пример-фикстуру, используемую тестами.
  Parallelization: Wave 7 | Blocked by: r03 | Blocks: v01
  References: `crates/stellwerk-cli/src/lib.rs:59-102,150`, `crates/stellwerk-cli/src/tui.rs:143-148`, `crates/stellwerk-cli/tests/run.rs`, `crates/stellwerk-cli/tests/tui_start.rs`.
  Acceptance criteria: `cargo test -p stellwerk-cli --test run` PASS.
  QA scenarios: happy — реальный движок; failure — без движка `MissingEngine`, не успех. Evidence `.omo/evidence/task-13-stellwerk-production-v1.txt`
  Commit: Y | `fix(cli): remove fixture fallback from production paths`

- [x] 14. C01: кандидат коммита из workspace по digest — merged 73df461 (2 файла; C2 src 57 / tests 100); RED E0609 `no field candidate` → GREEN build_runner 17/17; fmt exit 0, clippy -D warnings exit 0, `cargo build --workspace` exit 0; accept.sh V1 packet-c01 ПРИНЯТО; attestation 328 Verified; gate_evaluate passed:true (tail e2f2dbd8…); evidence ed15c94. FINDING: `Vec<CandidateFile>` не представляет удаления и смену симлинка — молча опускаются; решить в C02/V01 (не расширять C01)
  What to do / Must NOT do: `build_runner.rs` после успешной сессии собирает кандидат из workspace (файлы + digest), не из истории репозитория; `sandbox_bwrap.rs` не пропускает `.git` внутрь. Тест в `tests/build_runner.rs`: кандидат содержит ровно изменённые файлы и их digest; файл вне workspace в кандидат не попадает. НЕ создавать коммит в пользовательском репозитории; НЕ использовать `git worktree`.
  Parallelization: Wave 8 | Blocked by: 03, a02 | Blocks: c02
  References: `crates/stellwerk-cli/src/build_runner.rs:82-113`, `crates/stellwerk-task/src/m5/sandbox_bwrap.rs:184-206`, `crates/stellwerk-cli/tests/build_runner.rs`.
  Acceptance criteria: `cargo test -p stellwerk-cli --test build_runner` PASS.
  QA scenarios: happy — кандидат совпадает с правками; failure — посторонний файл не попадает. Evidence `.omo/evidence/task-14-stellwerk-production-v1.txt`
  Commit: Y | `feat(build): candidate from workspace with digests`

- [x] 15. C02: отказ коммита при грязном дереве — merged 2462251 (2 файла; C2 src 87 / tests 209); RED E0432 `unresolved imports accept_candidate/snapshot_workspace/CandidateVerdict/DirtyWorkspace/HARNESS_EXTENSION` на чистом клоне ed15c94 → GREEN candidate 7/7, build_runner 17/17 с Pi 1.0.2 (без PI_BIN); fmt exit 0, clippy -D warnings exit 0, `cargo build --workspace` exit 0; accept.sh V1 packet-c02 ПРИНЯТО; attestation 330 Verified; gate_evaluate passed:true (tail e2b25013…); evidence 3a596b9. Уточнение: заявление воркера, что Pi 1.0.2 не влезает в бюджет и нужен 0.87.0, не воспроизвелось — оркестратор прогнал 17/17 на 1.0.2; 0.87.0 в принятый результат не входит
  What to do / Must NOT do: перед принятием кандидата проверять, что workspace чист относительно базы и что digest всех файлов совпадает; иначе — названный отказ `DirtyWorkspace` с перечнем файлов. Тест `tests/candidate.rs`: лишняя правка в workspace даёт `DirtyWorkspace`; чистое дерево проходит. НЕ «чинить» дерево автоматически; НЕ игнорировать неотслеживаемые файлы.
  Parallelization: Wave 9 | Blocked by: c01 | Blocks: v02, o01
  References: `crates/stellwerk-cli/src/build_runner.rs`, `crates/stellwerk-cli/tests/candidate.rs`.
  Acceptance criteria: `cargo test -p stellwerk-cli --test candidate` PASS.
  QA scenarios: happy — чистое дерево; failure — `DirtyWorkspace` с именами файлов. Evidence `.omo/evidence/task-15-stellwerk-production-v1.txt`
  Commit: Y | `feat(build): refuse dirty workspace`

- [x] 16. V01: вертикальное доказательство host-executed edit — UNBLOCKED 2026-11-02 (owner B): `stellwerk_transport.ts:128-140` пишет файл сам через `writeFileSync` после `authorized`; хост не получает байты, поэтому `tests/vertical_v1.rs` без правок `transport.ts`/`build_runner.rs` не может доказать host execution. Нужно решение владельца: расширить `files_allowed` V01 (+`stellwerk_transport.ts`, `build_runner.rs`, `limit_waiver`) или вставить отдельный packet `a02b` и оставить V01 тест-only. Не расширять скоуп самостоятельно; `V1/packet-a02b.md` создан, V01 теперь зависит от `[a02b, r04]`, test-only. СОСТОЯНИЕ 2026-11-02: решение владельца (B) принято, `V1/packet-a02b.md` создан; текст выше историчен. DONE: merged `42e1e36`; `vertical_v1` 1/1 PASS на реальном Pi 1.0.2; естественного RED не было, потому что a02b уже связал путь — невакуумность доказана мутацией в одноразовом worktree; PolyForge `V1/v01` gate passed, tail `e3295962…`, attestation 346 Verified.
  What to do / Must NOT do: сквозной тест `tests/vertical_v1.rs`: Pi просит `edit` → хост авторизует с digest → файл изменён → digest после записи равен авторизованному → кандидат содержит файл. Стоп-условие: `pi --version` печатает не `1.0.2` — вернуть `BLOCKED_BY_PI_VERSION`, не подменять фикстурой. НЕ утверждать успех без реального прогона.
  Parallelization: Wave 8 | Blocked by: a02b, r04 | Blocks: —
  References: `crates/stellwerk-task/pi/stellwerk_transport.ts`, `crates/stellwerk-cli/src/build_runner.rs`, `docs/PI_1_0_2_PROBE.md`.
  Acceptance criteria: `cargo test -p stellwerk-cli --test vertical_v1` PASS.
  QA scenarios: happy — полный путь; failure — подмена digest ломает тест. Evidence `.omo/evidence/task-16-stellwerk-production-v1.txt`
  Commit: Y | `test(cli): vertical host-executed edit`

- [x] 17. V02: вертикальное доказательство отказа — UNBLOCKED 2026-11-02 (owner: contract-change): `crates/stellwerk-task/src/m5/model_transport.rs:187-202` отвечает `ErrorCode::ToolNotOffered` до `pi.on("tool_call")`, поэтому скриптованный `bash` через `run_build` даёт `gate=[] requests=0 turns=1 candidate=[]` и `tool_not_offered: tool \`bash\` was not offered`. Runner-level `run_once` уже пишет `Denied`-чек и зеленеет сразу, то есть честного RED там нет. Чтобы получить build-level Deny, нужна правка `model_transport.rs`/`build_runner.rs:80`/`stellwerk_transport.ts` — вне `files_allowed: [tests/vertical_v2.rs]`. Тест `vertical_v2.rs` (310 строк, > tests-бюджета 250; V02b `limit_waiver`) оставлен untracked RED, не закоммичен. Решение: contract-change, ADR-0023; V02 `files_allowed` 3 (`model_transport.rs`, `model_transport_wire.rs`, ADR-0023); V02b `files_allowed` 2 (`stellwerk_transport.ts`, `vertical_v2.rs`); 310 test lines разрешены ADR и waiver. СОСТОЯНИЕ 2026-11-02: V02 merged `1c0fe3e` (wire 20/20), V02b merged `7821ce0` (vertical_v2 2/2, host_tools 4/4); held `bash` registered via `prepareLoadout`, но не предлагается модели.
  What to do / Must NOT do: `tests/vertical_v2.rs`: запрещённый вызов даёт `Deny`/`Suspended`, кандидат не создаётся, evidence фиксирует отказ, а не пустоту. НЕ позволять «нет ответа — значит разрешено».
  Parallelization: Wave 10 | Blocked by: c02 | Blocks: v03
  References: `crates/stellwerk-gateway/src/gate.rs:37-50`, `crates/stellwerk-cli/src/build_runner.rs`.
  Acceptance criteria: `cargo test -p stellwerk-cli --test vertical_v2` PASS.
  QA scenarios: happy — отказ зафиксирован; failure — отсутствие ответа трактуется как отказ, не как успех. Evidence `.omo/evidence/task-17-stellwerk-production-v1.txt`
  Commit: Y | `test(cli): vertical refused run leaves no candidate`

- [x] 18. O01: порядок evidence `ModelClaim → ToolAttestation → Validation` — closed 2026-11-02; commits `2ff1057` + final style `b782359` (formatting only, supersedes wrong-edition `b43cb06`); RED на базе 3a596b9 с новым тестом 13/3, GREEN 16/16, targeted vertical 1/1, `rustfmt --edition 2021 --check` 0, clippy 0, accept **ПРИНЯТО** на чистом клоне; attestation **334 Verified**, tail `69f94b00…`, evidence `ffd06ec`.
  Verified: порядок проверяется только для присутствующих звеньев; пустой `result_hash` у любого звена и пустая команда у ToolAttestation/Validation дают отказ; Validation.result_hash не сравнивается с ToolAttestation, ModelClaim не обязателен — vertical.rs:405-450 сохранён.
  Correction: `b43cb06` sorted imports по edition 2024; workspace — 2021. `b782359` вернул 2021-порядок, логика не менялась. Старые 332/`b43cb06`/`2d103ed` оставлены в истории как superseded.
  What to do / Must NOT do: в `evidence.rs` зафиксировать порядок и запретить `Validated` без `ToolAttestation`; каждое звено хранит свой digest и команду. Тест `tests/evidence.rs`: пропуск звена даёт отказ; цепочка из трёх звеньев даёт `Validated`. НЕ выводить аттестацию из текста модели.
  Parallelization: Wave 10 | Blocked by: 03, c02 | Blocks: o02, v03
  References: `crates/stellwerk-task/src/evidence.rs:61-189`, `.pf/ledger.jsonl` (реальная схема payload).
  Acceptance criteria: `cargo test -p stellwerk-task --test evidence` PASS.
  QA scenarios: happy — три звена; failure — `ModelClaim` без `ToolAttestation` не даёт `Validated`. Evidence `.omo/evidence/task-18-stellwerk-production-v1.txt`
  Commit: Y | `feat(evidence): enforce claim-attestation-validation order`

- [x] 19. O02: `Suspended` как отдельное состояние с evidence — closed 2026-11-02; commit `769c7fc` (test-only regression lock), `tests/runner.rs` +158; runner 16/16 incl. `suspended_and_deny_are_distinct`, fmt 2021 0, clippy 0, accept **ПРИНЯТО** (1 файл); attestation **337 Verified**, tail `210abd42…`, evidence `61c1ace`. Status: **ALREADY_SATISFIED** — production `run_once`/`gate` already distinguished `Suspended` vs `Denied`; no production change, no fake RED.
  What to do / Must NOT do: `runner.rs` различает «запрещено политикой» и «никто не ответил»; `Suspended` пишет evidence с причиной. Тест `tests/runner.rs`: неинтерактивный `Ask` даёт `Suspended`, а `Deny` — `Deny`; статусы различимы. НЕ сливать `Suspended` в `Deny`.
  Parallelization: Wave 11 | Blocked by: o01 | Blocks: o03, v04
  References: `crates/stellwerk-task/src/runner.rs:52-116,296-339`, `crates/stellwerk-gateway/src/gate.rs:33-50`.
  Acceptance criteria: `cargo test -p stellwerk-task --test runner` PASS.
  QA scenarios: happy — `Suspended` отличим; failure — молчаливый `Deny` ломает тест. Evidence `.omo/evidence/task-19-stellwerk-production-v1.txt`
  Commit: Y | `feat(runner): suspended is a distinct evidenced state`

- [x] 20. O03: evidence человеческой валидации — closed 2026-11-02; commit `ef82c4d`; RED на `validation("", …)` → `HumanValidated` (human_validation.rs:67), GREEN 1/1; `evidence` 16/16, `ledger_sqlite` 5/5, `vertical_n1` 2/2; fmt 2021 0, clippy 0; accept **ПРИНЯТО** (2 файла); attestation **339 Verified**, tail `15f5ce85…`, evidence `54762ec`.
  What to do / Must NOT do: `evidence.rs` принимает `Validation` только с явным человеческим решением и `task_id`; `AwaitingHumanValidation → HumanValidated` требует записи. Тест `tests/human_validation.rs`: без записи переход запрещён; с записью — разрешён. НЕ считать молчание валидацией.
  Parallelization: Wave 12 | Blocked by: o02 | Blocks: o04, v04
  References: `crates/stellwerk-core/src/contract.rs:185`, `crates/stellwerk-task/src/evidence.rs:111-189`.
  Acceptance criteria: `cargo test -p stellwerk-task --test human_validation` PASS.
  QA scenarios: happy — валидация с записью; failure — тишина не валидирует. Evidence `.omo/evidence/task-20-stellwerk-production-v1.txt`
  Commit: Y | `feat(evidence): human validation requires explicit record`

- [x] 21. O04: evidence опровержения — closed 2026-11-02; commit `3f35b4c`; RED на пустом основании → `Ok(Refutation { basis: "" })`, GREEN `refuted_requires_stated_basis` 1/1; `evidence` 16/16, `human_validation` 1/1, `ledger_sqlite` 5/5, `vertical_n1` 2/2; `cargo build -p stellwerk-task` 0, clippy `-D warnings` 0, `rustfmt --edition 2021 --check` 0; full `cargo test -p stellwerk-task` has pre-existing `workspace_is_rustfmt_clean` failure from untracked V02 `vertical_v2.rs`, outside O04. `Refutation` keeps `basis` + `refuting_data`; `status()` = `TaskStatus::Refuted`, never `Failed`; plain `Validation` remains `HumanValidated`. No `EvidenceKind::Refuted`, no contract change. Accept on clean clone `clone-o04-final-1793635479`: **ПРИНЯТО**; ModelClaim **340**, ToolAttestation **341 Verified**, tail `31f910c4e20e4ef37ee0b8c71d3c53f7d5cf76c5f059b3d8f264ee1362ee8fc5`, evidence commit `651c838`.
  What to do / Must NOT do: `evidence.rs` принимает `Refuted` только с названным основанием и ссылкой на опровергающие данные. Тест `tests/refutation.rs`: `Refuted` без основания отвергается. НЕ превращать `Refuted` в `Failed`.
  Parallelization: Wave 13 | Blocked by: o03 | Blocks: v04
  References: `crates/stellwerk-core/src/contract.rs:185`, `crates/stellwerk-task/src/evidence.rs`.
  Acceptance criteria: `cargo test -p stellwerk-task --test refutation` PASS.
  QA scenarios: happy — основание названо; failure — пустое основание отвергнуто. Evidence `.omo/evidence/task-21-stellwerk-production-v1.txt`
  Commit: Y | `feat(evidence): refutation requires stated basis`

- [x] 22. V03: вертикальное доказательство bindings evidence — ЗАКРЫТО V1/v03b 2026-11-03: deps `o01`/`v02` are merged, and the PolyForge schema probe is clear (entry 309), but `crates/stellwerk-task/src/evidence.rs` `evaluate`/`chain_tail` still ignore `recipe_digest`/`policy_digest`/`commit_sha`; `files_allowed: [tests/vertical_v3.rs]` cannot make a tamper test pass without changing production `evidence.rs` (and its unit test). Scope is outside this packet: open a separate packet with `evidence.rs` + `tests/evidence.rs` + `tests/vertical_v3.rs`; do not weaken the check into a local assertion.
  What to do / Must NOT do: `tests/vertical_v3.rs`: цепочка несёт `recipe_digest`/`policy_digest` и `commit_sha`, и подмена любого digest ломает проверку. Стоп-условие: пробный `evidence_append(kind=ModelClaim, task_id=probe-v03-schema, commit_sha=<git rev-parse HEAD>, diff_hash=sha256 пустой строки, payload={"recipe_digest":"0"*64,"policy_digest":"0"*64})` возвращает ошибку — вернуть `BLOCKED_BY_POLYFORGE_SCHEMA` и не подменять проверку локальной. `payload` в PolyForge — свободная строка (данные не интерпретируются), поэтому отказ означает именно схему, а не «поля нет». НЕ объявлять привязку доказанной без аттестации.
  Parallelization: Wave 11 | Blocked by: o01, v02 | Blocks: v04, i01
  References: `.pf/ledger.jsonl` (payload ToolAttestation), `crates/stellwerk-task/tests/evidence.rs`.
  Acceptance criteria: `cargo test -p stellwerk-cli --test vertical_v3` PASS.
  QA scenarios: happy — digest связан; failure — подмена ломает проверку. Evidence `.omo/evidence/task-22-stellwerk-production-v1.txt`
  Commit: Y | `test(cli): vertical evidence binding`
  ПРОБА СНЯТА (2026-11-01 21:10, HEAD `fea4fa7`): стоп-условие выполнено ДОСЛОВНО и отказа не дало. `evidence_append(kind=ModelClaim, task_id=probe-v03-schema, commit_sha=fea4fa79…, diff_hash=sha256("")=e3b0c442…, payload={"recipe_digest":"0"*64,"policy_digest":"0"*64})` → `{"entry_id":309,"state":"ModelClaimed","hash":"dd3c6c58…"}`. Схема `ModelClaim` принимает запись, `BLOCKED_BY_POLYFORGE_SCHEMA` НЕ заявляется. Сопутствующее измерение: `gate_report(probe-v03-schema)` при единственной `ModelClaim` отдаёт `passed:false` (`tail_hash` = последняя запись), то есть «не аттестовано» отличимо от «опровергнуто» — это и есть состояние, на котором строится O01. Бинарь `polyforge` в `PATH` отсутствует; работает MCP-ручка. Сам todo остаётся `[~]` из-за `o01`/`v02`, а не из-за схемы.
  ЗАКРЫТО 2026-11-03: V1/v03b реализован и принят (`dea1623`, packet fix `e3b2319`); clean-clone `accept.sh V1 packet-v03b` → ПРИНЯТО; PolyForge ModelClaim 347 / ToolAttestation 348 Verified, `gate_evaluate` passed, tail `3fb5fcb8adf1766a797d2276581a1d399a4a434e6a6adcfe0a38ae2374d4a368`. V03 itself прогнан на том же clean clone: `accept.sh V1 packet-v03` → ПРИНЯТО; ModelClaim 349 / ToolAttestation 350 Verified, gate passed, tail `bd0b0b0970e26c57767a3ff85630ee133e01fd7e94b2e2b0c6a83af49ce52824`. Открытый FINDING: `crates/stellwerk-cli/src/lib.rs:225` всё ещё создаёт `ToolAttestation` с `None` bindings; production `evaluate` не вызывается, вне `files_allowed` v03b.

- [x] 23. V04: вертикальное доказательство S-цепочки — ЗАКРЫТО 2026-11-03: commit `e718e9d`, 234 test lines; `cargo test -p stellwerk-cli --test vertical_v4` → 1 passed; `rustfmt --check` clean. V04 доказывает статусную цепочку Suspended → AwaitingHumanValidation → HumanValidated и отдельный Refuted; фактическое evidence-гейтированное возобновление остаётся за S02 (FINDING: `run_once` не читает evidence ledger).
  What to do / Must NOT do: `tests/vertical_v4.rs`: приостановка → человеческая валидация → возобновление → опровержение, каждый шаг с evidence и отличимым статусом. НЕ проходить шаг без записи.
  Parallelization: Wave 14 | Blocked by: o02, o03, o04, v03 | Blocks: s01
  References: `crates/stellwerk-core/src/contract.rs:185`, `crates/stellwerk-task/src/runner.rs`, `crates/stellwerk-task/src/evidence.rs`.
  Acceptance criteria: `cargo test -p stellwerk-cli --test vertical_v4` PASS.
  QA scenarios: happy — вся цепочка; failure — пропуск валидации ломает тест. Evidence `.omo/evidence/task-23-stellwerk-production-v1.txt`
  Commit: Y | `test(cli): vertical suspension-validation-refutation`

- [x] 24. I01: CI-гейт PolyForge вместо заглушки — ЗАКРЫТО 2026-11-03: commit `512d0e0` (3 files; tests 154 lines); `cargo test -p stellwerk-cli --test ci_gate` → 4 passed; `POLYFORGE_CMD=/nonexistent ./scripts/integrator/gate.sh probe` → exit 2 + `MissingPolyForge`; rustfmt/`sh -n`/clippy clean. `gate.sh` uses POSIX sh + python3 stdlib; workflow has no `continue-on-error`.
  What to do / Must NOT do: создать `scripts/integrator/gate.sh` (POSIX sh + python3 stdlib, без новых зависимостей): команда берётся из `POLYFORGE_CMD`, иначе из `modules.toml:14-20` (`command = "polyforge-mcp"`); гейт поднимает её по stdio и шлёт JSON-RPC `initialize` → `notifications/initialized` → `tools/call {"name":"gate_evaluate","arguments":{"task_id":"<arg1>","required":["Verified"]}}`. Exit 0 только при `result.ok:true`; exit 2 и слово `MissingPolyForge`, если команда не найдена (fail-closed — это НЕ успех); иначе exit 1 с телом ответа. Образец протокола — `crates/stellwerk-registry/src/cmd_measure.rs:207-262` (`run_session`), схема аргументов — `cmd_measure.rs:373`. В `.github/workflows/polyforge.yml` заменить `echo TODO` на `run: ./scripts/integrator/gate.sh "${{ github.head_ref }}"`; `continue-on-error` на этом шаге отсутствует. Тест `tests/ci_gate.rs`: (1) поведение — `POLYFORGE_CMD` указывает на фиктивный shell-скрипт: ответ `ok:false` → exit 1, ответ `ok:true` → exit 0; (2) fail-closed — `POLYFORGE_CMD=/nonexistent` → exit 2 и `MissingPolyForge`; (3) smoke — YAML содержит `scripts/integrator/gate.sh` и не содержит `continue-on-error` в job `gate`. НЕ утверждать гейт по наличию строки в YAML; НЕ оставлять `continue-on-error`.
  Parallelization: Wave 12 | Blocked by: v03 | Blocks: i02
  References: `.github/workflows/polyforge.yml`, `modules.toml:14-20`, `crates/stellwerk-registry/src/cmd_measure.rs:207-262,369-375`, `scripts/integrator/accept.sh`.
  Acceptance criteria: `cargo test -p stellwerk-cli --test ci_gate` PASS; `POLYFORGE_CMD=/nonexistent ./scripts/integrator/gate.sh probe` → exit 2.
  QA scenarios: happy — `gate.sh` с отвечающим `ok:true` даёт exit 0; failure — `ok:false` даёт exit 1, отсутствие бинаря даёт exit 2. Evidence `.omo/evidence/task-24-stellwerk-production-v1.txt`
  Commit: Y | `ci(polyforge): gate_evaluate replaces TODO stub`

- [x] 25. I02: негативный тест CI-гейта — ЗАКРЫТО 2026-11-03: commit `9fb97c8` (test-only, 27 lines; file 181 lines); `cargo test -p stellwerk-cli --test ci_gate` → 5 passed; exact packet test `unattested_bundle_returns_exit_one` asserts `ok:false`/`MissingToolAttestation` → exit 1 and `ok:true`/`Verified` → exit 0. Natural RED unavailable: behavior was already implemented by I01; no failure fabricated. rustfmt clean.
  What to do / Must NOT do: `tests/ci_gate.rs` добавляет поведенческую проверку отказа: фиктивный `polyforge-mcp` отвечает `{"result":{"ok":false,"state":"MissingToolAttestation"}}` → `gate.sh` exit 1 и печатает `MissingToolAttestation`; при `{"result":{"ok":true,"state":"Verified"}}` → exit 0. НЕ проверять только подстроку в YAML; НЕ считать exit 0 при `ok:false`; НЕ делать негативный тест мягким (`contains` вместо кода возврата).
  Parallelization: Wave 13 | Blocked by: i01 | Blocks: s01
  References: `.github/workflows/polyforge.yml`, `scripts/integrator/gate.sh`.
  Acceptance criteria: `cargo test -p stellwerk-cli --test ci_gate` PASS.
  QA scenarios: happy — бандл без аттестации даёт exit 1 с названной причиной; failure — `ok:false` с exit 0 ломает тест. Evidence `.omo/evidence/task-25-stellwerk-production-v1.txt`
  Commit: Y | `test(ci): gate rejects unattested bundle`

- [x] 26. S01: автомат приостановки — ЗАКРЫТО 2026-11-03: commit `ee1c1e2`; `runner::suspension_status` maps `Ask` (вопрос есть) → `AwaitingHumanValidation`, `Suspended` (спросить некого) → `Suspended`, `Allow`/`Deny` → `None`; `tests/suspension.rs` 223 lines, 2 tests; `cargo test -p stellwerk-task --test suspension` → 2 passed; runner 16/16, authorization 8/8; fmt/clippy clean; C2 src +10 / tests 223. FINDING: `approval::next_status` still collapses `Ask|Suspended` to `Suspended` (outside `files_allowed`); new automaton not yet wired there.
  What to do / Must NOT do: `runner.rs` реализует `Suspended → AwaitingHumanValidation` только при наличии вопроса человеку и отличимый отказ при его отсутствии. Тест `tests/suspension.rs`: неинтерактивный прогон не зависает и не разрешает. НЕ вводить таймаут как «разрешение».
  Parallelization: Wave 15 | Blocked by: v04, i02 | Blocks: s02
  References: `crates/stellwerk-task/src/runner.rs`, `crates/stellwerk-core/src/contract.rs:185`.
  Acceptance criteria: `cargo test -p stellwerk-task --test suspension` PASS.
  QA scenarios: happy — приостановка названа; failure — таймаут не даёт `Allow`. Evidence `.omo/evidence/task-26-stellwerk-production-v1.txt`
  Commit: Y | `feat(runner): suspension state machine`

- [x] 27. S02: возобновление после человеческой валидации — ЗАКРЫТО 2026-11-03: commit `e0562fe`; `runner::resume_after_human_validation(ledger, task_id)` filters entries by own `task_id` before `evidence_status`; test `resume_only_on_own_task_validation` uses `LeakyLedger` returning all entries, proves foreign Validation → `AwaitingHumanValidation`, own → `HumanValidated`, no record → `AwaitingHumanValidation`, empty task_id → no resume; `cargo test -p stellwerk-task --test human_validation` → 2 passed; suspension 2/2, runner 16/16; fmt/clippy clean; C2 2 files, src +15, test 176 total / +78.
  What to do / Must NOT do: `HumanValidated` разрешает продолжение только с записью и только для того `task_id`. Тест в `tests/human_validation.rs`: валидация чужой задачи не возобновляет. НЕ принимать валидацию без `task_id`.
  Parallelization: Wave 16 | Blocked by: s01 | Blocks: s03
  References: `crates/stellwerk-task/src/runner.rs`, `crates/stellwerk-task/src/evidence.rs`.
  Acceptance criteria: `cargo test -p stellwerk-task --test human_validation` PASS.
  QA scenarios: happy — своя задача возобновлена; failure — чужая валидация отвергнута. Evidence `.omo/evidence/task-27-stellwerk-production-v1.txt`
  Commit: Y | `feat(runner): resume only on own validation`

- [x] 28. S03: опровержение останавливает цепочку — DONE: c2a2688, test-only ALREADY_SATISFIED
  What to do / Must NOT do: `Refuted` запрещает дальнейшие переходы и требует основания. Тест в `tests/refutation.rs`: после `Refuted` переход в `Done` отвергается. НЕ позволять «опровергнуто, но завершено».
  Parallelization: Wave 17 | Blocked by: s02 | Blocks: q01
  References: `crates/stellwerk-task/src/runner.rs`, `crates/stellwerk-core/src/contract.rs:185`.
  Acceptance criteria: `cargo test -p stellwerk-task --test refutation` PASS.
  QA scenarios: happy — цепочка остановлена; failure — `Done` после `Refuted` отвергнут. Evidence `.omo/evidence/task-28-stellwerk-production-v1.txt`
  Commit: Y | `feat(runner): refutation halts the chain`

- [x] 29. Q01: квартир отказанных прогонов — DONE: 3b3aae9
  What to do / Must NOT do: отказанный прогон помещается в карантин с причиной и не смешивается с успешными кандидатами. Тест `tests/quarantine.rs`: отказ даёт запись в карантине и не даёт кандидата. НЕ удалять отказанные артефакты молча.
  Parallelization: Wave 18 | Blocked by: s03 | Blocks: q02
  References: `crates/stellwerk-task/src/runner.rs`, `crates/stellwerk-cli/src/build_runner.rs`.
  Acceptance criteria: `cargo test -p stellwerk-task --test quarantine` PASS.
  QA scenarios: happy — карантин с причиной; failure — отказанный кандидат не попадает в успешные. Evidence `.omo/evidence/task-29-stellwerk-production-v1.txt`
  Commit: Y | `feat(runner): quarantine refused runs`

- [x] 30. Q02: срок хранения карантина — DONE: 894c58c
  What to do / Must NOT do: карантин имеет явный срок и не чистится «по умолчанию». Тест в `tests/quarantine.rs`: запись старше срока помечается истёкшей, но не исчезает без явного решения. НЕ удалять по таймеру.
  Parallelization: Wave 19 | Blocked by: q01 | Blocks: —
  References: `crates/stellwerk-task/src/runner.rs`.
  Acceptance criteria: `cargo test -p stellwerk-task --test quarantine` PASS.
  QA scenarios: happy — истечение помечено; failure — запись исчезла без решения, тест падает. Evidence `.omo/evidence/task-30-stellwerk-production-v1.txt`
  Commit: Y | `feat(runner): explicit quarantine retention`

## Final verification wave

> Runs after ALL todos. ALL must APPROVE. Surface results and wait for the user's explicit okay.

- [x] F1. Plan compliance audit — `python3 scripts/plan-lint.py` exit 0 и в выводе нет строк `[C1]` и `[C9]`; `python3 scripts/packet_replay.py validate` exit 0; `python3 scripts/plan-lint.py --self-test` exit 0; `python3 scripts/plan-lint.py .sisyphus/plans/V1` печатает `V1: 34 пакетов, нарушений нет`.
  ПЕРЕИЗМЕРЕНО 2026-11-04 на `80ed4c1`: `plan-lint.py` exit 0, `[C1]`/`[C9]` 0; `--self-test` → `провалов 0 из 47 проверок`; scoped `.sisyphus/plans/V1` → `V1: 34 пакетов, нарушений нет`; `packet_replay.py validate` exit 0. C8: 16 расхождений зеркала (12 N2 + 4 V1), зеркало не источник; канон — `.sisyphus/plans/V1`. Логи: `.omo/evidence/final-wave-80ed4c1/f1-*.log`.
  ВЕРДИКТ: APPROVE на `80ed4c1` (измерено 2026-11-04) по критериям F1. C8 mirror drift зафиксирован как FINDING и не блокирует F1: `plan-lint` печатает его всегда, а источником истины остаётся канон. Исторический APPROVE на `fea4fa7` (29 пакетов) сохранён в истории.
- [x] F2. Code quality review — `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test` по шести крейтам; ни одного `#[allow]`/`#[ignore]` в диффе. — ПЕРЕИЗМЕРЕНО 2026-11-04 на `80ed4c1`: fmt exit 0, clippy exit 0, six-crate `cargo test` exit 0 (693 passed, 0 failed, 0 ignored; 95 test summaries), `#[allow]`/`#[ignore]`/`skip` в .rs-диффе 0. Логи: `.omo/evidence/final-wave-80ed4c1/f2-*.log`. F2 repair is test-only: `pi_transport.rs` reconciled with a02b raw-content/host-apply and v02b held-bash contracts; `vertical.rs` fixture gained `profile = "pi"`; `audit_live` 7/7. `pi_transport` 15/15, `vertical` 12/12. H1/09 replay suite is outside this six-crate command and remains a separate H1 finding.
  ВЕРДИКТ: APPROVE на `80ed4c1` (измерено 2026-11-04), ранее APPROVE на `42e1e36`. Основание — фактические exit codes выше, а не текст воркера. Дифф затрагивает только тесты: `crates/stellwerk-task/tests/pi_transport.rs` (+90/−11) и `crates/stellwerk-task/tests/vertical.rs` (+1); production-код не менялся. `tool_call_goes_to_socket_with_args` теперь проверяет сырые байты и digest; `apply_host_write` применяет только `AuthorizedCall`; добавлены отказы для held `bash` и non-authorized edit; старые REJECT ниже историчны.
  ИСТОРИЧЕСКИЙ ВЕРДИКТ: REJECT на `fea4fa7` (измерено 2026-11-01 21:20). `cargo fmt --all -- --check` → exit 1, 18 диффов в 9 файлах: `cli/tests/run.rs` (R01), `gateway/src/gate.rs` и `gateway/tests/gate.rs` (A01), `task/src/m5/gate_channel.rs` (A01), `task/tests/authorization.rs` (A03), два файла владельца (`gateway/tests/session_failure.rs`, `gateway/tests/shaping.rs`), два существовавших до работы (`task/tests/http_wire.rs`, `task/tests/model_backend.rs`). `cargo clippy --workspace --all-targets -- -D warnings` → exit 101: `manual implementation of a no-op waker`, `gateway/src/session.rs:613` — тот же дефект, что зафиксирован в P00-базе, то есть до этой работы. Пять файлов из моих коммитов неотформатированы — это долг, который обязан закрыть P01 (todo 3), но P01 `[~]` из-за файлов владельца; отдельным `style:`-коммитом без теста не закрываю (запрет «коммит без теста»).
- [x] F3. Real QA — каждый из 34 пакетов (29 канонических + `packet-00`/`packet-04` + `a01b` + `a02b` + `v02b`, добавленные 2026-11-02) прогнан через `scripts/integrator/accept.sh V1 packet-<id> <sandbox>` (все GREEN, RED-пакетов нет), и для каждого после merge в main выполнен `gate_evaluate(task_id=<id>, required=["Verified"])` с `ok:true`; `gate_report(<id>)` приложен к evidence. Гейт, объявленный пройденным без этого вызова, — провал F3. — PASS 2026-11-04 на `80ed4c1`: clean clone `f3-clone-80ed4c1`; 34/34 accept rc=0 (`f3-accept-80ed4c1.log`); 34/34 `gate_evaluate` → `passed:true`, `missing:[]`, `verified:1`; 34/34 `gate_report` → `passed:true`, единый chain tail `742fbb7b03ef6cfc846a607e8e632566b76d128defedacf6f8a56755efe57cac`; отчёты приложены в `.omo/evidence/f3-gate-reports.md` и `.omo/evidence/f3-gate-reports.json`. Историческое `BLOCKED: 13/33` и `ВЕРДИКТ: REJECT` ниже сохранены как история, но перекрыты этим измерением; счёт пакетов исправлен 33→34.
  ВЕРДИКТ: REJECT на `fea4fa7` (измерено 2026-11-01 21:20). Смёржено 4 пакета из 29 (`p00`, `l01`, `r01`, `03`; `a01` частично, `a03` как страж). `gate_evaluate` не выполнен ни для одного пакета — выполнять его нечего: 23 пакета помечены `[~]`, их цель не существует в main. Прогонять `accept.sh` по неисполненным пакетам и объявлять гейт пройденным значило бы ровно то, что F3 называет провалом. Бинарь `polyforge` в `PATH` отсутствует, но MCP-ручки `evidence_append`/`gate_report`/`gate_evaluate` доступны и проверены пробой (entry 308/309, `passed:false` при единственной `ModelClaim`), поэтому перезапуск F3 после разблокировки не потребует установки CLI.
- [x] F4. Scope fidelity — дифф не содержит `git worktree`, `gix`, `cargo add`, `fixture-engine` в `ALLOWED_PROFILES`, `pi-fixture.sh` в продакшн-путях, `--gating`.
  ВЕРДИКТ: APPROVE на `80ed4c1` (измерено 2026-11-04, база `fea4fa7`, 52 коммита). Проверялось по коду (`crates/**/*.rs`), а не по прозе: добавленных строк с `git worktree` — 0, `gix` — 0, `cargo add` — 0, `--gating` — 0. `ALLOWED_PROFILES` = `&[EngineProfile::new("pi", &[])]` (`execution.rs:97`); `fixture-engine` только в отрицательных тестах и константе `FIXTURE_ENGINE`, не в `ALLOWED_PROFILES`; `pi-fixture.sh` только в тестовых путях и негативных проверках, не в продакшн-путях. Логи: `.omo/evidence/final-wave-80ed4c1/f4-*.diff`.

## Commit strategy

- Один пакет — один коммит; `chore(evidence): attest <packet> after merge` — отдельным коммитом после merge в main (вариант B).
- Порядок: сначала тест-сьютка на main, потом merge, потом evidence.
- Канон пакетов (`V1/packet-*.md`) входит в коммит первого исполняемого пакета.

## Success criteria

1. `just check` и `just check-gate` exit 0.
2. `just plan-lint` exit 0 на всём каноне, без `--gating`.
3. `stellwerk run` не содержит fixture-пути; Pi — единственный движок.
4. Эффект инструмента исполняет хост; digest после записи равен авторизованному.
5. `TaskStatus` различает человеческую валидацию и опровержение явными вариантами.
6. CI-гейт PolyForge вызывает `gate_evaluate` и падает без `ToolAttestation`.
7. Ни одного пакета без нового теста; ни одного гейта, объявленного пройденным без `gate_evaluate`.
8. Ни одного нового C9: каждое пересечение V1 с N1/N2/TUI/M1 объявлено в `depends_on` либо несёт `overlap_waiver: C9` с причиной; `just plan-lint` зелёный без `--gating`.
9. Ни одной вакуумной приёмки: каждая команда приёмки падает, если откатить правку пакета (это и проверяет QA-сценарий failure каждого пакета).
