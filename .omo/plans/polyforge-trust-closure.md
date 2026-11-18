# PolyForge Trust Closure — RIEGEL delta (вариант A)

**Slug:** `polyforge-trust-closure`
**Intent:** CLEAR
**Scope:** A — trust-critical only
**Status:** approved (A); canonicalization blocked by Prometheus file policy
**Baseline:** `/home/izyabretatel/gits/polyforge` @ `763eb69` (`master`)
**Canonical plan:** `.sisyphus/plans/RIEGEL/index.md`

## 1. Цель

Сделать так, чтобы доказательства PolyForge нельзя было принять за достаточные,
когда они такими не являются:

1. MCP-gate не проходит без явно названных required-состояний.
2. CLI не создаёт `Verified`/`Validated` без реального запуска allowlisted-инструмента.
3. Gate отклоняет `Verified`, полученный из грязного или чужого checkout.
4. CI self-gate проверяет текущий прогон, а не вечную запись `bootstrap`.
5. Внешний anchor можно проверить воспроизводимой командой, и его граница названа.
6. `polyforge-mcp` входит в coverage floor.

Это **delta** к RIEGEL, а не новый план с нуля. B-01 уже выполнен; B-02…B-13
остаются в силе. Новые пакеты: **B-14…B-18**.

## 2. Почему A, а не весь аудит

A закрывает именно trust-дыры. B (весь аудит) добавляет стратегию: переименование
в Riegel, `cargo-public-api`, `cargo-crap`, SBOM, freeze sandbox, distribution.
Эти темы уже разложены в B-07, B-09, B-10, B-11, B-12 и не должны смешиваться с
исправлением доказательств. C (только аудит-документ) не закрывает ни одну дыру.

## 3. Non-goals

- Не переименовывать PolyForge в Riegel (B-11/B-12).
- Не добавлять `cargo-public-api`, `cargo-crap`, `cargo-auditable` (B-07/B-09/B-10).
- Не включать live sandbox / freeze sandbox и не менять модель изоляции.
- Не вводить криптографическую identity оператора сверх существующего поля
  `validator` и отзыва (B-06).
- Не переписывать историю `.pf/ledger.jsonl`; только append.
- Не добавлять зависимости, не менять корневой `Cargo.toml`, не трогать
  `Cargo.lock` (он уже грязный до начала работы).
- Не «догонять ledger до N записей»: по RIEGEL index строки 41–42 это не цель.

## 4. Установленные факты (read-only, 2026-10-24)

| Факт | Где |
| --- | --- |
| HEAD `763eb69`; B-01 закрыт `7eda071` | `git log` |
| В ledger 5 записей | `.pf/ledger.jsonl` |
| `seq=1` — `ToolAttestation` от `cargo --version`, `claim_git_mismatch=true`, `git_dirty=true` | `.pf/ledger.jsonl` |
| CI self-gate: `task-id: bootstrap`, `required: verified` | `.github/workflows/ci.yml:34-38` |
| CI self-gate использует опубликованный `tensorov/polyforge-action@v1` | `.github/workflows/ci.yml:35` |
| Локальный action существует и умеет предпочитать workspace-бинарь | `.github/actions/polyforge/entrypoint.sh:43-52` |
| Coverage floor: только `polyforge-core` и `polyforge-toolrunner` | `.github/workflows/ci.yml:57` |
| `GateEvaluateParams.required` имеет `#[serde(default)]` и `Default` | `crates/polyforge-mcp/src/server.rs:195-201` |
| `gate_evaluate` передаёт пустой `required` в `evaluate_complete` | `crates/polyforge-mcp/src/server.rs:361-381` |
| CLI `append tool_attestation`/`validation` создаёт promoted-состояние без запуска инструмента | `crates/polyforge-cli/src/main.rs:152-226` |
| README first-run документирует этот bypass | `README.md:53,56` |
| `verify_and_append` уже умеет честный запуск инструмента | `crates/polyforge-toolrunner/src/verify.rs:153-221` |
| `claim_git_mismatch`/`git_dirty` записываются, но не блокируют gate | `crates/polyforge-toolrunner/src/verify.rs:145-147,199-213` |
| `gate_report` считает SHA-256, а не проверяет подпись | `crates/polyforge-mcp/src/server.rs:390-433` |
| Внешний anchor описан как «не trust boundary» | `SECURITY.md:112-113` |
| `polyforge-mcp` вне coverage floor | `.github/workflows/ci.yml:57`; B-08 |
| `polyforge-toolrunner` allowlist содержит `cargo test` | `crates/polyforge-toolrunner/src/runner.rs:131-135` |

## 5. Допущения

1. `polyforge-toolrunner::verify_and_append` — единственный честный путь к
   `Verified`: он запускает allowlisted-инструмент и сам пишет attestation.
2. `polyforge-cli verify` можно добавить без новой зависимости: CLI уже зависит
   от `polyforge-toolrunner`.
3. CI checkout чистый; `git diff HEAD` пуст; `claim_git_mismatch` и `git_dirty`
   ложны для честного self-gate.
4. `cargo test` есть в allowlist и принимает типизированные аргументы; если
   конкретный аргумент (`--workspace`) не разрешён, пакет возвращает BLOCKED, а
   не ослабляет allowlist.
5. Для B-17 `cosign` доступен в CI; локальная проверка bundle без `cosign`
   обязана падать закрыто, а не пропускать.
6. Пакеты выполняются строго последовательно в одном worktree. Пересечение
   `ci.yml` у B-17/B-18 допустимо только в этой последовательности; C9 здесь не
   применяется, потому что milestone один.

## 6. Очередь пакетов

```
B-01 (done) → B-13 → B-02 → B-03 → B-04 → B-05 → B-16 → B-14 → B-15a → B-15b1
→ B-15b2 → B-15b3 → B-17 → B-18 → B-06 → B-07 → B-08 → B-09 → B-10 → B-11 → B-12
```

B-16 идёт после B-05, потому что оба трогают `gate.rs`. B-18 идёт после B-15 и
B-17, потому что использует `pf verify` и вызывает `verify-anchor.sh`. B-17
создаёт скрипт до того, как B-18 добавит вызов в CI; файл `ci.yml` меняется один
раз за последовательность.

---

## B-14 — MCP gate требует явные required-состояния

**Goal (RED):** `gate_evaluate` без `required` или с `required: []` возвращает
ошибку, а не `passed: true`.

**files_allowed:**
- `crates/polyforge-mcp/src/server.rs`
- `crates/polyforge-mcp/tests/server_tools.rs`
- `crates/polyforge-mcp/tests/client_smoke.rs`

**test:** `cargo test -p polyforge-mcp`

**acceptance:**
- У `GateEvaluateParams.required` нет `#[serde(default)]`; отсутствие поля —
  ошибка десериализации, а не пустой Vec.
- `gate_evaluate` с `required: []` возвращает `invalid_params` с текстом
  `required must name at least one evidence state`.
- `required: ["Verified"]` на одном `ModelClaim` даёт `passed: false`,
  `missing: ["Verified"]`.
- После `evidence_verify` тот же вызов даёт `passed: true`.
- `client_smoke` больше не проверяет «пустой required проходит»; он передаёт
  `["Verified"]`.
- Никакого default-состояния (`Verified`/`Validated`) не подставляется.

**forbidden:**
- `required.unwrap_or(vec![Verified])`, `if required.is_empty() { required = ... }`.
- Принимать пустой `required` как «все состояния».
- Менять семантику `gate.rs` или `evidence_append`.
- Добавлять зависимость.

**depends_on:** `[]`
**max_diff_lines:** 120
**data_class:** private
**lane:** runtime

**Почему это отдельный пакет:** MCP-поверхность — отдельный вход; `gate.rs`
корректен, но MCP позволяет обойти требование, не называя его. Это ровно
аудит-пункт «empty required passes on a ModelClaim».

---

## B-15 — CLI: честный `verify`, без прямого promotion

**Goal (RED):** CLI не может создать `Verified`/`Validated` без запуска
allowlisted-инструмента; честный путь — новая команда `pf verify`.

**files_allowed:**
- `crates/polyforge-cli/src/main.rs`
- `README.md`
- `docs/integrations/langfuse.md`

**test:** `cargo test -p polyforge-cli`

**acceptance:**
- `pf append` принимает только `model_claim` и `discrepancy`. `tool_attestation`,
  `eval_attestation`, `validation` удалены из `parse_kind` и `cmd_append` и
  возвращают ошибку с указанием использовать `pf verify`.
- Новая команда:
  `pf verify <task_id> --claim <seq> --tool "<canonical-name>" [-- <args>...]`.
  Она вызывает `polyforge_toolrunner::lookup`, затем
  `verify_and_append(&mut ledger, task_id, claim_id, &tool, &args)`.
- На успехе печатает `verified entry <seq>`; на ошибке инструмента печатает
  `ToolFailed` и возвращает exit 1, не создавая `Verified`.
- Нельзя передать произвольный payload как доказательство: payload больше не
  принимается для promoted-состояний.
- README first-run: `polyforge-cli append model_claim ...` → `polyforge-cli verify
  ... --tool "cargo test" -- --offline` (пример), без `append tool_attestation` и
  `append validation`.
- `docs/integrations/langfuse.md` больше не показывает `append
  tool_attestation`; показывает `pf verify`.
- Unit-тесты в `main.rs`:
  - `append tool_attestation` → error, ledger не изменился;
  - `append validation` → error, ledger не изменился;
  - `verify` с `cargo test` и `--offline` → `Verified`, gate проходит;
  - `verify` с несуществующим claim → error, ledger не изменился.

**forbidden:**
- Оставлять любой путь, который создаёт promoted-состояние из строки payload.
- Добавлять `--force`, `--trust-me`, `PF_SKIP_TOOL`.
- Менять `evidence.rs`/`promote` или ослаблять state machine.
- Добавлять зависимость: CLI уже зависит от `polyforge-toolrunner`.
- Оставлять README-пример с удалённой командой.

**depends_on:** `[]`
**max_diff_lines:** 180
**data_class:** private
**lane:** runtime

**Почему это отдельный пакет:** MCP уже запрещает модели создавать promoted
состояния, но CLI — второй вход. Если у модели есть `bash` и approve, она
обходит MCP через CLI. Удаление прямого promotion и добавление `pf verify`
закрывает этот вход, не ослабляя операторский workflow.

---

## B-16 — gate отклоняет грязный и чужой checkout

**Goal (RED):** `Verified` с `claim_git_mismatch=true` или `git_dirty=true` не
удовлетворяет gate; отказ называет причину.

**files_allowed:**
- `crates/polyforge-core/src/gate.rs`
- `crates/polyforge-core/tests/commit_binding.rs`
- `docs/design/gate-binding.md`

**test:** `cargo test -p polyforge-core --test commit_binding`

**acceptance:**
- Gate при подсчёте `Verified`/`Validated` читает `payload.git_state`.
- Если `claim_git_mismatch == true`, возвращает `GateError::EvidenceRejected`
  (или именованный вариант) с `reason="claim_git_mismatch"`.
- Если `git_dirty == true`, возвращает `GateError::EvidenceRejected` с
  `reason="git_dirty"`.
- Отсутствие `git_state` у старой записи не трактуется как «чисто»; это
  `UnknownGitState` и отказ, если запись претендует на `Verified`.
- Позитивный контроль: чистая запись с совпадающим commit/diff проходит.
- `cargo --version` как единственный `ToolAttestation` не закрывает требование
  теста; это уже B-05, здесь проверяется только git-привязка.
- Старые записи не переписываются; отказ вычисляется при чтении.

**forbidden:**
- `git_state.unwrap_or(clean)`, `if mismatch { warn }`.
- Считать отсутствие `git_state` разрешением.
- Менять формат ledger или переписывать `.pf/ledger.jsonl`.
- Ослаблять B-04/B-05.

**depends_on:** `[05]`
**max_diff_lines:** 150
**data_class:** private
**lane:** runtime

**Почему это отдельный пакет:** B-04/B-05 уже говорят «результат другого
checkout» и «cargo --version», но не называют `git_dirty` и не требуют
fail-closed на `git_state`. Это добавление к их контракту, а не дубль.

---

## B-17 — внешний anchor проверяется воспроизводимой командой

**Goal (GREEN):** есть команда, которая проверяет подписанный bundle и
публикационную запись, и документация честно называет, что она доказывает.

**files_allowed:**
- `scripts/verify-anchor.sh`
- `docs/design/anchoring.md`
- `README.md`

**test:** `bash scripts/verify-anchor.sh --self-test`

**acceptance:**
- `scripts/verify-anchor.sh --check-local <ledger> <anchor>` проверяет
  `entry_count` и `head_hash`; malformed/missing anchor — exit 1.
- `scripts/verify-anchor.sh --check-bundle <bundle> <statement>` вызывает
  `cosign verify-blob` с ожидаемыми `--certificate-identity-regexp` и
  `--certificate-oidc-issuer`; если `cosign` нет — exit 1, не skip.
- `--self-test` создаёт временный ledger/anchor, проверяет happy path и три
  отказа: missing anchor, wrong count, wrong hash. Bundle-путь в self-test
  проверяет только fail-closed при отсутствии `cosign`, без сети.
- `docs/design/anchoring.md` говорит: подпись доказывает целостность текущего
  tail, но не непрерывность истории; `.pf/ledger.jsonl.anchor` — локальный
  sidecar, не внешний witness; кто и чем проверяет bundle.
- README содержит одну команду проверки для потребителя.
- B-18 добавляет вызов `--check-bundle` в CI `publish-anchor`; сам B-17 не
  трогает `ci.yml`, чтобы не пересекаться с B-18.

**forbidden:**
- Называть локальный sidecar внешним anchor.
- Пропускать проверку, если bundle отсутствует.
- Обещать, что подпись переписанного ledger доказывает непрерывность.
- Делать `gate.rs` сетевым/недетерминированным.
- Добавлять зависимость на cosign в Rust.

**depends_on:** `[02]`
**max_diff_lines:** 180
**data_class:** public
**lane:** infra

**Почему это отдельный пакет:** CI уже подписывает, но потребитель не имеет
команды проверки, а `SECURITY.md` прямо говорит «не trust boundary». Пакет
превращает anchor из артефакта в проверяемую границу, не ломая чистоту gate.

---

## B-18 — self-gate на текущий прогон, а не на `bootstrap`

**Goal (RED):** CI self-gate проверяет свежие доказательства текущего прогона;
`task-id: bootstrap` и опубликованный action для self-gate исчезают.

**files_allowed:**
- `scripts/self-gate.sh`
- `.github/workflows/ci.yml`
- `docs/design/self-gating.md`

**test:** `bash scripts/self-gate.sh --self-test && cargo test -p polyforge-cli`

**acceptance:**
- `scripts/self-gate.sh --ci`:
  1. создаёт временный ledger в `target/self-gate/ledger.jsonl`;
  2. `task_id="self-gate-${GITHUB_SHA:-$(git rev-parse HEAD)}"`;
  3. `commit_sha` — полный HEAD; `diff_hash` — `sha256` от `git diff HEAD`
     (тот же алгоритм, что `verify.rs`); в чистом CI это пустой diff;
  4. добавляет `ModelClaim` через `pf append model_claim`;
  5. запускает `pf verify --tool "cargo test" -- --workspace` и получает
     `Verified`;
  6. запускает `pf gate "$task_id" --required verified`;
  7. при любом отказе — exit 1, без fallback на `bootstrap`.
- `scripts/self-gate.sh --self-test` проверяет: missing claim → red; успешный
  fixture claim+attestation → green; `bootstrap` никогда не читается.
- CI:
  - шаг `Run tests` заменяется на `scripts/self-gate.sh --ci` (тесты
    выполняются ровно один раз, под toolrunner);
  - шаг `PolyForge self-gate` больше не использует
    `tensorov/polyforge-action@v1` и `task-id: bootstrap`;
  - `publish-anchor` после `cosign sign-blob` вызывает
    `scripts/verify-anchor.sh --check-bundle /tmp/anchor-pub.bundle
    /tmp/chain-statement.json`;
  - `parity` остаётся для потребителей; локальный action не обязан быть
    единственным путём self-gate.
- `docs/design/self-gating.md` объясняет: self-gate живёт в CI-артефакте
  `self-gate-ledger`, а `.pf/ledger.jsonl` — release-история; «догнать до N
  записей» не цель.
- В `.pf/ledger.jsonl` не добавляется фиктивная запись `bootstrap` для текущего
  коммита.

**forbidden:**
- `task-id: bootstrap`, `required: verified` без свежей задачи.
- `cargo --version` как доказательство теста.
- Автоматически коммитить/подделывать `.pf/ledger.jsonl`, чтобы «было N записей».
- Fallback на опубликованный action, если локальный gate упал.
- `|| true`, `continue-on-error` для self-gate.
- Трогать историю workflow, force-push, amend прошлых CI-коммитов.

**depends_on:** `[15a, 17]`
**max_diff_lines:** 200
**data_class:** private
**lane:** infra

**Почему это отдельный пакет:** B-04/B-05 исправляют семантику gate, но CI
всё ещё гейтит вечный `bootstrap`. Без B-18 trust-дыра остаётся на уровне
процесса: «gate умеет», но «gate не запускается на текущем коммите».

---

## 7. Глобальные проверки после каждого пакета

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Для B-17/B-18 дополнительно:

```bash
bash -n scripts/verify-anchor.sh
bash scripts/verify-anchor.sh --self-test
bash scripts/self-gate.sh --self-test
scripts/check-action-parity.sh
```

Для B-08 (существующий пакет) — отдельный owner-only prerequisite:
`cargo llvm-cov --version`; без него B-08 BLOCKED, не «зелёный».

## 8. Evidence и протокол

- Каждый пакет: RED → показать падающий тест; GREEN → показать проходящий;
  PROOF → `evidence_append(ModelClaim)` + `evidence_verify(cmd=<точная команда
  теста>)`.
- Для пакетов, выполненных в worktree, действует вариант B из AGENTS:
  merge --no-ff → `evidence_verify` на main → `gate_evaluate` → отдельный
  коммит `chore(evidence): attest <packet> after merge`.
- Не аттестовать заглушку на main, пока реализация только в worktree.
- Не объявлять гейт пройденным без `gate_evaluate`.
- Тест-сьютку гонять на main до merge; база worktree отстаёт.

## 9. Риски и остановки

| Риск | Реакция |
| --- | --- |
| `cargo test --workspace` не проходит через allowlist/typed args | BLOCKED; не ослаблять allowlist; вернуть FINDING |
| `cosign` недоступен в CI | B-17/B-18 BLOCKED; не пропускать проверку |
| `pf verify` требует `Tool` с полями, недоступными из CLI | использовать `lookup(name)` + `verify_and_append`; не строить `Tool` вручную |
| B-16 конфликтует с B-05 по `gate.rs` | строго последовательно; B-05 сначала, B-16 rebase |
| B-17/B-18 конфликтуют по `ci.yml` | B-18 сначала, B-17 после; один diff на файл |
| Coverage floor для `polyforge-mcp` падает | FINDING с фактическим coverage; порог не понижать (B-08) |
| `Cargo.lock` уже грязный | не включать в пакеты; не коммитить без необходимости |

## 10. Approval

Нужен явный выбор:

- **A** — выполнить B-14…B-18 в указанном порядке (этот план).
- **B** — добавить B-07/B-09/B-10/B-11/B-12 и стратегические решения.
- **C** — только доработать аудит-документ, без плана и кода.

После «A» / «окей»:
1. добавить packet-14…packet-18 в `.sisyphus/plans/RIEGEL`;
2. обновить `index.md` (порядок, ссылки, статус B-01);
3. передать план воркеру отдельной сессией.

Код не трогается до этого шага.

## 11. Канонизация (выполняет не Prometheus)

Prometheus не имеет права писать в `.sisyphus`: попытка создать
`.sisyphus/plans/RIEGEL/packet-14.md` отклонена `prometheus-md-only`. Поэтому
канонизация — отдельный шаг воркера/владельца, не реализация продукта.

Нужно:

1. Скопировать готовые `packet-14.md` … `packet-18.md` и `index.md` из
   `/home/izyabretatel/gits/stellwerk/.omo/plans/RIEGEL/` в
   `/home/izyabretatel/gits/polyforge/.sisyphus/plans/RIEGEL/`.
2. Сверить diff `index.md`: порядок, ссылки на B-14…B-18, пометка B-17 → B-18.
3. Проверить, что `packet-14…18` не пересекаются по файлам иначе, чем заявлено:
   B-16/B-04/B-05 — `gate.rs`; B-15/B-17 — `README.md`; B-18 — `ci.yml`.
   Все пересечения последовательные, C9 не применяется (один milestone).
4. После канонизации передать план воркеру отдельной сессией, не запускать
   реализацию из этой сессии.

## TODOs

- [x] 1. B-16: в `crates/polyforge-core/src/gate.rs` gate отвергает `Verified`/`Validated` при `claim_git_mismatch=true`, `git_dirty=true` или отсутствующем `git_state`; добавить `crates/polyforge-core/tests/commit_binding.rs` и обновить `docs/design/gate-binding.md`; `cargo test -p polyforge-core --test commit_binding` и `cargo test -p polyforge-core`.
- [x] 2. B-14: в `crates/polyforge-mcp/src/server.rs` убрать serde-default у `GateEvaluateParams.required` и отвергать пустой `required`; обновить `server_tools.rs` и `client_smoke.rs`; `cargo test -p polyforge-mcp`.
- [x] 3. B-15a: в `crates/polyforge-cli/src/main.rs` добавить `pf verify <task> --claim <seq> --tool <name> [-- args]` через `lookup` + `verify_and_append`; обновить `README.md`, `docs/integrations/langfuse.md`; `cargo test -p polyforge-cli`.
  - Примечание: B-15a закрыт по своим unit-тестам (`--bin polyforge-cli` 32/32, clippy/fmt clean) и ручному first-run в чистом git checkout; полный `cargo test -p polyforge-cli` ещё красный на 10 legacy-тестах, их миграция — B-15b1.
- [x] 4. B-15b1: перевести `tests/army_flow.rs`, `tests/gate.rs`, `tests/mutation_guards.rs` с `append tool_attestation`/`eval_attestation` на `pf verify`; `cargo test -p polyforge-cli --test army_flow --test gate --test mutation_guards`.
  - Примечание: закрыт; три файла, 22 теста (6+7+9) проходят, fmt/clippy clean. C2 tests 237 ≤ 250 (army 93, gate 82, mutation 62); helper-дублирование сжато без удаления проверок.
- [x] 5. B-15b2: перевести `tests/summary.rs` на `pf verify` и core-фикстуру `Validated`; `cargo test -p polyforge-cli --test summary`.
  - Примечание: закрыт; 5/5 тестов, fmt/clippy clean. C2 tests 120 ≤ 250; `seed_verified` через `pf verify`, `seed_validated` через `Ledger` + `promote`, запрещённых `append tool_attestation`/`append validation` нет.
- [x] 6. B-15b3: удалить `tool_attestation`/`eval_attestation`/`validation` из `parse_kind`/`cmd_append`/usage; `cargo test -p polyforge-cli`.
  - Примечание: закрыт; `cargo test -p polyforge-cli` 76/76 (33 unit + 43 integration), fmt/clippy clean. `parse_kind` принимает только `model_claim`/`discrepancy`; удалённые kind'ы отвергаются до `Ledger::new` с указанием `pf verify`; добавлен тест, что ledger не меняется. Module docs исправлены: `tool_attestation` — из `pf verify`, `validation` — не append-path.
- [x] 7. B-17: создать `scripts/verify-anchor.sh` с `--check-local`, `--check-bundle`, `--self-test`; создать `docs/design/anchoring.md` и обновить README; `bash scripts/verify-anchor.sh --self-test`.
- [x] 8. B-18: создать `scripts/self-gate.sh` с временным ledger и `pf verify --tool "cargo test" -- --workspace`; заменить `bootstrap` в `.github/workflows/ci.yml`; добавить `docs/design/self-gating.md`; `bash scripts/self-gate.sh --self-test && cargo test -p polyforge-cli`.
  - Примечание: закрыт; `bash scripts/self-gate.sh --self-test` PASS, temp cleanup empty on success и failure-path (`PF_BIN=/bin/false` rc=1, find empty); `cargo test -p polyforge-cli` 76/76; `bash -n`/YAML clean. C2 added 186 (ci.yml 11 + self-gate.sh 129 + self-gating.md 46) ≤ 200, 3 files.
- [x] 9. B-18 CI wiring: в `publish-anchor` вызвать `scripts/verify-anchor.sh --check-bundle`; проверить отсутствие `task-id: bootstrap`; `bash -n scripts/self-gate.sh`.
  - Примечание: закрыт; self-gate step заменяет `tensorov/polyforge-action@v1`/`task-id: bootstrap`, добавлен `Upload self-gate ledger` (`if: always()`, artifact `self-gate-ledger`, path `target/self-gate/`); `publish-anchor` вызывает `--check-bundle` после `cosign sign-blob`; bootstrap grep empty, YAML OK, `bash -n` rc 0, `git diff --check` clean.

## Final Verification Wave

- [x] F1. Plan compliance audit: сверить B-14…B-18 и B-02/B-05 (A2) с каноном RIEGEL; подтвердить отсутствие B-03/B-06…B-13; B-04 допускается только как prerequisite B-05 (`packet-05.md:10`), не как самостоятельный scope.
  - APPROVE 2026-10-25: F1 rerun accepted after fixing dependency direction (`825f85b`). Implemented packets have 0 dependencies on non-`выполнен`; RIEGEL plan-lint 23/0 C1/C3/C4; C8 clean; C9 global only. B-04 allowed as B-05 prerequisite. Evidence/gate tail `b1bbfad2…`.
  - Обновление 2026-10-25: B-02/B-05 реализованы (коммиты 13c6134, 767ff6c); B-15 canon выровнен на `cargo test --offline` (packet-15a/b1/b2); `packet-15.md` удалён из канона как superseded; статусы B-02/B-04/B-05/B-14/B-15a/b1/b2/b3/B-16/B-16a/B-16b/B-17/B-18 = `выполнен`; RIEGEL plan-lint: 23 пакета, 0 нарушений C1/C3/C4; evidence/gate PASSED (tail `b1bbfad2…`). F1 остаётся `[~]` до явного APPROVE. B-04 не самостоятельный scope: `packet-05.md:10` объявляет `depends_on: [04]`, и B-04 уже стоит в последовательности плана (строка 86). C8/C9 при запуске из stellwerk — артефакт кросс-репо зеркала, не канон.
  - Исправление dependency inversion 2026-10-25: C1-связи были направлены от реализованных пакетов к отсутствующим (B-15a→B-03, B-18→B-08), что и поймал F1. Направление перевёрнуто: B-15a больше не зависит от B-03; B-03 зависит от `[02, 15a, 17]`. B-18 больше не зависит от B-08; B-08 зависит от `[07, 18]`. Проверка: у всех `выполнен`-пакетов 0 зависимостей на не-`выполнен`; RIEGEL plan-lint 23 пакета, 0 нарушений C1/C3/C4.
- [x] F2. Code quality review: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, отсутствие `#[allow]`, `#[ignore]`, новых зависимостей.
  - Примечание: APPROVE после B-16a. F2-блокеры закрыты: `ledger_flow` exit 0 (`passed: true`), TUI `Validated` наследует `git_state`, gate принимает. Проверены: fmt 0, clippy 0, `commit_binding` 7/7, core 66+7, TUI 54+4+1+26, MCP 35, example 0. B-16a не добавляет `#[allow]`/`#[ignore]`; baseline `too_many_arguments`/`large_enum_variant` не менялись. Новых зависимостей, `.pf` и `gate.rs` изменений нет. `Cargo.lock` содержит только pre-existing 0.4.0→0.4.1 bump.
- [x] F3. Real manual QA: реальные вызовы MCP gate, CLI verify, gate rejection, `verify-anchor.sh --self-test`, `self-gate.sh --self-test`.
  - Примечание: APPROVE после корректировки критерия. `packet-16.md` требует проверку `payload.git_state` на момент аттестации, а не повторное чтение текущего checkout; прежний тест «dirty after verify» был вне контракта. Проверены: `cargo test -p polyforge-core --test commit_binding` 7/7, `cargo test -p polyforge-core` 65+7, `cargo test -p polyforge-mcp` 35, `cargo test -p polyforge-cli` 76, `verify-anchor.sh --self-test` PASS, `self-gate.sh --self-test` PASS; точные причины `git_dirty`/`claim_git_mismatch`/`UnknownGitState`; MCP/CLI пути зелёные.
- [x] F4. Scope fidelity: проверить diff по пакетам, ≤3 файла и ≤200 строк, нет изменений `.pf`, `Cargo.toml`, `Cargo.lock`.
  - Примечание: APPROVE после split B-16b. B-16a = 3 файла (144/17), `lib.rs` чист; B-16b = 2 файла (`app.rs` +20/−3, `snapshots.rs` +31/−5; C2 src 20 / tests 31; raw 59 ≤ 60). B-14/15a/15b1/15b2/15b3/16/17/18 в C2-бюджете; shared `main.rs` B-15a 167 + B-15b3 32 = 199; shared `README.md` B-15a 11 + B-17 8 = 19. `.pf` нет, `Cargo.toml` нет, `Cargo.lock` — только pre-existing 0.4.0→0.4.1 bump, пакетами не вызван.


