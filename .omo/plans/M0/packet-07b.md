# packet-07b
plan_id: M0
goal: Калибровка (calibration) на реальном polyforge-mcp: rename bytes_per_token→tokens_per_byte, подобрать коэффициент 813/B, записать measured_at и modules.lock в 732..=894.
files_allowed: [modules.toml, modules.lock]
max_diff_lines: 200
test: cargo test -p stellwerk-registry measure
acceptance: (1) `command -v polyforge-mcp` → 0, иначе STATUS: BLOCKED; (2) modules.toml: ключ `tokens_per_byte` (не bytes_per_token), `measured_at` непустой ISO8601; (3) после `target/debug/stellwerk module measure polyforge` modules.lock: `context_cost_tokens` integer ∈ [732, 894]; (4) `cargo test -p stellwerk-registry measure` → 0 (регресс, alias в model уже принимает новый ключ — код НЕ менялся с 07a, files_allowed не включает src).
forbidden: [cargo add, правка корневого Cargo.toml, правка src/**, правка contract.rs, правка cli/model, ручная подгонка context_cost_tokens (пишет только бинарь), ослабление тестов]
depends_on: [packet-07a]

## Приёмочный packet (не фича)

Lane m0/measure, ветка после 07a. Ты работаешь с собранным бинарём и конфигом. Никакого Rust-кода.

Верbatim (оставить в modules.toml комментарии при редактировании / в отчёте):

> 813 ±10% — это context_input-дельта измерения OpenCode (docs/CONTEXT_BUDGET.md), включая оверхед форматирования тулов, НЕ чистые байты схем; тест — pinning round-trip (bytes × tokens_per_byte ≈ 813), не проверка правильности коэффициента.

## Процедура (по шагам)

1. Precondition: `command -v polyforge-mcp` → нет: STATUS: BLOCKED (нужен PATH).
2. `cargo build -p stellwerk-registry` (бинарь `target/debug/stellwerk`).
3. В modules.toml заменить блок calibration:
   - `bytes_per_token` → `tokens_per_byte` (model.rs имеет alias на старый ключ, но после rename файл использует новое имя сериализации; alias продолжает принимать оба — код не трогаем).
   - Значение оставь `0.0` пока; `measured_at` пока `""`.
4. Прогон 0 (замер байтов): `target/debug/stellwerk module measure polyforge` → stdout `measure polyforge: schema_bytes=<B> context_cost_tokens=0` (tpb=0). Если ответ на initialize — не JSON-RPC result с protocolVersion (фрейминг/версия протокола не та, что в 07a) → STATUS: BLOCKED, калибровку не продолжай. Если модуль уже с командой polyforge-mcp и она в PATH — OK. Выпиши B.
5. Вычисли `tokens_per_byte = round(813.0 / B, 6)` (6 знаков после запятой, f64). Запиши в modules.toml. `measured_at` = текущий ISO8601 (UTC), напр. `date -u +%Y-%m-%dT%H:%M:%SZ`.
6. Прогон 1: снова `target/debug/stellwerk module measure polyforge` → бинарь пересчитывает T=round(B*tpb) и пишет modules.lock. Ожидай T ∈ 732..=894 (813±10%).
7. Если T вне диапазона: проверь B из stderr/логов, повтори с точностью до деления; если сервер недоступен/падает — BLOCKED.
8. Регресс: `cargo test -p stellwerk-registry measure` → 0 (фикстурные тесты не зависят от репо-файлов).
9. Проверь modules.lock: `version = 1`, секция `[polyforge]`, `context_cost_tokens = <T>` целое.

## Не делать

- Не подгоняй T руками в lock — только бинарь.
- Не правь src/ (включая alias — он уже there с 01).
- Не добавляй новые ключи в calibration сверх tokens_per_byte + measured_at.

## Evidence

- Приложи: вывод прогона 0 (с B), формулу 813/B, вывод прогона 1 (с T), `grep -A2 polyforge modules.lock`, вывод cargo test measure.
- PROOF: evidence_append + evidence_verify(cmd='cargo test -p stellwerk-registry measure').
- GATE: gate_evaluate.
- STATUS: DONE (или BLOCKED с причиной) / FILES / TEST / EVIDENCE.
