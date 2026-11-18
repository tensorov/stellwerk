# packet-16 — gate отклоняет грязный и чужой checkout

plan_id: R1
status: предложен
goal: RED — `Verified` с `claim_git_mismatch=true` или `git_dirty=true` не удовлетворяет gate; отказ называет причину.
files_allowed: [crates/polyforge-core/src/gate.rs, crates/polyforge-core/tests/commit_binding.rs, docs/design/gate-binding.md]
max_diff_lines: 150
test: cargo test -p polyforge-core --test commit_binding
acceptance: >
  Gate при подсчёте `Verified`/`Validated` читает `payload.git_state`. Если
  `claim_git_mismatch == true`, возвращает `GateError::EvidenceRejected` (или
  именованный вариант) с `reason="claim_git_mismatch"`. Если `git_dirty == true`,
  возвращает `GateError::EvidenceRejected` с `reason="git_dirty"`. Отсутствие
  `git_state` у старой записи не трактуется как «чисто»; это `UnknownGitState`
  и отказ, если запись претендует на `Verified`. Позитивный контроль: чистая
  запись с совпадающим commit/diff проходит. `cargo --version` как единственный
  `ToolAttestation` не закрывает требование теста; это уже B-05, здесь
  проверяется только git-привязка. Старые записи не переписываются; отказ
  вычисляется при чтении.
forbidden: [`git_state.unwrap_or(clean)`, `if mismatch { warn }`, считать отсутствие `git_state` разрешением, менять формат ledger или переписывать `.pf/ledger.jsonl`, ослаблять B-04/B-05]
depends_on: [05]
mutation_threshold: 80
data_class: private
lane: runtime

## Контекст

B-04/B-05 уже говорят «результат другого checkout» и «cargo --version», но не называют
`git_dirty` и не требуют fail-closed на `git_state`. Это добавление к их контракту, а
не дубль. Ошибка при чтении старых записей — штатная: история не переписывается.

## Что сделать

1. В `gate.rs` при подсчёте `Verified`/`Validated` читать `payload.git_state`.
2. Отвергать `claim_git_mismatch=true` с названной причиной.
3. Отвергать `git_dirty=true` с названной причиной.
4. Отвергать отсутствие/неизвестный `git_state` у записи, претендующей на `Verified`.
5. Добавить позитивный контроль: чистая запись с совпадающим commit/diff проходит.
6. Обновить `docs/design/gate-binding.md`, не меняя B-04/B-05.

## Проверка

RED: `cargo test -p polyforge-core --test commit_binding` падает на каждом из трёх
отказов. GREEN: тот же тест проходит, позитивный контроль зелёный. Набор негативных
тестов должен быть параметрическим, иначе он молча перестанет проверяться.
