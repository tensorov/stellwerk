---
id: c01
milestone: V1
status: GREEN
depends_on: [03, a02]
files_allowed: [crates/stellwerk-cli/src/build_runner.rs, crates/stellwerk-task/src/m5/sandbox_bwrap.rs, crates/stellwerk-cli/tests/build_runner.rs]
overlap_waiver: C9 — N2/00c, N2/00h3b, N2/00h3c, N2/00h3d, N2/00j1, N2/00j6, последовательное исполнение
acceptance: cargo test -p stellwerk-cli --test build_runner
---

# c01 — Кандидат коммита из workspace по digest

## Цель
После успешной сессии кандидат собирается из файлов workspace с их digest, потому что кандидат из истории репозитория содержит правки, которых сессия не делала.

## Обоснование (измерено)
- `crates/stellwerk-cli/src/build_runner.rs:82` — здесь начинается работа с workspace, откуда кандидат обязан брать файлы.
- `crates/stellwerk-task/src/m5/sandbox_bwrap.rs:184` — здесь заданы границы workspace, и `.git` не должен попадать внутрь кандидата.

## Шаг 1 — падающий тест
`crates/stellwerk-cli/tests/build_runner.rs`, тест `candidate_contains_only_changed_workspace_files`: утверждает, что кандидат содержит ровно изменённые файлы и их digest, а файл вне workspace в кандидат не попадает; до сборки из workspace кандидат берётся из истории и содержит лишние файлы, поэтому тест падает на составе кандидата.

## Приёмка
cargo test -p stellwerk-cli --test build_runner
Ожидается PASS.

## Запрещено
- нельзя собирать кандидат из истории репозитория.
- запрещено создавать коммит в пользовательском репозитории.
- нельзя пропускать `.git` внутрь workspace.
