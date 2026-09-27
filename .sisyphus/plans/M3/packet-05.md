# packet-05
plan_id: M3
goal: RED: shaping вывода — ответ длиннее 2k токенов обязан превратиться в хеш и путь к артефакту, а не литься в контекст.
files_allowed: [crates/stellwerk-gateway/src/shaping.rs, crates/stellwerk-gateway/tests/shaping.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test shaping
acceptance: RED: пакет собирается, тест компилируется, прогон падает на заглушке.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [04]
data_class: public
lane: m3/shaping-red

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

Это самый «дешёвый по токенам, дорогой по деньгам» механизм проекта: 2k токенов на
вызов, а воркер живёт в окне ~43k (см. `docs/CONTEXT_BUDGET.md`).

1. `src/shaping.rs`:
   - `pub const MAX_OUTPUT_TOKENS: u64 = 2048;`
   - `pub struct Shaped { pub text: String, pub truncated: bool, pub sha256: String,
     pub artifact_path: Option<PathBuf> }`;
   - `pub fn shape(output: &str, tokens_per_byte: f64, dir: &Path) -> Result<Shaped, ShapeError>` — **RED-заглушка**.
2. Логика, которую задают тесты:
   - `tokens = round(bytes * tokens_per_byte)` — та же формула, что в
     `stellwerk_registry::model::tokens_for`. Не заводи свою: формула калибруется;
   - `tokens <= 2048` → `truncated=false`, `artifact_path=None`, текст как есть;
   - `tokens > 2048` → первые N байт остаются в `text`, полное тело пишется в
     `<dir>/<sha256>.txt`, `truncated=true`, `sha256` заполнен;
   - хеш считается по **полному** выводу, а не по усечённому: иначе ссылка на артефакт
     не проверяема;
   - `MAX_OUTPUT_TOKENS` переопределяется из переменной окружения
     `STELLWERK_MAX_OUTPUT_TOKENS` (значение по умолчанию 2048).
3. Тесты в `tests/shaping.rs`:
   - короткий вывод проходит без изменений (байт в байт);
   - длинный вывод усекается, `truncated == true`, `artifact_path` существует;
   - содержимое артефакта совпадает с исходным выводом;
   - `sha256` в ответе равен хешу полного вывода;
   - порог из окружения уважается;
   - `dir` не существует → `ShapeError::Io`, а не паника.

## Evidence

- RED-вывод `cargo test -p stellwerk-gateway --test shaping`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
