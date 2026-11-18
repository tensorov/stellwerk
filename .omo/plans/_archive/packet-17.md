# packet-17 — внешний anchor проверяется воспроизводимой командой

plan_id: R1
status: предложен
goal: GREEN — есть команда, которая проверяет подписанный bundle и публикационную запись, и документация честно называет, что она доказывает.
files_allowed: [scripts/verify-anchor.sh, docs/design/anchoring.md, README.md]
max_diff_lines: 180
test: bash scripts/verify-anchor.sh --self-test
acceptance: >
  `scripts/verify-anchor.sh --check-local <ledger> <anchor>` проверяет
  `entry_count` и `head_hash`; malformed/missing anchor — exit 1.
  `scripts/verify-anchor.sh --check-bundle <bundle> <statement>` вызывает
  `cosign verify-blob` с ожидаемыми `--certificate-identity-regexp` и
  `--certificate-oidc-issuer`; если `cosign` нет — exit 1, не skip.
  `--self-test` создаёт временный ledger/anchor, проверяет happy path и три
  отказа: missing anchor, wrong count, wrong hash. Bundle-путь в self-test
  проверяет только fail-closed при отсутствии `cosign`, без сети.
  `docs/design/anchoring.md` говорит: подпись доказывает целостность текущего
  tail, но не непрерывность истории; `.pf/ledger.jsonl.anchor` — локальный
  sidecar, не внешний witness; кто и чем проверяет bundle. README содержит одну
  команду проверки для потребителя. B-18 добавляет вызов `--check-bundle` в CI
  `publish-anchor`; сам B-17 не трогает `ci.yml`, чтобы не пересекаться с B-18.
forbidden: [называть локальный sidecar внешним anchor, пропускать проверку если bundle отсутствует, обещать что подпись переписанного ledger доказывает непрерывность, делать `gate.rs` сетевым/недетерминированным, добавлять зависимость на cosign в Rust]
depends_on: [02]
mutation_threshold: 0
data_class: public
lane: infra

## Контекст

CI уже подписывает, но потребитель не имеет команды проверки, а `SECURITY.md` прямо
говорит «не trust boundary». Пакет превращает anchor из артефакта в проверяемую
границу, не ломая чистоту gate.

## Что сделать

1. `scripts/verify-anchor.sh` с режимами `--check-local`, `--check-bundle`,
   `--self-test`.
2. `--check-local` проверяет `entry_count` и `head_hash`; missing/malformed anchor —
   exit 1.
3. `--check-bundle` вызывает `cosign verify-blob` с ожидаемыми identity/issuer; без
   `cosign` — exit 1, не skip.
4. `--self-test` проверяет happy path и три отказа: missing anchor, wrong count,
   wrong hash; bundle-путь только fail-closed при отсутствии `cosign`, без сети.
5. Документация: подпись доказывает целостность tail, но не непрерывность истории;
   локальный sidecar — не внешний witness.
6. README — одна команда проверки для потребителя.

## Проверка

`bash scripts/verify-anchor.sh --self-test` зелёный. Отдельно проверить, что при
отсутствии `cosign` `--check-bundle` возвращает 1, а не 0. `gate.rs` не меняется.
