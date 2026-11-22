# stellwerk-publication-readiness - Work Plan

## TL;DR (For humans)

**What you'll get:** Проект переходит из «работает на одной машине» в «любой может склонировать, собрать и проверить». Появляются лицензии, честный README и quickstart с двумя уровнями проверки, fail-closed `doctor`, чистый HEAD без рантайм-мусора, машинные стражи (дерево, ссылки, секреты во всей истории), честный CI с отдельно названным RED-отчётом и доказуемая портируемость. Публикация идёт в `tensorov/stellwerk` с тегом `v0.0.1`, полной историей и fail-closed preflight.

**Why this approach:** Проверка исполняемая, а не декларативная — каждая претензия подтверждается командой на чистой копии. Строгий гейт отделён от отчёта, который сегодня красный по делу; прятать RED за `|| true` запрещено. Чистится снимок, история публикуется целиком, а её неизбежные следы (имя автора, старый хост, абляции) явно заявляются в SECURITY вместо тихой перезаписи.

**What it will NOT do:** Не переписывает историю (SHA и evidence-цепочка важнее косметики), не публикует в crates.io, не добавляет зависимостей, не чинит записанные FINDING'и и не трогает `contract.rs`/`modules.lock`.

**Effort:** Большой — 23 задачи в 6 волнах (0-я — разбор рабочего дерева после V2), последняя — необратимое внешнее действие.
**Risk:** Средний — почти всё обратимо, кроме публикации; полный тест-уровень требует внешних инструментов и честно возвращает «не проверено» (exit 2), а не ложный ноль.
**Decisions to sanity-check:** (a) публичная подпись — `Aleksei Molchanov`, а в 784 коммитах остаются ранний псевдоним, старые адреса и gmail, (b) старый домен и приватный IP вырезаются из планов, но остаются в истории, (c) RED-отчёт живёт отдельным постоянно красным job'ом.

Your next move: подтвердить три пункта выше и передать план в `$start-work`. Full execution detail follows below.

---

> TL;DR (machine): 23 todos in 6 waves (1/8/6/3/4/1). Wave 0 resolves the tracked tree left by the completed V2 boulder: the tested routing repin (`opencode.json`, `.omo/omo.jsonc`, `tests/test_omo_config.py`) is committed or reverted by its own green/red rule, the V2 checkboxes and this plan are committed as artifacts. Dual MIT/Apache with `Aleksei Molchanov` copyright; workspace metadata + `authors = ["Aleksei Molchanov <aleksei.v.molchanov@gmail.com>"]` + ADR-0025 + ADR-0028; literal `publish = false` in all 7 manifests split 3/3/1 with `cargo metadata` proof; `ledger_encoding` synthetic fixtures (TDD) so untracking cannot break tests; 19 canon packets committed and C8 added to `plan-lint` total plus `--root` drift test; poisoned-mutex → typed `Poisoned`; README/quickstart with tier-1/tier-2; fail-closed `doctor`; untrack artifacts and `.omo` runtime; redact live endpoints (`ssh -p <port> <host>`, private IPv4) from canon+mirror plans; CHANGELOG/SECURITY/CONTRIBUTING with identity, IP-attestation and history-composition statements; dangling refs with documented exemptions; `repo_hygiene` + `scan-history.sh` (all-history secret scan, shallow → exit 2) in `stellwerk-registry` tests; `just check` strict vs `just status`; `ci.yml` hard gate with `fetch-depth: 0` + `spec.yml` `spec (expected RED)` + ADR-0027; polyforge removal after rewriting `ci_gate` (ADR-0026); `verify-fresh-clone.sh` tier-1 (incl. history scan) / tier-2 exit 2; tag `v0.0.1` + idempotent push of only `main` and the tag. F1-F4 gate.

## Scope
### Must have
- Публичный репозиторий `tensorov/stellwerk`, полная история (784 коммита, без squash/rebase/rewrite).
- Правовая оболочка: `LICENSE-MIT` + `LICENSE-APACHE` (copyright `Aleksei Molchanov`, совпадает с `Cargo.toml:7`), `[workspace.package]` metadata c `authors` на gmail, `publish = false` в **каждом из 7** манифестов (`crates/*` + `xtask`).
- README + `docs/quickstart.md` с двумя уровнями: dependency-light гейт и полный прогон с Pi/bwrap/nightly; честный `just doctor` (MISSING vs UNCONFIGURED, fail-closed).
- Гигиена HEAD: рантайм-артефакты не трекаются; `.omo` публикует только `plans/` + `omo.jsonc`; канон `.sisyphus/plans` закоммичен; `plan-lint` **гейтит** C8.
- **Аудит экспозиции:** скан **всей истории** на секреты (fail-closed, shallow → exit 2); живые эндпоинты (`ssh -p <port> <host>`, приватные IPv4) вырезаны из публичных планов в обеих деревьях; в SECURITY заявлены идентичность автора, состав истории и IP-attestation.
- Машинные стражи: `repo_hygiene` (трекаемое дерево, висячие ссылки, эндпоинты) и `scan-history.sh` в зелёном таргете, без новых зависимостей.
- Честный CI: жёсткий гейт повторяет локальный (с `fetch-depth: 0`); RED-сьют `task`+`gateway` — отдельный файл `spec.yml` с именем `spec (expected RED)`, без `continue-on-error`.
- Доказуемая портируемость: `ledger_encoding` не зависит от внутренних журналов; чистый клон проходит tier-1 (включая скан истории); tier-2 требует названных предпосылок и иначе exit 2.
- Релиз: CHANGELOG/SECURITY/CONTRIBUTING, `docs/release.md`, аннотированный тег `v0.0.1`, публикация только `main` + тег.

### Must NOT have (guardrails, anti-slop, scope boundaries)
- Никакой перезаписи истории: публикуются текущие коммиты как есть. Публичная подпись — `Aleksei Molchanov`; адреса автора (старый домен почты, gmail) и ранний псевдоним в 634 коммитах остаются в метаданных навсегда и заявляются в SECURITY. `filter-repo`/squash запрещены, потому что ломают все SHA и привязку ToolAttestation к `commit_sha`.
- Никакого crates.io: `publish = false` буквально в каждом манифесте; `publish.workspace` не вводится (ADR-0025).
- Никаких новых зависимостей; YAML разбирается строками; `cargo metadata` — вызовом, не крейтом; скан истории — `git` + `rg`/POSIX sh, не новый crate.
- Никаких `#[allow(...)]`, `#[ignore]`, `|| true` в обязательных job'ах, `continue-on-error` нигде.
- Не трогаем `crates/stellwerk-core/src/contract.rs`, `modules.lock`, корневой `Cargo.toml` сверх `[workspace.package]` (contract-change + ADR), `.github/workflows/` (contract-change + ADR).
- **Планы санитизируем ровно в одном измерении:** живые эндпоинты (домен, порт, приватный IP) заменяются на `<host>`/`<port>`/`<dev-host>`. Всё остальное в `.omo/plans/**` и `.sisyphus/plans/**` — исторические наряды: `/home/izyabretatel` там разрешён, косметика не делается.
- Не «чиним заодно» `integrator_profile.rs` deny-list и `modules.lock` — документируются, не переделываются.
- Не удаляем runtime-пути `.stellwerk/{inbox,artifacts,pending,plan}` из кода — они остаются рабочими, просто не трекаются.
- Не доверяем `plan-lint` exit 0 без строки `ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном`: C8 сегодня не входит в `total`.
- Не публикуем при грязном дереве; не коммитим и не откатываем чужие незакоммиченные правки (`opencode.json`, `tests/test_omo_config.py`, `.omo/omo.jsonc`, `.omo/plans/stellwerk-production-v2.md`) — при их наличии BLOCKED.
- Не пушим `--all`, `--tags`, `--mirror`: 73 локальные ветки (`ablation-*`, `canary/*`), 37 коммитов вне main и тег `backup/pre-rollback-8a8224d` остаются локальными.

### Recorded, deliberately out of scope (FINDING — не чинить в этом плане)
- `crates/stellwerk-store/src/sqlite.rs:100,152,179,199,228,254,282,306,335,362,387,414,432,463` — 18 × `lock().expect("store mutex poisoned")`.
- `crates/stellwerk-cli/src/tui.rs:179,181,216` — 3 × `lock().unwrap()`.
- `crates/stellwerk-task/src/cli.rs:342,358,385` — `unwrap_or_else(|e| e.into_inner())`: восстановление без названной политики.
- `crates/stellwerk-task/src/ledger_sqlite.rs:102-111` — паника на любой ошибке `read`; после todo 8 отравление остаётся паникой на этом пути.
- `modules.lock:3-6` хранит только `context_cost_tokens`, ревизия не пиннится, `gate.sh` читает только `command` — риск в `docs/release.md`.
- `crates/stellwerk-task/src/m5/integrator_profile.rs:41-45` (`FOREIGN_CONFIGS`) — документируется.
- `docs/p00-baseline.md:21` — исторический снимок («doctor exit 0»); не правим, новое поведение в quickstart.

## Verification strategy
> Zero human intervention - all verification is agent-executed.
- Test decision: **TDD** для любого Rust/скрипт-изменения (RED → GREEN → PROOF). Для документов/лицензий/метаданных — исполняемая приёмка (ADR-0028; ADR-0006 сюда не относится).
- Два уровня: **tier-1** (`just check`, включая `just history`) не требует Pi/bwrap/nightly; **tier-2** (`cargo test --workspace --exclude xtask` + отдельные `xtask`-тесты) требует названного окружения и иначе даёт exit 2, никогда 0.
- Скан истории fail-closed: shallow-клон или отсутствие `git` → exit 2; находки печатаются как `commit:path` без значений секретов.
- Evidence: `.omo/evidence/publication/task-<N>-stellwerk-publication-readiness.md`.
- PROOF по AGENTS: RED-вывод → GREEN-вывод → `evidence_append(ModelClaim)` + `evidence_verify(cmd=<точная команда>)` → `gate_evaluate`.

## Execution strategy
### Parallel execution waves

- **Wave 0 (1)** — рабочее дерево после закрытого V2: `23`. Идёт первой, до любой правки продукта.
- **Wave 1 (8)** — правовая/манифестная оболочка + тест-фикстура + канон: `1,2,3,4,5,6,7,8`.
- **Wave 2 (6)** — тексты, уборка HEAD, редакция эндпоинтов: `9,10,11,12,13,14`. `11` зависит от `6`.
- **Wave 3 (3)** — ссылки, стражи (дерево + история), честный `just check`: `15,16,17`.
- **Wave 4 (4)** — CI-контракт, spec-файл, снятие polyforge, чистый клон: `18,19,20,21`; `21` — хвост волны после `18,19,20`.
- **Wave 5 (1)** — публикация `22`: серийная и последняя, необратимое внешнее действие после зелёной F-волны.

### Dependency matrix
| Todo | Depends on | Blocks | Can parallelize with |
| --- | --- | --- | --- |
| 23 (Wave 0) resolve V2 working tree | — | 22 | — |
| 1 licenses | — | 9 | 2,3,4,5,6,7,8 |
| 2 workspace metadata + ADR-0025/0028 | — | 9,22 | 1,3,4,5,6,7,8 |
| 3 publish=false (core/store/gateway) | — | 22 | 1,2,4,5,6,7,8 |
| 4 publish=false (task/registry/cli) | — | 22 | 1,2,3,5,6,7,8 |
| 5 publish=false (xtask) + metadata proof | 3,4 | 22 | 1,2,6,7,8 |
| 6 ledger_encoding fixture | — | 11,21 | 1,2,3,4,5,7,8 |
| 7 canon commit + C8 gating | — | 16,17,21 | 1,2,3,4,5,6,8 |
| 8 mutex expect + read-panic finding | — | 21 | 1,2,3,4,5,6,7 |
| 9 README + quickstart | 1,2 | 22 | 10,11,12,13,14 |
| 10 honest doctor | — | 21 | 9,11,12,13,14 |
| 11 untrack artifacts | 6 | 15,16,21 | 9,10,12,13,14 |
| 12 untrack .omo runtime | — | 16,21 | 9,10,11,13,14 |
| 13 redact live endpoints in plans | — | 16,21,22 | 9,10,11,12,14 |
| 14 CHANGELOG/SECURITY/CONTRIBUTING | — | 22 | 9,10,11,12,13 |
| 15 dangling refs | 11 | 16,21 | 17 |
| 16 repo_hygiene + history scan | 7,11,12,13,15 | 17,21,22 | — |
| 17 just check honesty + xtask gate | 7,16 | 18,21 | 15 |
| 18 ci.yml + ci_contract + ADR-0027 | 17 | 19,20,21 | — |
| 19 spec.yml expected-RED | 18 | 21 | 20 |
| 20 polyforge removal + ci_gate + ADR-0026 | 18 | 21 | 19 |
| 21 fresh-clone tiers | 6,7,8,10,11,12,13,15,16,17,18,19,20 | 22 | — |
| 22 tag + publish | 2,3,4,5,9,13,14,16,21 | — | — |

## Todos
> Implementation + Test = ONE todo. Never separate.
<!-- APPEND TASK BATCHES BELOW THIS LINE WITH edit/apply_patch - never rewrite the headers above. -->
- [ ] 23. (Wave 0 — выполняется первой; номер 23, потому что todos 1–22 уже согласованы, порядок задаёт волна) Resolve the tracked working tree left by the completed V2 boulder
  What to do: (a) прогнать routing-контракт: `PYTEST_DISABLE_PLUGIN_AUTOLOAD=1 python3 -m pytest tests/test_omo_config.py -q` (сейчас 8 passed; голый `pytest` падает на сломанном плагине `pytest_aiohttp` — это окружение, не тест, поэтому флаг обязателен). (b) при зелёном — закоммитить `opencode.json`, `.omo/omo.jsonc`, `tests/test_omo_config.py` одним config-пакетом (`model`/`small_model` + 183 цепочки агентов + контракт-тест), evidence в `.omo/evidence/publication/task-0-stellwerk-publication-readiness.md`; при красном — `git checkout -- opencode.json .omo/omo.jsonc tests/test_omo_config.py`, записать причину в evidence, routing остаётся на HEAD. (c) закоммитить `.omo/plans/stellwerk-production-v2.md` (тики `[x]` после брудера) и `.omo/plans/stellwerk-publication-readiness.md` отдельным artifact-коммитом. (d) в evidence зафиксировать полный `git status --porcelain` до/после: остаться должны только untracked, которые закрывают todos 7/11/12/16.
  Must NOT do: не трогать untracked (`.omo/evidence`, `.omo/drafts`, `.omo/archive`, `.omo/start-work`, `.playwright-mcp`, `mutants*`, `.sisyphus/plans/{H1,N1,N2,TUI}`) — это работа todos 7/11/12/16; не коммитить `.omo/drafts/**` (приватный, todo 12); не менять код; не пушить; не «дочищать» `.sisyphus`.
  Parallelization: Wave 0 | Blocked by: — | Blocks: 22
  References: `git status --porcelain`; `tests/test_omo_config.py:1-20`; `scripts/check-omo-config.sh:4,15`; `opencode.json:1-3`; `.omo/omo.jsonc:2-199`.
  Acceptance: `test -z "$(git status --porcelain | rg -v '^\?\?')" && PYTEST_DISABLE_PLUGIN_AUTOLOAD=1 python3 -m pytest tests/test_omo_config.py -q | rg -q '8 passed' && git log --format=%s -3 | rg -q 'publication plan'`
  QA scenarios: happy — tracked-модификаций 0, тест `8 passed`; failure — оставить `opencode.json` изменённым, первая проверка непустая. Evidence `.omo/evidence/publication/task-0-stellwerk-publication-readiness.md`
  Commit: Y | `chore(config): repin agent routing to deepseek-v4.1-flash-free` + `chore(plans): add publication plan and tick V2 boulder`

- [ ] 1. Add dual license files
  What to do: `LICENSE-MIT` (MIT, `Copyright (c) 2026 Aleksei Molchanov`) и `LICENSE-APACHE` (полный Apache-2.0, copyright `2026 Aleksei Molchanov`).
  Must NOT do: не трогать `Cargo.toml`; не добавлять badge'ы; не выдумывать второй email.
  Parallelization: Wave 1 | Blocked by: — | Blocks: 9
  References: `Cargo.toml:7` (`license = "MIT OR Apache-2.0"`).
  Acceptance: `test -s LICENSE-MIT && test -s LICENSE-APACHE && rg -q 'Aleksei Molchanov' LICENSE-MIT LICENSE-APACHE && rg -q 'Apache License' LICENSE-APACHE && rg -q 'MIT License' LICENSE-MIT && rg -q 'MIT OR Apache-2.0' Cargo.toml`
  QA scenarios: happy — команды exit 0; failure — удалить `LICENSE-APACHE`, `test -s` даёт 1. Evidence `.omo/evidence/publication/task-1-stellwerk-publication-readiness.md`
  Commit: Y | `docs(license): add MIT and Apache-2.0 texts`

- [ ] 2. Workspace publication metadata + ADR-0025 + ADR-0028
  What to do: в `[workspace.package]` (`Cargo.toml:5-8`) добавить `description`, `repository = "https://github.com/tensorov/stellwerk"`, `homepage`, `readme = "README.md"`, `authors = ["Aleksei Molchanov <aleksei.v.molchanov@gmail.com>"]`; `license`/`edition`/`rust-version` не менять. ADR-0025: метаданные на workspace, `publish = false` буквально в 7 манифестах, наследование не используется. ADR-0028: лицензии/документы/метаданные проверяются исполняемой приёмкой, а не новым Rust-тестом; ADR-0006 (PolyForge/Riegel) не применим.
  Must NOT do: не менять `members`/`[workspace.dependencies]`; не добавлять `publish` в workspace; не переписывать авторов истории (`.mailmap` не меняет raw-коммиты).
  Parallelization: Wave 1 | Blocked by: — | Blocks: 9,22
  References: `Cargo.toml:1-44`; формат — `docs/adr/0022-*.md`.
  Acceptance: `python3 -c "import tomllib;d=tomllib.load(open('Cargo.toml','rb'));p=d['workspace']['package'];assert p['repository']=='https://github.com/tensorov/stellwerk' and p['readme']=='README.md' and p['description'];assert any('Aleksei Molchanov <aleksei.v.molchanov@gmail.com>' in a for a in p['authors']),p['authors'];print('ok')" && cargo metadata --no-deps --format-version 1 >/dev/null && test -s docs/adr/0025-publication-metadata.md && test -s docs/adr/0028-artifact-verification-by-execution.md`
  QA scenarios: happy — ассерты и `cargo metadata`; failure — убрать `authors`, python падает. Evidence `.omo/evidence/publication/task-2-stellwerk-publication-readiness.md`
  Commit: Y | `chore(manifest): workspace publication metadata` (footer `contract-change`, ADR-0025/0028)

- [ ] 3. Mark core/store/gateway internal
  What to do: `publish = false` в `[package]` трёх манифестов.
  Must NOT do: не трогать версии/зависимости; не добавлять `publish` в workspace.
  Parallelization: Wave 1 | Blocked by: — | Blocks: 22
  References: `crates/stellwerk-{core,store,gateway}/Cargo.toml`; ADR-0025.
  Acceptance: `for c in core store gateway; do grep -qx 'publish = false' crates/stellwerk-$c/Cargo.toml || exit 1; done; cargo metadata --no-deps --format-version 1 >/dev/null`
  QA scenarios: happy — цикл 0; failure — убрать строку из `store`, цикл падает. Evidence `.omo/evidence/publication/task-3-stellwerk-publication-readiness.md`
  Commit: Y | `chore(manifest): mark core, store, gateway internal (1/3)`

- [ ] 4. Mark task/registry/cli internal
  What to do: то же для task/registry/cli.
  Must NOT do: не менять `[[bin]]`/features.
  Parallelization: Wave 1 | Blocked by: — | Blocks: 22
  References: те же + ADR-0025.
  Acceptance: `for c in task registry cli; do grep -qx 'publish = false' crates/stellwerk-$c/Cargo.toml || exit 1; done`
  QA scenarios: happy — 3/3; failure — убрать из `cli`, падает. Evidence `.omo/evidence/publication/task-4-stellwerk-publication-readiness.md`
  Commit: Y | `chore(manifest): mark task, registry, cli internal (2/3)`

- [ ] 5. Mark xtask internal + prove publish == [] for all 7 members
  What to do: `publish = false` в `xtask/Cargo.toml`; затем доказать метаданными, что публиковать нечего.
  Must NOT do: не править корневой `Cargo.toml` здесь; не считать «6 крейтов» — пакетов 7.
  Parallelization: Wave 1 | Blocked by: 3,4 | Blocks: 22
  References: `xtask/Cargo.toml:1-10`; `cargo metadata --no-deps`.
  Acceptance: `python3 -c "import json,subprocess;m=json.loads(subprocess.check_output(['cargo','metadata','--no-deps','--format-version','1']));ps=m['packages'];assert len(ps)==7,len(ps);bad=[p['name'] for p in ps if p.get('publish')!=[]];assert not bad,bad;print('publish=[] for',len(ps))"`
  QA scenarios: happy — `publish=[] for 7`; failure — убрать строку из `xtask`, `bad=['xtask']`. Evidence `.omo/evidence/publication/task-5-stellwerk-publication-readiness.md`
  Commit: Y | `chore(manifest): mark xtask internal (3/3)`

- [ ] 6. Freeze ledger encoding baseline in fixtures (test-first)
  What to do: `xtask/tests/ledger_encoding.rs` читает `.stellwerk/FINDINGS.md` и `.night/STATE.md` и падает при их отсутствии (`:14,25-26,41-49`). Сначала поменять `BASELINE` на `xtask/tests/fixtures/ledger_encoding/{FINDINGS.md,STATE.md}` → RED, затем создать два **минимальных** синтетических файла с тем же числом `U+FFFD` (4 символа/2 строки и 2/1). В doc-комментарии: живой журнал внутренний и не публикуется, фикстура замораживает исторический baseline.
  Must NOT do: не копировать 5005-строчный `FINDINGS.md`; не ставить `#[ignore]`/рантайм-skip; не удалять `a_fresh_replacement_char_is_detected`.
  Parallelization: Wave 1 | Blocked by: — | Blocks: 11,21
  References: `xtask/tests/ledger_encoding.rs:12-51`; ADR-0003 (счёт строк).
  Acceptance: `cargo test -p xtask --test ledger_encoding && rg -q 'fixtures/ledger_encoding' xtask/tests/ledger_encoding.rs && wc -l xtask/tests/fixtures/ledger_encoding/*.md | tail -1`
  QA scenarios: happy — тест зелёный, фикстуры ≤3 строк; failure — удалить фикстуру, тест падает. Evidence `.omo/evidence/publication/task-6-stellwerk-publication-readiness.md`
  Commit: Y | `test(xtask): freeze ledger encoding baseline in fixtures`

- [ ] 7. Commit canon packets and make C8 gate the lint (test-first)
  What to do: (a) `git add .sisyphus/plans/H1 .sisyphus/plans/N1 .sisyphus/plans/N2 .sisyphus/plans/TUI` (19 файлов; artifact). (b) RED: в `crates/stellwerk-cli/tests/plan_lint.rs` — temp-дерево canon+mirror с расходящимся `depends_on`, вызов через `--root`, ожидание exit 1. (c) GREEN: `scripts/plan-lint.py` — `--root` и `total += len(c8)`; расширить `plan_lint_is_green` проверкой `ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном` и отсутствием `расхождений`.
  Must NOT do: не принимать exit 0 без строки зеркала; не считать 19 «пакетами»; не трогать `_archive`.
  Parallelization: Wave 1 | Blocked by: — | Blocks: 16,17,21
  References: `scripts/plan-lint.py:1202-1210,1234`; `crates/stellwerk-cli/tests/plan_lint.rs:21-47`; self-test C8 `:1006-1013`.
  Acceptance: `cargo test -p stellwerk-cli --test plan_lint && python3 scripts/plan-lint.py --self-test && ./scripts/plan-lint.sh | grep -Fq 'ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном' && ! ./scripts/plan-lint.sh | grep -q 'расхождений' && test -z "$(git status --porcelain .sisyphus)"`
  QA scenarios: happy — тест + self-test + строки; failure — удалить один mirror-пакет, новый тест падает. Evidence `.omo/evidence/publication/task-7-stellwerk-publication-readiness.md`
  Commit: Y | `fix(plan-lint): gate mirror drift` + `docs(plans): commit H1/N1/N2/TUI canon`

- [ ] 8. Typed error on poisoned mutex (3 sites, TDD)
  What to do: тестом первым добавить `Poisoned` в ошибку леджера; заменить `lock().expect(...)` на `map_err(|_| Poisoned)` в `ledger_sqlite.rs:65,93` и `build_runner.rs:639`.
  Must NOT do: не трогать 18 сайтов `store/sqlite.rs`, `tui.rs`, `task/cli.rs`; не менять trait сигнатуры; не оставлять панику без записи.
  Parallelization: Wave 1 | Blocked by: — | Blocks: 21
  References: `crates/stellwerk-task/src/ledger_sqlite.rs:65,93`; `crates/stellwerk-cli/src/build_runner.rs:639`; `ledger_sqlite.rs:102-111` (FINDING).
  Acceptance: `cargo test -p stellwerk-task --lib ledger_sqlite && cargo test -p stellwerk-cli --lib build_runner && ! rg -n 'lock\(\)\.(expect|unwrap)' crates/stellwerk-task/src/ledger_sqlite.rs crates/stellwerk-cli/src/build_runner.rs`
  QA scenarios: happy — тест на `Poisoned` зелёный; failure — вернуть `expect`, тест падает. Evidence `.omo/evidence/publication/task-8-stellwerk-publication-readiness.md`
  Commit: Y | `fix(ledger): return typed error on poisoned mutex`

- [ ] 9. README + docs/quickstart.md
  What to do: README (сейчас 10 строк, «M0»): one-liner, pre-1.0, 6 крейтов, **два бинаря** (`stellwerk`, `stellwerk-task`) + `cargo xtask` (алиас в `.cargo/config.toml`), явно: `stellwerk task` не существует, задача — `stellwerk-task task ...`. quickstart: `rust-toolchain.toml` пинит **1.99.0**, MSRV `Cargo.toml:8` — **1.88**; prerequisites (rustup+rustfmt+clippy, `nightly-2026-10-01` только для мутаций, `python3`, `bwrap`, Pi 1.0.2 через `PI_BIN`/`$HOME/.pi/agent/bin/pi`, `STELLWERK_PI_EXTENSION`, token-файл, `gh`); **tier-1** `just check` (включая `just history`) и **tier-2** `cargo test --workspace --exclude xtask` + `cargo test -p xtask --test ledger_encoding`; команды сборки/запуска и примеры; строка: untracked = HEAD, история содержит внутренние артефакты (ссылка на SECURITY).
  Must NOT do: не заявлять crates.io; не писать «`just check` = весь test»; не обещать «чистый клон без предпосылок»; badge, который 404.
  Parallelization: Wave 2 | Blocked by: 1,2 | Blocks: 22
  References: `rust-toolchain.toml:2`; `Cargo.toml:8`; `justfile:15-82`; `crates/stellwerk-cli/examples/n1-smoke.toml`; `crates/stellwerk-cli/src/lib.rs:91-114`.
  Acceptance: `test -f docs/quickstart.md && rg -q 'stellwerk-task' README.md docs/quickstart.md && rg -q 'STELLWERK_PI_EXTENSION' docs/quickstart.md && rg -q 'PI_BIN' docs/quickstart.md && rg -q '1.99.0' docs/quickstart.md && rg -q '1.88' docs/quickstart.md && rg -q 'tier-1|tier 1' docs/quickstart.md && rg -q 'cargo test --workspace --exclude xtask' docs/quickstart.md && rg -q 'LICENSE-MIT' README.md && ! rg -q 'crates\.io' README.md docs/quickstart.md`
  QA scenarios: happy — все `rg`; failure — убрать `tier-1`, ассерт падает. F3 исполняет tier-1 команды. Evidence `.omo/evidence/publication/task-9-stellwerk-publication-readiness.md`
  Commit: Y | `docs(readme): public quickstart with two verification tiers`

- [ ] 10. Honest `just doctor` (fail-closed) + test
  What to do: вынести проверки в `scripts/doctor.sh` (POSIX), рецепт `doctor` = вызов. `cargo`, `rustc`, `opencode`, `polyforge-mcp`, `gh` (мягко), Pi (`PI_BIN` или `$HOME/.pi/agent/bin/pi`), `STELLWERK_PI_EXTENSION` (OK/MISSING/UNCONFIGURED), token-файл читаем и непуст, `bwrap`. Обязательное отсутствует → `<name>: MISSING|UNCONFIGURED (<hint>)`, exit 1. `scripts/tests/test_doctor.sh`: happy со стабами; failure `PI_BIN=/nonexistent`; снятый extension → UNCONFIGURED.
  Must NOT do: не «ок» на отсутствие; не `|| true`; не требовать сеть; не править `docs/p00-baseline.md`.
  Parallelization: Wave 2 | Blocked by: — | Blocks: 21
  References: `justfile:76-82`; `crates/stellwerk-cli/src/lib.rs:91-114`.
  Acceptance: `bash scripts/tests/test_doctor.sh && (env PI_BIN=/nonexistent bash scripts/doctor.sh; test $? -eq 1) && (env -u STELLWERK_PI_EXTENSION bash scripts/doctor.sh 2>&1 | grep -q UNCONFIGURED)`
  QA scenarios: happy — стабы 0; failure — `/nonexistent` даёт 1 и `pi: MISSING`. Evidence `.omo/evidence/publication/task-10-stellwerk-publication-readiness.md`
  Commit: Y | `feat(doctor): fail closed on missing runtime prerequisites`

- [ ] 11. Untrack internal run artifacts + ignore rules
  What to do: после todo 6 `git rm -r --cached .night .stellwerk .ablation-pi mutation-evidence PLACEHOLDER docs/diagnostics scripts/ablation/__pycache__`; `.gitignore` += `/.night/`, `/.stellwerk/`, `/.ablation-pi/`, `/mutation-evidence/`, `/PLACEHOLDER/`, `/docs/diagnostics/`, `**/mutants.out*/`.
  Must NOT do: не удалять с диска; не трогать `.omo/plans`; не добавлять `/.ablation-solpi/`/`/.ablation-omp/`.
  Parallelization: Wave 2 | Blocked by: 6 | Blocks: 15,16,21
  References: `git ls-files`; `.gitignore:11-51`; runtime-пути `.stellwerk/{inbox,artifacts,pending,plan}` (`session.rs:216,300`, `approval.rs:17`, `cmd_update.rs:11,93,110,464,496,540`).
  Acceptance: `! git ls-files | rg '^(\.night/|\.stellwerk/|\.ablation-pi/|mutation-evidence/|PLACEHOLDER/|docs/diagnostics/|.*__pycache__)' && git check-ignore -q .night/STATE.md && git check-ignore -q .stellwerk/inbox/x && git check-ignore -q mutation-evidence/x && test -f .night/STATE.md && cargo test -p xtask --test ledger_encoding`
  QA scenarios: happy — все проверки; failure — `git add -f .night/STATE.md`, ассерт падает. Evidence `.omo/evidence/publication/task-11-stellwerk-publication-readiness.md`
  Commit: Y | `chore(repo): untrack internal run artifacts`

- [ ] 12. Keep OMO plans public, runtime private
  What to do: `git rm -r --cached .omo/evidence .omo/run-continuation .omo/drafts`; `.gitignore` += `.omo/evidence/`, `.omo/drafts/`, `.omo/archive/`; оставить `.omo/plans/**` и `.omo/omo.jsonc`.
  Must NOT do: не трогать `.omo/plans`/`_archive`; не удалять evidence с диска.
  Parallelization: Wave 2 | Blocked by: — | Blocks: 16,21
  References: `git ls-files .omo`; `.gitignore:42-51`.
  Acceptance: `test -z "$(git ls-files .omo | rg -v '^(\.omo/omo\.jsonc|\.omo/plans/)')" && test -z "$(git ls-files .omo/archive)" && git check-ignore -q .omo/evidence/x && git check-ignore -q .omo/drafts/x`
  QA scenarios: happy — пусто; failure — `git add -f .omo/evidence/x`, ассерт падает. Evidence `.omo/evidence/publication/task-12-stellwerk-publication-readiness.md`
  Commit: Y | `chore(omo): publish plans only`

- [ ] 13. Redact live endpoints from public plans (canon + mirror)
  What to do: в `.omo/plans/TUI/packet-00..04.md`, `.omo/plans/tui.md` и `.sisyphus/plans/TUI/packet-00..04.md` заменить живые эндпоинты на плейсхолдеры: домен → `<host>`, порт → `<port>`, приватный IPv4 → `<dev-host>`; строки вида `ssh -p <port> <host>` и `hostname <name>` переписать с плейсхолдерами. Редакция касается **только** живых эндпоинтов; остальной текст не трогать. В шапке TUI packet-00 добавить строку: «эндпоинты заменены плейсхолдерами при публикации; старый домен больше не принадлежит автору».
  Must NOT do: не менять `depends_on`/`risk_class`/`files`/`deliverable` (C8); не трогать `/home/izyabretatel` (идентичность принята); не править ADR; не пытаться «почистить» историю.
  Parallelization: Wave 2 | Blocked by: — | Blocks: 16,21,22
  References: `rg -n 'ssh -p|192\.168\.' .omo/plans .sisyphus/plans`.
  Acceptance: `! rg -q 'ssh -p [0-9]+ [a-z0-9.-]+\.(ru|com|net|io)' .omo/plans .sisyphus/plans && ! rg -q '(^|[^0-9])192\.168\.[0-9]' .omo/plans .sisyphus/plans && rg -q '<host>' .omo/plans/TUI/packet-00.md && ./scripts/plan-lint.sh`
  QA scenarios: happy — `rg` пуст, `plan-lint` 0 и зеркало совпадает; failure — вернуть домен в один пакет, первый `rg` падает. Evidence `.omo/evidence/publication/task-13-stellwerk-publication-readiness.md`
  Commit: Y | `docs(plans): redact live endpoints before publication`

- [ ] 14. CHANGELOG, SECURITY, CONTRIBUTING
  What to do: `CHANGELOG.md` (Keep a Changelog, `[Unreleased]`, `[0.0.1] — 2026-11-19`); `SECURITY.md` — поддержка только main, приватные GitHub advisories, контакт `aleksei.v.molchanov@gmail.com`; **три явных заявления**: (1) идентичность: автор `Aleksei Molchanov`, публичный хэндл `tensorov`; (2) состав истории: HEAD очищен, история содержит имя/адреса автора (старый домен почты, gmail), ранние коммиты под псевдонимом, старый хост/порт, абляции и имена моделей, `Co-authored-by: Sisyphus`, внутренние журналы; rewrite не выполнялся; (3) IP-attestation: «код написан вне рамок найма, это личный open-source проект, права принадлежат автору». `CONTRIBUTING.md` — TDD, packet ≤3 файла/C2, `just check` vs `just status`, запрет `#[allow]`/`cargo add`, порядок main→worktree→merge→attest.
  Must NOT do: не обещать crates.io; не указывать несуществующий email; не заявлять «история очищена»; не дублировать ADR.
  Parallelization: Wave 2 | Blocked by: — | Blocks: 22
  References: `AGENTS.md`; `docs/adr/`; `justfile`.
  Acceptance: `test -s CHANGELOG.md && test -s SECURITY.md && test -s CONTRIBUTING.md && rg -q '## \[Unreleased\]' CHANGELOG.md && rg -q '## \[0\.0\.1\]' CHANGELOG.md && rg -qi 'advisories|vulnerability' SECURITY.md && rg -q 'aleksei.v.molchanov@gmail.com' SECURITY.md && rg -q 'Aleksei Molchanov' SECURITY.md && rg -q 'вне рамок найма|outside.*employment' SECURITY.md && rg -q 'история' SECURITY.md && rg -q 'псевдоним' SECURITY.md && rg -q 'just status' CONTRIBUTING.md`
  QA scenarios: happy — все ассерты; failure — убрать `[0.0.1]`, падает. Evidence `.omo/evidence/publication/task-14-stellwerk-publication-readiness.md`
  Commit: Y | `docs: add changelog, security policy, contributing guide`

- [ ] 15. Replace links to unpublished journals in code and docs
  What to do: `crates/stellwerk-core/src/hexfmt.rs:4` (`.stellwerk/DUPLICATES.md`) и `crates/stellwerk-store/src/lib.rs:14` (`.night/STATE.md`) — «внутренний журнал (не публикуется)»; `docs/PIVOT.md:114` + преамбула — та же формулировка и строка: ADR-0005/0006/0013, планы и `scripts/worker/*` ссылаются на исторический архив, их не трогаем.
  Must NOT do: не править ADR и планы; не менять `scripts/worker/*`; не менять поведение; не сканировать рабочий каталог обычным `rg` — `docs/diagnostics/**` снят с индекса в todo 11, но остаётся на диске, поэтому приёмка идёт по tracked-файлам (`git grep`).
  Parallelization: Wave 3 | Blocked by: 11 | Blocks: 16,21
  References: `hexfmt.rs:4`; `store/lib.rs:14`; `docs/PIVOT.md:114`; `docs/adr/0005:178`, `0006:92`, `0013:39,182`; `.omo/plans/N1/index.md:6`; `.sisyphus/plans/TUI/packet-0*.md`.
  Acceptance: `! git grep -qE '\.night/|\.stellwerk/(FINDINGS|DUPLICATES)' -- crates docs ':(exclude)docs/adr' && cargo test -p stellwerk-core -p stellwerk-store --lib`
  QA scenarios: happy — `rg` пуст, тесты зелёные; failure — вернуть ссылку, `rg` падает. Evidence `.omo/evidence/publication/task-15-stellwerk-publication-readiness.md`
  Commit: Y | `docs: replace links to unpublished internal journals`

- [ ] 16. Repository hygiene guard + all-history secret scan
  What to do: (a) `crates/stellwerk-registry/tests/repo_hygiene.rs`: `git ls-files -z`; нет классов `.night/`, `.stellwerk/`, `.ablation-pi/`, `mutation-evidence/`, `PLACEHOLDER/`, `docs/diagnostics/`, `.omo/archive/`, `__pycache__/`, `mutants.out`; вне `.omo/plans/**`/`.sisyphus/plans/**` нет `/home/izyabretatel`; `LICENSE-*`/`SECURITY.md`/`.omo/omo.jsonc` трекаются; в `.omo/{evidence,run-continuation,drafts}/` нет трекаемых; в `crates/**/*.rs` и `docs/**` (кроме `docs/adr/**`) нет ссылок на снятые пути; **в планах нет живых эндпоинтов** (`ssh -p <digits> <host>`, приватные IPv4). (b) `scripts/scan-history.sh`: перебрать все blobs (`git rev-list --objects --all` + `git cat-file --batch`); паттерны `ghp_`, `github_pat_`, `gho_`, `xox[baprs]-`, `AKIA[0-9A-Z]{16}`, `AIza…`, `sk-[A-Za-z0-9]{20,}`, `-----BEGIN [A-Z ]*PRIVATE KEY-----`, `(API_KEY|TOKEN|SECRET|PASSWORD)\s*=\s*[A-Za-z0-9/+_-]{20,}`, `Bearer [A-Za-z0-9._-]{30,}`; печатать только `commit:path`; exit 0 clean / 1 findings / **2 если репозиторий shallow или нет `git`**. (c) `scripts/tests/test_scan_history.sh`: temp-репо с фейковым `ghp_…` в коммите → exit 1 и значение не печатается; чистый → 0; shallow → 2.
  Must NOT do: не тянуть зависимости; не проверять untracked; не печатать найденные значения; не хардкодить 117 имён; не трактовать «скан не запустился» как «чисто».
  Parallelization: Wave 3 | Blocked by: 7,11,12,13,15 | Blocks: 17,21,22
  References: `git ls-files`; `.gitignore`; `.git/shallow`; исключения из Scope.
  Acceptance: `cargo test -p stellwerk-registry --test repo_hygiene && bash scripts/tests/test_scan_history.sh && bash scripts/scan-history.sh && test "$(git rev-parse --is-shallow-repository)" = false`
  QA scenarios: happy — 3 команды; failure — `git add -f mutants.out/x` → guard падает; фейковый `ghp_` в temp → скан exit 1. Evidence `.omo/evidence/publication/task-16-stellwerk-publication-readiness.md`
  Commit: Y | `test(repo): guard the public tree and scan all history`

- [ ] 17. Separate the strict gate from the RED report (`just` + `xtask gate`)
  What to do: `check` = `check-gate check-risk-tables history scripts public-tree` (без `test-status`); `status` = `test-status` + пояснение «informational»; `history` = `bash scripts/scan-history.sh`; `scripts` = inline-цикл `for f in scripts/tests/*.sh; do bash "$$f"; done`; `public-tree` = `cargo test -p stellwerk-registry --test repo_hygiene`. `xtask/src/main.rs` `gate` → `just check status`; строка в README (`just check` — строгий гейт, `just status` — сводка).
  Must NOT do: не менять тело `test-status`; не `|| true`; не ломать `check-status` (перевести на `status`).
  Parallelization: Wave 3 | Blocked by: 7,16 | Blocks: 18,21
  References: `justfile:15-29,45-65`; `xtask/src/main.rs:91-96`; `README.md:9`.
  Acceptance: `just --dry-run check | rg -q 'scan-history\.sh' && just --dry-run check | rg -q 'repo_hygiene' && ! just --dry-run check | rg -q 'test-status' && just --dry-run status | rg -q 'test-status' && just --dry-run check-status | rg -q 'test-status' && just history && just scripts && cargo test -p stellwerk-registry --test repo_hygiene && rg -q 'just check status' xtask/src/main.rs`
  QA scenarios: happy — всё 0; failure — вернуть `test-status` в `check`, ассерт падает. Evidence `.omo/evidence/publication/task-17-stellwerk-publication-readiness.md`
  Commit: Y | `chore(just): separate strict gate from informational report`

- [ ] 18. CI hard gate + contract test + ADR-0027
  What to do: `.github/workflows/ci.yml` — обязательные job'ы `fmt`, `clippy`, `registry`, `core`, `store`, `risk-tables`, `scripts`, `repo-hygiene`, `history`, `plan-lint`; `actions/checkout` с `fetch-depth: 0` (иначе скан истории даёт shallow → exit 2); без `|| true`, без `continue-on-error`; toolchain `1.99.0` (не `@stable`). Создать `crates/stellwerk-registry/tests/ci_contract.rs` (строковые ассерты) и `docs/adr/0027-ci-mirrors-local-gate.md`: жёсткий гейт = tier-1, RED-сьют — отдельный файл (todo 19), обязательные checks живут в branch protection, не в YAML.
  Must NOT do: не менять версии actions; не пинить счётчики тестов; не добавлять Pi/bwrap в жёсткий гейт; не запускать history-скан без полной истории.
  Parallelization: Wave 4 | Blocked by: 17 | Blocks: 19,20,21
  References: `.github/workflows/ci.yml:1-60`; `justfile:20-29,45-65`; `crates/stellwerk-cli/tests/ci_gate.rs` (прецедент).
  Acceptance: `cargo test -p stellwerk-registry --test ci_contract && ! rg -q '\|\| true' .github/workflows/ci.yml && ! rg -q 'continue-on-error' .github/workflows/ci.yml && rg -q 'fetch-depth: 0' .github/workflows/ci.yml && rg -q 'cargo test -p stellwerk-registry' .github/workflows/ci.yml && rg -q '1\.99\.0' .github/workflows/ci.yml`
  QA scenarios: happy — тест + 6 `rg`; failure — убрать `fetch-depth: 0`, тест падает. Evidence `.omo/evidence/publication/task-18-stellwerk-publication-readiness.md`
  Commit: Y | `ci: mirror the local strict gate` (footer `contract-change`, ADR-0027)

- [ ] 19. Explicit expected-RED spec workflow
  What to do: `.github/workflows/spec.yml`: job name `spec (expected RED)`; push/PR; `fetch-depth: 0`; `apt-get install -y bubblewrap python3`; `cargo test -p stellwerk-task --locked --no-fail-fast` и `-p stellwerk-gateway`; **без** `continue-on-error`; не входит в branch protection. Расширить `ci_contract.rs`: spec.yml существует, имя содержит `expected RED`, без `continue-on-error`, запускает ровно task+gateway, `ci.yml` его не дублирует.
  Must NOT do: не добавлять `|| true`; не включать task/gateway в `ci.yml`; не притворяться, что workflow зелёный.
  Parallelization: Wave 4 | Blocked by: 18 | Blocks: 21
  References: `justfile:52-60`; `ci_contract.rs` (todo 18).
  Acceptance: `cargo test -p stellwerk-registry --test ci_contract && test -s .github/workflows/spec.yml && rg -q 'expected RED' .github/workflows/spec.yml && ! rg -q 'continue-on-error' .github/workflows/spec.yml && rg -q 'stellwerk-task' .github/workflows/spec.yml && rg -q 'stellwerk-gateway' .github/workflows/spec.yml`
  QA scenarios: happy — тест + `rg`; failure — добавить `continue-on-error`, тест падает. Evidence `.omo/evidence/publication/task-19-stellwerk-publication-readiness.md`
  Commit: Y | `ci: name the expected-RED spec suite` (footer `contract-change`, ADR-0027)

- [ ] 20. Remove the dead polyforge workflow (test-first) + ADR-0026
  What to do: RED: `crates/stellwerk-cli/tests/ci_gate.rs:165-181` читает `.github/workflows/polyforge.yml`; переписать тест на новый контракт — файла нет, `scripts/integrator/gate.sh` остаётся fail-closed (тесты 155-163 не трогать). GREEN: удалить workflow; ADR-0026: gate требует `polyforge-mcp` + `task_id`, в Actions нет ни того, ни другого; gate локален.
  Must NOT do: не менять `gate.sh`; не переносить гейт в CI; не удалять тест «fail-closed без polyforge».
  Parallelization: Wave 4 | Blocked by: 18 | Blocks: 21
  References: `crates/stellwerk-cli/tests/ci_gate.rs:155-181`; `.github/workflows/polyforge.yml`; `scripts/integrator/gate.sh:78-80`.
  Acceptance: `cargo test -p stellwerk-cli --test ci_gate && test ! -e .github/workflows/polyforge.yml && test -s docs/adr/0026-local-gate-not-github-action.md && ! git grep -q 'polyforge\.yml'`
  QA scenarios: happy — тест зелёный, файла нет; failure — вернуть файл, тест падает. Evidence `.omo/evidence/publication/task-20-stellwerk-publication-readiness.md`
  Commit: Y | `ci: remove the dead polyforge workflow` (footer `contract-change`, ADR-0026)

- [ ] 21. Fresh-clone proof with two tiers
  What to do: `scripts/verify-fresh-clone.sh` (`set -euo pipefail`, trap cleanup): clone `--no-local` в `mktemp -d`; **tier-1** всегда: `cargo build --locked -p stellwerk-cli --bin stellwerk`, `./target/debug/stellwerk --help`, `just check` (включая history), `bash scripts/scan-history.sh`, `cargo test -p xtask --test ledger_encoding`, `git ls-files .omo/evidence | wc -l` = 0; **tier-2** только при rustup+`nightly-2026-10-01`+python3+bwrap+Pi(`PI_BIN`)+`STELLWERK_PI_EXTENSION`+token: `cargo test --workspace --exclude xtask --locked`; иначе `TIER-2 SKIPPED: <missing…>` и **exit 2**. Печать `FRESH CLONE: OK (tier-1)` / `OK (tier-1+tier-2)`.
  Must NOT do: не ставить nightly; не тянуть сеть сверх cargo; не возвращать 0 при отсутствии предпосылок tier-2; не клонировать shallow.
  Parallelization: Wave 4 (хвост, после 18,19,20) | Blocked by: 6,7,8,10,11,12,13,15,16,17,18,19,20 | Blocks: 22
  References: `xtask/tests/mutation_toolchain.rs:38-47`; `crates/stellwerk-cli/tests/run.rs:811-824`; `build_runner.rs:186-195`; `crates/stellwerk-task/tests/{integrator_profile.rs:83,secrets.rs:209}`; `rust-toolchain.toml`.
  Acceptance: `bash scripts/verify-fresh-clone.sh; test $? -eq 0 -o $? -eq 2` и в выводе `FRESH CLONE: OK` или `TIER-2 SKIPPED` с причиной
  QA scenarios: happy — tier-1 зелёный, tier-2 либо зелёный, либо exit 2 с именами; failure — добавить `/home/izyabretatel` в `docs/`, guard в клоне падает. Evidence `.omo/evidence/publication/task-21-stellwerk-publication-readiness.md`
  Commit: Y | `test(release): prove a clean clone with named tiers`

- [ ] 22. Tag v0.0.1 and publish tensorov/stellwerk
  What to do: preflight fail-closed: `test -z "$(git status --porcelain)"`; ветка `main`; `just check`; `bash scripts/scan-history.sh`; `bash scripts/verify-fresh-clone.sh` (0 или 2); `gh auth status`; `gh api repos/tensorov/stellwerk` → **404 = отсутствует**, любой другой ненулевой код (401/403/5xx/сеть) = BLOCKED; `git remote -v` пуст; `git tag -l v0.0.1` пуст; `git config user.name` = `Aleksei Molchanov`, `git config user.email` = `aleksei.v.molchanov@gmail.com`, `git log -1 --format='%an <%ae>'` = `Aleksei Molchanov <aleksei.v.molchanov@gmail.com>`. Затем `git tag -a v0.0.1 -m "stellwerk 0.0.1"`; `gh repo create tensorov/stellwerk --public --source=. --remote=origin --push` (только текущая ветка); `git push origin v0.0.1`; проверка `git ls-remote --heads --tags origin` = ровно 2 строки (`refs/heads/main`, `refs/tags/v0.0.1`). Повторный запуск при существующем непустом репозитории — отказ.
  Must NOT do: force-push; `git push --all|--tags|--mirror`; менять owner/repo; продолжать без `gh auth`; публиковать при грязном дереве; добавлять новые коммиты в конфиг/планы сверх todo 23 — при tracked-модификациях BLOCKED.
  Parallelization: Wave 5 | Blocked by: 2,3,4,5,9,13,14,16,21,23 | Blocks: —
  References: `docs/release.md`; `gh` CLI; `Cargo.toml:4`.
  Acceptance: `gh repo view tensorov/stellwerk --json visibility,defaultBranchRef -q '.visibility+" "+.defaultBranchRef.name' | grep -qx 'PUBLIC main' && test "$(git ls-remote --heads --tags origin | wc -l)" -eq 2 && git ls-remote --tags origin v0.0.1 | grep -q v0.0.1`
  QA scenarios: happy — PUBLIC main + ровно 2 refs; failure — `gh auth logout` → остановка до tag/push; существующий непустой repo → отказ; `git push --tags` → ассерт на 2 refs падает. Evidence `.omo/evidence/publication/task-22-stellwerk-publication-readiness.md`
  Commit: N | внешнее действие; evidence отдельным `chore(evidence): attest publication`

## Final verification wave
> Runs in parallel after ALL todos. ALL must APPROVE. Surface results and wait for the user's explicit okay before declaring complete.
- [ ] F1. Plan compliance audit — все 23 evidence-файла существуют; `./scripts/plan-lint.sh` exit 0 **и** строка «совпадает с каноном»; `git status --porcelain` содержит только ожидаемое; `git ls-files` без классов из todos 11/12; канон закоммичен (`git status .sisyphus` пуст); в публичных планах нет живых эндпоинтов.
- [ ] F2. Code quality review — `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p xtask --test ledger_encoding`, `just check`, `cargo test -p stellwerk-registry --test repo_hygiene --test ci_contract`, `bash scripts/scan-history.sh`; `! rg -n '#\[(allow|ignore)' crates/ xtask/`; `! rg -n 'lock\(\)\.(expect|unwrap)' crates/stellwerk-task/src/ledger_sqlite.rs crates/stellwerk-cli/src/build_runner.rs`.
- [ ] F3. Real manual QA — `bash scripts/verify-fresh-clone.sh`; tier-1 команды quickstart исполнены; `stellwerk --help`/`stellwerk-task --help`; `just doctor` на неполном окружении падает с MISSING/UNCONFIGURED; `just status` печатает RED, `just check` — нет; `cargo metadata` даёт `publish == []` для 7 пакетов.
- [ ] F4. Scope fidelity — нет новых зависимостей; `contract.rs`/`modules.lock` не тронуты; workflows менялись только в 18/19/20 с ADR; история не переписана (`git log --oneline | wc -l` ≥ 784); `/home/izyabretatel` вне планов/ADR отсутствует; **живые эндпоинты вырезаны, история проверена сканером, состав истории (имя, адреса, хост, абляции, модели, co-author) заявлен в SECURITY**; IP-attestation на месте; на remote ровно `main` + `v0.0.1`.

## Commit strategy
- Один todo = один коммит; исключения: 23 (два коммита — config и artifact) и 22 (внешнее действие + отдельный evidence-коммит).
- Типы: `chore(config)`, `docs(license)`, `chore(manifest)`, `test(xtask)`, `fix(plan-lint)` + `docs(plans)`, `fix(ledger)`, `docs(readme)`, `feat(doctor)`, `chore(repo)`, `chore(omo)`, `docs(plans)`, `docs`, `test(repo)`, `chore(just)`, `ci`, `test(release)`, `chore(evidence)`.
- `contract-change` + ADR для 2 (0025/0028), 18/19 (0027), 20 (0026).
- Artifact/plan-коммиты (7, 11, 12, 13) освобождены от C2; src/test-коммиты ≤3 файла и ≤200 строк по C2.
- Новые коммиты — с `user.name = Aleksei Molchanov` и `user.email = aleksei.v.molchanov@gmail.com`; история не переписывается.
- Никаких `--amend` после push, force-push, неаннотированных тегов, `push --all|--tags|--mirror`.

## Success criteria
0. `git status --porcelain` без tracked-модификаций; routing-тест `8 passed`; план публикации и V2-тики закоммичены (todo 23).
1. `./scripts/plan-lint.sh` → exit 0 **и** `ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном`, без `расхождений`; C8 входит в `total`.
2. `just check` → exit 0 без `test-status` в цепочке и с `history`; `just status` отдельно печатает RED-счётчики.
3. `cargo test -p xtask --test ledger_encoding` зелёный на чистом клоне; `cargo test -p stellwerk-registry --test repo_hygiene --test ci_contract` зелёные; clippy `-D warnings` чист.
4. `bash scripts/scan-history.sh` → exit 0 (не shallow), находок нет; в публичном дереве нет `ssh -p <digits> <host>` и приватных IPv4.
5. `bash scripts/verify-fresh-clone.sh` → `FRESH CLONE: OK` (tier-1) и либо tier-2 зелёный, либо exit 2 с названными предпосылками.
6. `git ls-files` без `.night/`, `.stellwerk/`, `.ablation-pi/`, `mutation-evidence/`, `PLACEHOLDER/`, `docs/diagnostics/`, `__pycache__`, `.omo/{evidence,run-continuation,drafts,archive}/`; `.sisyphus` канон закоммичен.
7. `LICENSE-MIT`, `LICENSE-APACHE`, `CHANGELOG.md`, `SECURITY.md`, `CONTRIBUTING.md`, `docs/quickstart.md`, `docs/release.md` непусты; `cargo metadata` даёт `publish == []` для 7 пакетов и `authors = Aleksei Molchanov <aleksei.v.molchanov@gmail.com>`.
8. `.github/workflows/ci.yml` без `|| true`/`continue-on-error`, с registry/core/store/history и `fetch-depth: 0`; `spec.yml` с именем `spec (expected RED)` без `continue-on-error`; `polyforge.yml` отсутствует.
9. `gh repo view tensorov/stellwerk --json visibility` → `PUBLIC`; `git ls-remote --heads --tags origin` → ровно `main` + `v0.0.1`; SECURITY содержит идентичность, состав истории и IP-attestation.
