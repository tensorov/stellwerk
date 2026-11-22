# N1-07 — evidence: записи без претензии на власть

**label:** contract-change · **mutation_gate:** yes (≥75%)
**depends_on:** [N1-06] · **ADR:** решения 5 зафиксированы владельцем

## Цель

`evaluate()` (`evidence.rs:77`) требует `ToolAttestation` **и** `Validation`, но
конструктор есть только у `model_claim` (`evidence.rs:61`) — гейт физически не
может собрать цепочку.

**Решение 5 меняет смысл, а не только форму:**

- конструкторы структур **не являются authority**;
- `ToolAttestation` появляется **только после фактической релевантной проверки** —
  не «функция вернула Ok», а «проверка действительно выполнена и результат
  релевантен»;
- `Validation` — **отдельное явное решение человека**;
- **до `Validation` результат ждёт человека: `Done` и `gate PASS` не объявляются**;
- тестовая `Validation` допустима **только как явно помеченная фикстура**.

## production_call

- `evidence::tool_attestation` ← `orchestrator::execute` (после релевантной проверки)
- `evidence::validation` ← **только** путь решения человека, не оркестратор

## files_allowed

- `crates/stellwerk-task/src/evidence.rs`
- `crates/stellwerk-task/tests/evidence.rs`

## Что делать

1. `pub fn tool_attestation(task_id, id, command, result_hash, ts) -> EvidenceEntry`.
   Команда **принимается**: инструмент запускался, и это доказывает.
2. `pub fn validation(task_id, id, subject, result_hash, ts) -> EvidenceEntry`.
   **Проверено:** у `EvidenceEntry` (`contract.rs:449`) ровно 6 полей —
   `id, task_id, kind, command, result_hash, ts`. Поля «кто проверял» нет,
   поэтому `subject` кладётся в `command`. Новых полей не вводить.
3. **`evaluate` не переписывать.** Он читает `RunOutcome.verdict` как данное
   (F-2026-09-29-20); пересчёт внутри гейта выбрал бы сторону в открытом споре.
4. **Тип статуса.** Добавить явное состояние «ждёт человека» — результат без
   `Validation` **не** `Done`. Существующие состояния не сливать: отсутствие
   ответа и сознательное решение неотличимы друг от друга, а закон 6 это запрещает.
5. `model_claim.command` остаётся пустой строкой, задать его нельзя.

## Почему `mutation_gate: yes`

Мутант, выпускающий `ToolAttestation` без проверки или подставляющий
`ModelClaim` вместо него, ломает смысл цепочки тихо — такой мутант и поймать
обязательно.

## Запрещено трогать

`crates/stellwerk-core/src/contract.rs` — поля уже есть, контракт не меняется.
`runner.rs`, `crates/stellwerk-gateway/**`, store, `justfile`, `.github/`,
`scripts/plan-lint.py`, `xtask/`.

## Приёмка

```sh
cargo build --workspace --all-targets; echo "BUILD_EXIT=$?"
cargo test -p stellwerk-task --test evidence; echo "TEST_EXIT=$?"
```

**Оба 0.** Обязательные тесты:

- цепочка `tool_attestation` + **человеческая** `validation` → состояние
  «принято человеком»;
- цепочка **без** `Validation` → «ждёт человека», и **не** `Done`;
- тестовая `Validation` помечена как фикстура и не выдаётся за решение человека;
- `ToolAttestation` нельзя получить без зафиксированной фактической проверки.

Проверка, что конструкторы — единственный путь:

```sh
grep -rn "EvidenceEntry {" crates/*/src/ | grep -v "pub struct"
```

Пусто, кроме трёх конструкторов.