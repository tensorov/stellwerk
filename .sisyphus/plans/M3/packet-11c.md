# packet-11c
plan_id: M3
goal: RED: вопрос оператору, его истечение, запрет субагентов и сбой инструмента.
files_allowed: [crates/stellwerk-gateway/tests/session_failure.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test session_failure
acceptance: RED: `bash -c 'cargo build -p stellwerk-gateway && cargo test -p stellwerk-gateway --test session_failure --no-run && ! cargo test -p stellwerk-gateway --test session_failure'` → exit 0.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [11b]
data_class: public
lane: m3/session-failure-red

## Контекст

- Третья и последняя часть разбиения M3-11. Поверхность — в 11a, маршрутизация — в
  11b, здесь вопросы оператору, их истечение, запрет субагентов и сбой инструмента.
- Разбито на три части, а не на две: восемь сценариев исходного пакета не влезают в
  один пакет с лимитом 200 строк, а урезать их значило бы выбросить требования В3/В4.
- `src/session.rs` и `src/lib.rs` в `files_allowed` НЕ входят — они принадлежат 11a.
  - У крейта нет `tokio`, и `Cargo.toml` вне `files_allowed`, поэтому исполнятеля для
    асинхронного `handle` тест берёт тот же, что и в 11b: `drive` из
    `stellwerk_gateway::session`, объявленный в 11a на `std`. Им пользоваться, свой
    не писать: без `tokio` и без права трогать `Cargo.toml` другого источника нет, а
    самописный исполнитель в каждом тесте — это расхождение, которое потом придётся
    искать в трёх местах сразу.

## Что сделать

1. Тесты в `tests/session_failure.rs`, четыре сценария. Импорт публичным API, **не**
   `#[path]` (F-14).
  2. `ask_in_a_non_interactive_run_suspends_and_leaves_the_question_in_the_inbox` —
     `Ask` в неинтерактивном прогоне даёт `Verdict::Suspended`, инструмент **не
     вызван**, и в `.stellwerk/inbox/<run_id>.json` лежит `ApprovalRequest` с
     непустым `args_hash`. Это **НЕ `Deny`**: «никто не ответил» и «запрещено» —
     разные вещи, и в расследовании их нельзя смешивать. Тест обязан читать файл, а не
     доверять возвращённому значению: запись в поток и её отсутствие неотличимы, пока
     не проверишь файл.
     **Временный каталог — так же, как в `tests/shaping.rs`:**
     `std::env::temp_dir().join(format!("stellwerk-session-{}-{tag}",
     std::process::id()))` плюс `fs::create_dir_all` и `remove_dir_all` на drop.
     Крейта `tempfile` в workspace нет и `Cargo.toml` вне `files_allowed`, поэтому
     искать нечего: `cargo add` запрещён, а своя реализация фикстуры без Drop —
     это ровно тот «хелпер-реализация», ради которого тестовый бюджет и ограничен.
3. `an_expired_question_is_not_a_deny` — истёкший вопрос даёт отказ с
   `SessionError::ApprovalTimeout`, и тест явно требует, чтобы это **не** было
   `Verdict::Deny`. Причина обязана называться: «невалидно» без причины нечинимо.
4. `risk_r2_and_above_forbids_subagents_at_the_gate` — при `risk >= R2` глубина
   субагентов обязана остаться 0, и проверяется это **полем**, а не текстом
   инструкции: правило без наблюдаемого состояния через полгода не отличить от
   отсутствия правила.
5. `a_failing_tool_still_leaves_a_receipt_behind` — ошибка инструмента даёт
   `SessionError::Tool`, **и чек при этом остаётся**: сбой вызова — тоже факт. Чек
   едет внутри варианта ошибки, потому что при сигнатуре
   `Result<CallOutcome, SessionError>` иначе он просто исчез бы, а вызов без следа
   неотличим от небывшего. Тест обязан проверить, что чек доступен ПОСЛЕ ошибки, и
   отдельно — что инструмент был вызван ровно один раз.
6. Утверждай по варианту, а не через `is_err`.
7. `handle` остаётся **RED-заглушкой**; реализация — packet-12.

## Evidence

- RED-вывод `cargo test -p stellwerk-gateway --test session_failure`: четыре теста
  компилируются и падают на заглушке.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
