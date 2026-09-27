# packet-01
plan_id: M3
goal: Создать крейт `stellwerk-gateway` и падающий тест отображения RiskClass → permission. contract-change: добавляется член workspace.
files_allowed: [Cargo.toml, crates/stellwerk-gateway/Cargo.toml, crates/stellwerk-gateway/src/lib.rs, crates/stellwerk-gateway/src/permission.rs, crates/stellwerk-gateway/tests/permission.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test permission
acceptance: RED: `bash -c 'cargo build -p stellwerk-gateway && cargo test -p stellwerk-gateway --test permission --no-run && ! cargo test -p stellwerk-gateway --test permission'` → exit 0: собирается, тест компилируется, падает на assert.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [—]
data_class: public
lane: m3/skeleton

## Контекст

- **Новый крейт `stellwerk-gateway`** — виртуальный MCP-сервер: он НЕ выполняет
  инструменты сам, а решает, что можно, спрашивает, когда надо, и пишет чек.
- Крейт зависит от `stellwerk-core` и `stellwerk-store` по path. Новых крейтов нет;
  `cargo add` запрещён.
- Решения принимаются ТОЛЬКО через предикаты контракта:
  `RiskClass::required_level`, `PolicyLevel::permits`, `DataClass::permits_model`
  (все реализованы в M1, packet-05/06/04). Своих копий правил не заводить.
- Схемы MCP-модулей уже измерены: `measure` даёт `context_cost_tokens` (polyforge = 813).
  Gateway решает, что монтировать, по этим числам, а не по вкусу.
- Ключи и секреты не читать и не логировать: `auth.json`, `models.json`, `env | grep KEY`
  запрещены (`AGENTS.md`).

## Что сделать

**contract-change:** правка корневого `Cargo.toml` — только добавление члена `members`.

1. Корневой `Cargo.toml`: `crates/stellwerk-gateway` в `members`. Больше ничего.
2. `Cargo.toml` крейта: `stellwerk-core`, `stellwerk-store` по path; из workspace —
   `serde`, `serde_json`, `thiserror`, `sha2`. `clap` и `tokio` — по необходимости.
3. `src/permission.rs`:
   - `pub enum Permission { Allow, Ask, Deny }` (сериализуется lowercase);
   - `pub fn permission_for(risk: RiskClass) -> Permission` — **RED-заглушка**;
   - `PermissionError` (thiserror) с `NotImplemented`.
4. Тесты в `tests/permission.rs` — лестница решений:

| RiskClass | Permission | почему |
|---|---|---|
| R0 | Allow | чтение, эффекта нет |
| R1 | Allow | запись в песочницу, откатывается её удалением |
| R2 | Ask | сетевой выход: наружу уходят данные |
| R3 | Ask | доступ к секретам: решает человек |
| R4 | Deny | внешний наблюдаемый эффект без ведома владельца |
| R5 | Deny | необратимое разрушение |

   Плюс: `permission_for` для всех шести классов проверяется таблицей, а не
   перечислением — иначе добавление R6 в контракт не сломает тест молча.
5. Реализацию НЕ пиши — это packet-02.

## Evidence

- RED-вывод `cargo test -p stellwerk-gateway --test permission`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
