# packet-03
plan_id: M1
goal: Реализовать `NetworkPolicy::allows` — правило сетевой изоляции, на котором держится offline-first.
files_allowed: [crates/stellwerk-core/src/contract.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-core --test workspace_policy
acceptance: `cargo test -p stellwerk-core --test workspace_policy` → 8 passed; 0 failed.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [01]
data_class: public
lane: m1/core-net

## Контекст

- Крейт `stellwerk-core`, единственный файл контракта — `src/contract.rs`.
- Контракт заморожен: правится ТОЛЬКО предикат, названный в этом пакете. Любая другая
  правка `contract.rs` — нарушение packet'а.
- Предикаты объявлены с телом `async { todo!(...) }` и строкой `let _ = (…)`, чтобы
  `clippy -D warnings` оставался зелёным. При реализации `let _ = …` УДАЛЯЕТСЯ.
- RED-тесты написаны ночной сессией (фаза 3) в `crates/stellwerk-core/tests/`.
  Их нельзя править: они приказ, а не предложение.
- Правило проекта: `todo!()` лучше неверного ответа — пока предикат не реализован,
  вызов падает loudly.

## Что сделать

Семантика из `tests/workspace_policy.rs`:
- `Deny` запрещает ЛЮБОЙ url: и `""`, и `not-a-url-at-all`, и `file:///etc/passwd`;
- `AllowList` разрешает url, только если ХОСТ входит в список ТОЧНО: `evil.example.com`
  не проходит под `example.com`, `example.com.evil.net` — тоже;
- пустой `AllowList` запрещает всё, иначе опечатка в конфиге молча открыла бы сеть;
- `Allow` разрешает всё без разбора.

1. Извлечь хост из url БЕЗ новых зависимостей: подстрока между `://` и первым из `/ ? #`.
   У `file://` хост пуст.
2. Сравнение — точное равенство строк. НЕ приводить к нижнему регистру: тесты требуют
   строгого поведения.
3. `Allow` → `true` сразу.
4. Удалить `let _ = url;`.

## Evidence

- `cargo test -p stellwerk-core --test workspace_policy` → 8 passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
