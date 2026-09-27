# packet-02
plan_id: M3
goal: Реализовать отображение RiskClass → Permission по лестнице R0–R5.
files_allowed: [crates/stellwerk-gateway/src/permission.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test permission
acceptance: `cargo test -p stellwerk-gateway --test permission` → 9 passed; 0 failed. Все 9 тестов написаны в M3/packet-01; этот пакет реализует `permission.rs` так, чтобы они прошли, и НЕ создаёт тестов.
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

### В1 — R4: `Deny` → `Ask`, и только при `L5Owner`

Таблица берёт **уровень из контракта** (`RiskClass::required_level`, см. ADR-0002),
 gateway своих копий уровней не заводит. R4 — это `L5Owner` + `Ask`:

- при `L4Admin` и ниже R4 даёт `Deny`;
- при `L5Owner` — `Ask`, разрешение на **одно действие**;
- `ApproveForTask` для R4 — **явное отдельное действие человека**, не дефолт:
  одобрение одного `gh pr create` не открывает все внешние вызовы до конца задачи.
- Таймаут ожидания R4 — **4 часа** (внешние действия теряют актуальность; общий
  дефолт 24ч применяется к R2/R3).
- R5 остаётся `Deny` безусловно для любого уровня, включая `L5Owner`. Владелец
  делает `push --force` руками, не через агента.

### В2 — неклассифицированный инструмент: отдельное состояние, не `R0`

`risk.unwrap_or(R0)` запрещён: отсутствие данных превращается в разрешение —
ровно тот класс ошибки, ради которого существует M3.

```rust
enum ToolRisk { Classified(RiskClass), Unclassified }
```

- `Unclassified` в неинтерактивном прогоне → **всегда `Deny`**, без suspend.
  Приостанавливать задачу ради неизвестного инструмента бессмысленно: правильный
  ответ — классифицировать его в `modules.toml` и перезапустить.
- `Ask` при `Unclassified` допустим **только** в интерактивной сессии `L5Owner`,
  где человек видит имя инструмента и аргументы.
Тесты на `Unclassified` (включая обязательный «новый MCP-тул без `risk` в манифесте
→ `Deny`, а не `Allow`») написаны в **M3/packet-01**, потому что этот пакет не
имеет права трогать тесты. Здесь они только исполняются.

## Evidence

- `cargo test -p stellwerk-gateway --test permission` → passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
