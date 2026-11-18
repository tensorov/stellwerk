---
id: c02
milestone: V1
status: GREEN
depends_on: [c01]
files_allowed: [crates/stellwerk-cli/src/build_runner.rs, crates/stellwerk-cli/tests/candidate.rs]
overlap_waiver: C9 — N2/00h3b, N2/00h3c, N2/00h3d, N2/00j1, последовательное исполнение
acceptance: cargo test -p stellwerk-cli --test candidate
---

# c02 — Отказ коммита при грязном дереве

## Цель
Перед принятием кандидата проверяется чистота workspace относительно базы и совпадение digest всех файлов, иначе возвращается `DirtyWorkspace` с перечнем файлов, потому что принятый грязный кандидат смешивает чужие правки с авторизованными.

## Обоснование (измерено)
- `crates/stellwerk-cli/src/build_runner.rs:364` — рабочее дерево получается копированием workspace, поэтому проверка чистоты обязана смотреть туда же, а не в историю репозитория.
- `crates/stellwerk-cli/src/build_runner.rs:82` — копирование workspace — единственный источник файлов кандидата, и лишняя правка в нём должна быть видна.

## Шаг 1 — падающий тест
`crates/stellwerk-cli/tests/candidate.rs`, тест `dirty_workspace_is_refused_with_file_list`: утверждает, что лишняя правка в workspace даёт `DirtyWorkspace` с именами файлов, а чистое дерево проходит; до проверки грязный кандидат принимается молча, и тест падает на отсутствии отказа.

## Приёмка
cargo test -p stellwerk-cli --test candidate
Ожидается PASS.

## Запрещено
- нельзя чинить дерево автоматически вместо отказа.
- запрещено игнорировать неотслеживаемые файлы при проверке чистоты.
- нельзя принимать кандидат при несовпадении digest.
