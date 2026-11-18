# Task 14 — V2-10 (F-11): align V1/packet-01 with the shipped Unpark+park

Packet: `.sisyphus/plans/V1/packet-01.md` (canon) + `.omo/plans/V1/packet-01.md` (mirror)
Commit: `7ac57cd05534c241e50bf6fbf544bad60a0e9108` — `docs(plans): align packet-01 with Unpark+park`
Files changed: exactly 2
- `.sisyphus/plans/V1/packet-01.md` (3+/3-)
- `.omo/plans/V1/packet-01.md` (3+/3-)

## RED evidence (before edit)

```
$ rg -n "Waker::noop" .sisyphus/plans/V1/packet-01.md; echo "EXIT=$?"
15:Ручной no-op waker заменён на `Waker::noop()`, и пробуждённая задача действительно продолжает исполнение, потому что зелёный clippy на самодельном waker ничего не говорит о планировании.
EXIT=0

$ rg -n "Unpark|park" .sisyphus/plans/V1/packet-01.md; echo "EXIT=$?"
EXIT=1
```

The packet described `Waker::noop()` as the fix and had zero mention of the shipped
`Unpark`/`park` implementation.

## GREEN evidence (after edit)

```
$ ! rg -n "Waker::noop" .sisyphus/plans/V1/packet-01.md; echo "NO_MATCH_EXIT=$?"
NO_MATCH_EXIT=0

$ rg -n "Unpark|park" .sisyphus/plans/V1/packet-01.md; echo "MATCH_EXIT=$?"
15:Ручной no-op waker заменён на настоящий: `drive` строит `Unpark(std::thread::current())`, реализующий `Wake` через `unpark`, и спит в `park` на `Pending`, поэтому пробуждённая задача действительно продолжает исполнение — зелёный clippy на самодельном waker ничего не говорит о планировании.
18:- `crates/stellwerk-gateway/src/session.rs:608-637` — `pub fn drive`: `Unpark(thread::current())` + цикл `poll`, где `Poll::Pending` уходит в `park`, а `wake` будит поток через `unpark`.
MATCH_EXIT=0

$ cargo test -p stellwerk-gateway --test session
   Compiling stellwerk-core v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-core)
   Compiling stellwerk-store v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-store)
   Compiling stellwerk-gateway v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-gateway)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.83s
     Running tests/session.rs (target/debug/deps/session-704a343e368d7809)

running 5 tests
test deny_for_insufficient_level_still_leaves_a_receipt ... ok
test an_allowed_call_yields_exactly_one_receipt_and_counts_itself ... ok
test unknown_tool_is_denied_before_the_backend_ever_sees_it ... ok
test r5_is_denied_at_once_and_leaves_no_question_behind ... ok
test woken_task_continues_after_waker_replacement ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

$ python3 scripts/plan-lint.py
H1: 9 пакетов, нарушений нет
M1: 2 пакетов, нарушений нет
N1: 10 пакетов, нарушений нет
N2: 27 пакетов, нарушений нет
TUI: 5 пакетов, нарушений нет
V1: 34 пакетов, нарушений нет
V2: 17 пакетов, нарушений нет
ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном
ПЕРЕСЕЧЕНИЯ МЕЖДУ MILESTONE: порядок объявлен везде
ТАБЛИЦА risk_class: совпадает с contract.rs во всех трёх документах
LINT_EXIT=0

$ diff -q .sisyphus/plans/V1/packet-01.md .omo/plans/V1/packet-01.md; echo "DIFF_EXIT=$?"
DIFF_EXIT=0

$ git show --numstat --oneline HEAD
7ac57cd docs(plans): align packet-01 with Unpark+park
3	3	.omo/plans/V1/packet-01.md
3	3	.sisyphus/plans/V1/packet-01.md
```

`ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном` — C8 clean. `diff -q` exit 0 — mirror is a
byte-for-byte copy of canon. `git show --numstat` — exactly 2 files.

## git diff

```
diff --git a/.omo/plans/V1/packet-01.md b/.omo/plans/V1/packet-01.md
index 6a4119f..b15fa39 100644
--- a/.omo/plans/V1/packet-01.md
+++ b/.omo/plans/V1/packet-01.md
@@ -12,14 +12,14 @@ acceptance: cargo test -p stellwerk-gateway --test session
 # 01 — fmt и clippy в зелёное

 ## Цель
-Ручной no-op waker заменён на `Waker::noop()`, и пробуждённая задача действительно продолжает исполнение, потому что зелёный clippy на самодельном waker ничего не говорит о планировании.
+Ручной no-op waker заменён на настоящий: `drive` строит `Unpark(std::thread::current())`, реализующий `Wake` через `unpark`, и спит в `park` на `Pending`, поэтому пробуждённая задача действительно продолжает исполнение — зелёный clippy на самодельном waker ничего не говорит о планировании.

 ## Обоснование (измерено)
-- `crates/stellwerk-gateway/src/session.rs:613` — здесь стоит ручной no-op waker, который clippy отвергает и который не гарантирует возобновление задачи.
+- `crates/stellwerk-gateway/src/session.rs:608-637` — `pub fn drive`: `Unpark(thread::current())` + цикл `poll`, где `Poll::Pending` уходит в `park`, а `wake` будит поток через `unpark`.
 - `crates/stellwerk-task/tests/http_wire.rs` — тест попадает в тот же fmt/clippy прогон, поэтому механическая правка касается и его.

 ## Шаг 1 — падающий тест
-`crates/stellwerk-gateway/tests/session.rs`, тест `woken_task_continues_after_waker_replacement`: утверждает, что задача после пробуждения исполняется до конца; до замены waker ручной no-op не планирует задачу, поэтому продолжение не наблюдается и тест падает по таймауту ожидания результата.
+`crates/stellwerk-gateway/tests/session.rs`, тест `woken_task_continues_after_waker_replacement`: `WakesOnce` на первом опросе передаёт единственный waker второму потоку и возвращает `Pending`, а `Ready` отдаёт только после `wake`; `drive` опрашивает future на отдельном потоке, `recv_timeout(5s)` превращает потерянное пробуждение в отказ, а `polls <= 100` ловит busy-poll. До замены waker ручной no-op не планирует задачу, поэтому продолжение не наблюдается и тест падает по таймауту ожидания результата.

 ## Приёмка
 cargo test -p stellwerk-gateway --test session
diff --git a/.sisyphus/plans/V1/packet-01.md b/.sisyphus/plans/V1/packet-01.md
index 6a4119f..b15fa39 100644
--- a/.sisyphus/plans/V1/packet-01.md
+++ b/.sisyphus/plans/V1/packet-01.md
@@ -12,14 +12,14 @@ acceptance: cargo test -p stellwerk-gateway --test session
 # 01 — fmt и clippy в зелёное

 ## Цель
-Ручной no-op waker заменён на `Waker::noop()`, и пробуждённая задача действительно продолжает исполнение, потому что зелёный clippy на самодельном waker ничего не говорит о планировании.
+Ручной no-op waker заменён на настоящий: `drive` строит `Unpark(std::thread::current())`, реализующий `Wake` через `unpark`, и спит в `park` на `Pending`, поэтому пробуждённая задача действительно продолжает исполнение — зелёный clippy на самодельном waker ничего не говорит о планировании.

 ## Обоснование (измерено)
-- `crates/stellwerk-gateway/src/session.rs:613` — здесь стоит ручной no-op waker, который clippy отвергает и который не гарантирует возобновление задачи.
+- `crates/stellwerk-gateway/src/session.rs:608-637` — `pub fn drive`: `Unpark(thread::current())` + цикл `poll`, где `Poll::Pending` уходит в `park`, а `wake` будит поток через `unpark`.
 - `crates/stellwerk-task/tests/http_wire.rs` — тест попадает в тот же fmt/clippy прогон, поэтому механическая правка касается и его.

 ## Шаг 1 — падающий тест
-`crates/stellwerk-gateway/tests/session.rs`, тест `woken_task_continues_after_waker_replacement`: утверждает, что задача после пробуждения исполняется до конца; до замены waker ручной no-op не планирует задачу, поэтому продолжение не наблюдается и тест падает по таймауту ожидания результата.
+`crates/stellwerk-gateway/tests/session.rs`, тест `woken_task_continues_after_waker_replacement`: `WakesOnce` на первом опросе передаёт единственный waker второму потоку и возвращает `Pending`, а `Ready` отдаёт только после `wake`; `drive` опрашивает future на отдельном потоке, `recv_timeout(5s)` превращает потерянное пробуждение в отказ, а `polls <= 100` ловит busy-poll. До замены waker ручной no-op не планирует задачу, поэтому продолжение не наблюдается и тест падает по таймауту ожидания результата.

 ## Приёмка
 cargo test -p stellwerk-gateway --test session
```

Frontmatter (`status: GREEN`, `id`, `milestone`, `depends_on`, `files_allowed`,
`limit_waiver`, `overlap_waiver`, `acceptance`) is untouched — the diff starts at line 12.

## Notes

- Docs-only packet: no source or test file touched; `session.rs` and `tests/session.rs`
  were already GREEN and only the description was corrected.
- Canon and mirror edited identically in the same commit so C8 stays clean.

## PolyForge attestation (orchestrator, on main)

- `evidence_append(ModelClaim, task_id=V2/packet-10)` -> seq 400, hash
  `74b16c10aba41e5604153ddc67e81b6d38635204aab3507603ae485efeb31e2c`,
  commit `7ac57cd05534c241e50bf6fbf544bad60a0e9108`, diff_hash
  `a557c68150d4f178ac24089d52b89c4b29f01f9529f5cf3e743be35e1c9826fe`.
- `evidence_verify(cargo test -p stellwerk-gateway --test session)` -> seq 401
  `Verified`, exit 0, stdout_hash
  `795adba2a87970a4bec81829ea54426bd6b09786c36d9eff5aec0ddc39d0d2db`,
  `commit_sha` and `diff_hash` match the claim.
- `gate_evaluate(required=["Verified"])` -> passed, claimed 1 / verified 1 /
  validated 0, missing `[]`, chain tail
  `1002935889107322fe903930f2196e92b2e84e99b7660f3bea53ce20c1bf93ad`.

Orchestrator re-verification before ticking the checkbox (independent of the worker
report): `git show --numstat 7ac57cd` -> 2 files, 3/3 each, nothing outside
`.sisyphus/plans/V1/packet-01.md` + `.omo/plans/V1/packet-01.md`;
`rg -n "Waker::noop" .sisyphus/plans/V1/packet-01.md` exit 1;
`rg -n "Unpark|park"` matches lines 15 and 18; `diff -q` canon/mirror exit 0;
`python3 scripts/plan-lint.py` exit 0 with `ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном`;
`cargo test -p stellwerk-gateway --test session` -> 5 passed, 0 failed.
