---
id: N2/00j1
milestone: N2
title: Build больше не предлагает bash, но гейт помнит R5
depends_on: [00h3c, 00h3d]
allowed_changes:
  - crates/stellwerk-cli/src/build_runner.rs
  - crates/stellwerk-cli/tests/build_runner.rs
---

# 00j1 — Build больше не предлагает bash, но гейт помнит R5

## Зачем

`BUILD_TOOLS` перечисляет `bash` в `--tools`, хотя Build-агенту оболочка не нужна и
политика R5 всё равно её запрещает. Дефект двойной: модель получает инструмент,
которого заведомо не получит в исполнении, а end-to-end-тест
`the_shell_is_denied_before_it_touches_anything` проверяет путь, недостижимый by
design — Pi не вызывает необъявленный инструмент. Нужно развести два разных списка:
что предлагается модели (`BUILD_OFFERED`, без bash) и что гейт держит под запретом
(`BUILD_HELD`, с bash). Если просто убрать bash из гейта, R5 станет неотличим от
«инструмента нет», а это ровно тот дефект, когда неизвестное превращается в
разрешение.

## Что делается

1. `pub const BUILD_OFFERED: &[&str] = &["read","edit","write"]` и
   `pub const BUILD_HELD: &[&str] = &["read","bash","edit","write"]`.
2. `pub fn build_args(extension: &Path) -> Vec<String>` использует
   `pi_transport_args(extension, BUILD_OFFERED)`; функция публичная, потому что тест
   живёт в отдельном крейте `tests/`.
3. `run_build` вызывает `build_args` (вместо литерала `:383`) и
   `ToolSet::listed(BUILD_HELD)` (вместо литерала `:362`).
4. Заменить end-to-end `the_shell_is_denied_before_it_touches_anything`
   (`tests/build_runner.rs:243-281`) на два теста: `the_build_does_not_offer_bash`
   (`BUILD_OFFERED` и `build_args` без bash) и `bash_stays_a_gate_level_deny`
   (`BUILD_HELD` содержит bash; `session(BUILD_HELD).adjudicate("bash")` +
   `BuildPolicy::decide_call` даёт R5-решение).
5. Старый тест удаляется: Pi не вызывает необъявленный инструмент, end-to-end R5
   через Build-сессию недостижим by design; это покрывается legacy/чужим набором в
   00j3 и `tests/gate_channel.rs`.
6. Не трогать `permission.rs`, `gate.rs`, командный фильтр.

## RED

- `the_build_does_not_offer_bash` — `BUILD_OFFERED == ["read","edit","write"]`,
  `build_args` содержит `--tools` + `read,edit,write`, без `bash`.
- `bash_stays_a_gate_level_deny` — `BUILD_HELD` содержит bash, решение R5.
- Удаляется `the_shell_is_denied_before_it_touches_anything`.
- Остальные 15 тестов `tests/build_runner.rs` остаются зелёными; итог
  `16 passed` (15 − 1 + 2).

## Приёмка

```
cargo test -p stellwerk-cli --test build_runner
cargo clippy -p stellwerk-cli --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-cli -- --check
```

Ожидание: `16 passed` (15 − 1 + 2); clippy/fmt чисто.

## Не делается

Не меняются `permission.rs`, `gate.rs` и командный фильтр. R5 остаётся R5.
Никаких новых зависимостей, `#[allow]`/`#[ignore]`.

## Бюджет

Не больше 3 изменённых файлов. C2: src ≤ 200 логически добавленных строк,
tests ≤ 250 добавленных строк. Новые зависимости не добавляются. `#[allow(...)]`
и `#[ignore]` запрещены.
