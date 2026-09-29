# packet-11b
plan_id: M3
goal: RED: маршрутизация вызова — что допускается, что отказывает и что попадает в чек.
files_allowed: [crates/stellwerk-gateway/tests/session.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test session
acceptance: RED: `bash -c 'cargo build -p stellwerk-gateway && cargo test -p stellwerk-gateway --test session --no-run && ! cargo test -p stellwerk-gateway --test session'` → exit 0.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [11a]
data_class: public
lane: m3/session-routing-red

## Контекст

- Вторая часть разбиения M3-11. Поверхность типов объявлена в **11a**; здесь
  закрепляется поведение маршрутизации, в 11c — вопросы, таймауты и сбои.
- Gateway инструменты не выполняет: он решает, что можно, спрашивает, когда надо, и
  пишет чек.
- Решения принимаются ТОЛЬКО через предикаты контракта: `RiskClass::required_level`,
  `PolicyLevel::permits`, `DataClass::permits_model`. Своих копий правил не заводим.
- `src/session.rs` и `src/lib.rs` в `files_allowed` НЕ входят: они принадлежат 11a,
  и трогать их здесь — значит ловить седьмой раз тот же конфликт за один файл.
- Шов `ToolBackend` введён в 11a именно для этого: тест подставляет подделку и не
  зовёт внешний процесс. Подделка обязана **считать вызовы**: «инструмент не должен
  быть вызван» недоказуемо, если не видно, что его не вызвали.
- Тот же шов объясняет, почему крейт не зависит от `tokio`: исполнятеля для
  асинхронного `handle` в тесте взять неоткуда, а `Cargo.toml` вне `files_allowed`.
  Тест крутит future сам, и это оговорено в 11a.

## Что сделать

1. Тесты в `tests/session.rs`, четыре сценария маршрутизации. Импорт публичным API
   (`use stellwerk_gateway::session::…`), **не** `#[path]` (F-14).
2. `unknown_tool_is_denied_before_the_backend_ever_sees_it` — инструмент не в
   монтаже даёт `Verdict::Deny`, и подделка **не зафиксирована ни одного вызова**.
3. `deny_for_insufficient_level_still_leaves_a_receipt` — отказ по уровню полномочий
   всё равно оставляет чек: запрет — тоже факт, обязанный попасть в поток, иначе
   отклонённые попытки не видны в расследовании. Тест требует, чтобы чек был, и
   чтобы в нём стоял именно отказ.
4. `r5_is_denied_at_once_and_leaves_no_question_behind` — R5 отклоняется сразу,
   `Verdict::Suspended` не возникает и inbox остаётся пустым. Отсечка по риску
   касается только R5: у R3/R4 вердикт `Ask` разбирается в `decide`, и отсечка здесь
   сделала бы решения В1/В4 мёртвым кодом.
5. `an_allowed_call_yields_exactly_one_receipt_and_counts_itself` — успешный вызов
   даёт **ровно один** чек и увеличивает `calls`. Успех, который не оставил следа,
   неотличим от вызова, которого не было.
6. Каждый отказ утверждай **по варианту** (`Verdict::Deny`, `Verdict::Suspended`,
   `SessionError::…`), а не через `is_err` и не флагом: закон 6 требует, чтобы
   «запрещено политикой» и «никто не ответил» не совпадали.
7. `handle` остаётся **RED-заглушкой**; реализация — packet-12.

## Evidence

- RED-вывод `cargo test -p stellwerk-gateway --test session`: четыре теста
  компилируются и падают на заглушке.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
