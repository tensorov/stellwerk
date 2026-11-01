---
id: v03
milestone: V1
status: GREEN
depends_on: [o01, v02]
files_allowed: [crates/stellwerk-cli/tests/vertical_v3.rs]
acceptance: cargo test -p stellwerk-cli --test vertical_v3
---

# v03 — Вертикальное доказательство bindings evidence

## Цель
Цепочка несёт `recipe_digest`, `policy_digest` и `commit_sha`, и подмена любого digest ломает проверку, потому что привязка, которую нельзя сломать подменой, ничего не привязывает.

## Обоснование (измерено)
- `crates/stellwerk-task/tests/evidence.rs:270` — цепочка перечисляет `ModelClaim`, `ToolAttestation` и `Validation`, и bindings добавляются к этим звеньям.
- `crates/stellwerk-task/tests/evidence.rs:194` — хвост цепочки уже обязан быть настоящим digest, а не заглушкой, поэтому тест подмены опирается на существующую проверку.

## Шаг 1 — падающий тест
`crates/stellwerk-cli/tests/vertical_v3.rs`, тест `tampered_digest_breaks_binding`: требует, чтобы подмена `recipe_digest`, `policy_digest` или `commit_sha` ломала проверку цепочки; до появления bindings подменять нечего и проверка проходит, поэтому тест падает на отсутствии отказа.

## Приёмка
cargo test -p stellwerk-cli --test vertical_v3
Ожидается PASS.

## Запрещено
- нельзя объявлять привязку доказанной без аттестации.
- запрещено подменять проверку локальной при отказе схемы PolyForge.
- нельзя принимать цепочку без `recipe_digest`, `policy_digest` и `commit_sha`.
