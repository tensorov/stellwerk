---
id: 01
milestone: V1
status: GREEN
depends_on: [00]
files_allowed: [crates/stellwerk-gateway/src/session.rs, crates/stellwerk-gateway/tests/session.rs, crates/stellwerk-gateway/tests/session_failure.rs, crates/stellwerk-gateway/tests/shaping.rs, crates/stellwerk-task/tests/http_wire.rs, crates/stellwerk-task/tests/model_backend.rs]
limit_waiver: 6 файлов — механические правки fmt/clippy в одном проходе, путь исполнения не меняется
overlap_waiver: C9 — N2/00g3, N2/00g3b, N2/00j4, N2/00j5, N2/00j7a — общие тестовые файлы, последовательное исполнение
acceptance: cargo test -p stellwerk-gateway --test session
---

# 01 — fmt и clippy в зелёное

## Цель
Ручной no-op waker заменён на настоящий: `drive` строит `Unpark(std::thread::current())`, реализующий `Wake` через `unpark`, и спит в `park` на `Pending`, поэтому пробуждённая задача действительно продолжает исполнение — зелёный clippy на самодельном waker ничего не говорит о планировании.

## Обоснование (измерено)
- `crates/stellwerk-gateway/src/session.rs:608-637` — `pub fn drive`: `Unpark(thread::current())` + цикл `poll`, где `Poll::Pending` уходит в `park`, а `wake` будит поток через `unpark`.
- `crates/stellwerk-task/tests/http_wire.rs` — тест попадает в тот же fmt/clippy прогон, поэтому механическая правка касается и его.

## Шаг 1 — падающий тест
`crates/stellwerk-gateway/tests/session.rs`, тест `woken_task_continues_after_waker_replacement`: `WakesOnce` на первом опросе передаёт единственный waker второму потоку и возвращает `Pending`, а `Ready` отдаёт только после `wake`; `drive` опрашивает future на отдельном потоке, `recv_timeout(5s)` превращает потерянное пробуждение в отказ, а `polls <= 100` ловит busy-poll. До замены waker ручной no-op не планирует задачу, поэтому продолжение не наблюдается и тест падает по таймауту ожидания результата.

## Приёмка
cargo test -p stellwerk-gateway --test session
Ожидается PASS.

## Запрещено
- нельзя добавлять `#[allow]` вместо замены waker.
- запрещено перезаписывать `tests/session_failure.rs` и `tests/shaping.rs`, если `git status --porcelain` по ним не пуст.
- нельзя менять lint-политику ради зелёного clippy.
