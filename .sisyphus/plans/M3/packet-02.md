# packet-02
plan_id: M3
goal: Реализовать отображение RiskClass → Permission по лестнице R0–R5.
files_allowed: [crates/stellwerk-gateway/src/permission.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test permission
acceptance: `cargo test -p stellwerk-gateway --test permission` → 7 passed; 0 failed (6 классов + 1 на полноту таблицы).
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [01]
data_class: public
lane: m3/permission

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

1. `permission_for` — `match` по всем шести вариантам RiskClass. Исчерпывающий match
   обязателен: новый класс риска в контракте обязан заставить компилятор спросить
   решение, а не упасть в default.
2. `default`-ветки НЕ оставлять. Если завтра появится R6, молчаливый `Deny` был бы
   допустим (сберегает данные), но молчаливый `Allow` — катастрофа.
3. Таблицу вынеси в `const RISK_PERMISSION: [(RiskClass, Permission); 6]` и итерируй
   по ней: тогда тест «полнота таблицы» проверяет, что в ней ровно все классы
   (сравни длину с числом вариантов через исчерпывающий счётчик в тесте).
4. Убери `NotImplemented` из ошибок, если вариант стал неиспользуемым.

## Evidence

- `cargo test -p stellwerk-gateway --test permission` → passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
