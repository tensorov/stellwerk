# packet-06
plan_id: M3
goal: Реализовать shaping вывода: усечение, запись артефакта, хеш по полному телу.
files_allowed: [crates/stellwerk-gateway/src/shaping.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test shaping
acceptance: `cargo test -p stellwerk-gateway --test shaping` → 6 passed; 0 failed.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [05]
data_class: public
lane: m3/shaping

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

1. Порог: `std::env::var("STELLWERK_MAX_OUTPUT_TOKENS")` с fallback 2048; непарсимое
   значение → 2048 молча? НЕТ: верни `ShapeError::Config` — опечатка в конфиге не
   должна тихо отключать защиту.
2. Считай токены по формуле `round(bytes * tokens_per_byte)`. Значение `tokens_per_byte`
   приходит аргументом (калибровка — из `modules.lock`, не хардкодь 0.813).
3. `sha2` уже в workspace. Хеш полного вывода: hex-строка в нижнем регистре.
4. Артефакт: имя `<sha256>.txt` — детерминированное, одинаковый вывод перезаписывает
   себя, а не плодит мусор. Пиши атомарно: во временный файл, затем rename.
5. Усечение: оставь первые байты, рассчитанные как `MAX_OUTPUT_TOKENS / tokens_per_byte`,
   и добавь маркер обреза. Не режь посреди многобайтного UTF-8 — символ должен быть
   валидным, иначе битый вывод уходит в контекст модели.
6. Создай `dir`, если нет (`create_dir_all`). Никаких `rm` в крейте.

## Evidence

- `cargo test -p stellwerk-gateway --test shaping` → passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
