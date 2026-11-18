# packet-18 — self-gate на текущий прогон, а не на `bootstrap`

plan_id: R1
status: предложен
goal: RED — CI self-gate проверяет свежие доказательства текущего прогона; `task-id: bootstrap` и опубликованный action для self-gate исчезают.
files_allowed: [scripts/self-gate.sh, .github/workflows/ci.yml, docs/design/self-gating.md]
max_diff_lines: 200
test: bash scripts/self-gate.sh --self-test && cargo test -p polyforge-cli
acceptance: >
  `scripts/self-gate.sh --ci`: 1) создаёт временный ledger в
  `target/self-gate/ledger.jsonl`; 2) `task_id="self-gate-${GITHUB_SHA:-$(git
  rev-parse HEAD)}"`; 3) `commit_sha` — полный HEAD; `diff_hash` — `sha256` от
  `git diff HEAD` (тот же алгоритм, что `verify.rs`); в чистом CI это пустой
  diff; 4) добавляет `ModelClaim` через `pf append model_claim`; 5) запускает
  `pf verify --tool "cargo test" -- --workspace` и получает `Verified`;
  6) запускает `pf gate "$task_id" --required verified`; 7) при любом отказе —
  exit 1, без fallback на `bootstrap`. `scripts/self-gate.sh --self-test`
  проверяет: missing claim → red; успешный fixture claim+attestation → green;
  `bootstrap` никогда не читается. CI: шаг `Run tests` заменяется на
  `scripts/self-gate.sh --ci` (тесты выполняются ровно один раз, под
  toolrunner); шаг `PolyForge self-gate` больше не использует
  `tensorov/polyforge-action@v1` и `task-id: bootstrap`; `publish-anchor` после
  `cosign sign-blob` вызывает `scripts/verify-anchor.sh --check-bundle
  /tmp/anchor-pub.bundle /tmp/chain-statement.json`; `parity` остаётся для
  потребителей; локальный action не обязан быть единственным путём self-gate.
  `docs/design/self-gating.md` объясняет: self-gate живёт в CI-артефакте
  `self-gate-ledger`, а `.pf/ledger.jsonl` — release-история; «догнать до N
  записей» не цель. В `.pf/ledger.jsonl` не добавляется фиктивная запись
  `bootstrap` для текущего коммита.
forbidden: [`task-id: bootstrap`/`required: verified` без свежей задачи, `cargo --version` как доказательство теста, автоматически коммитить/подделывать `.pf/ledger.jsonl` чтобы «было N записей», fallback на опубликованный action если локальный gate упал, `|| true`/`continue-on-error` для self-gate, трогать историю workflow/force-push/amend прошлых CI-коммитов]
depends_on: [15, 17]
mutation_threshold: 0
data_class: private
lane: infra

## Контекст

B-04/B-05 исправляют семантику gate, но CI всё ещё гейтит вечный `bootstrap`. Без B-18
trust-дыра остаётся на уровне процесса: «gate умеет», но «gate не запускается на
текущем коммите». B-17 идёт до B-18, потому что B-18 вызывает `verify-anchor.sh`.

## Что сделать

1. `scripts/self-gate.sh --ci`: временный ledger, task id от `GITHUB_SHA`/HEAD,
   `ModelClaim`, `pf verify --tool "cargo test" -- --workspace`, `pf gate`.
2. Любой отказ — exit 1; никакого fallback на `bootstrap` или опубликованный action.
3. `scripts/self-gate.sh --self-test`: missing claim → red; fixture claim+attestation
   → green; `bootstrap` не читается.
4. В CI заменить `Run tests` на `scripts/self-gate.sh --ci`; убрать
   `tensorov/polyforge-action@v1` и `task-id: bootstrap` из self-gate.
5. В `publish-anchor` после `cosign sign-blob` вызвать
   `scripts/verify-anchor.sh --check-bundle`.
6. `docs/design/self-gating.md`: self-gate — CI-артефакт, `.pf` — release-история;
   «догнать до N записей» не цель.

## Проверка

`bash scripts/self-gate.sh --self-test && cargo test -p polyforge-cli`. Отдельно
проверить, что `grep -R "task-id: bootstrap" .github/workflows/ci.yml` пуст, а
`self-gate` падает при отсутствии свежего `ModelClaim`.
