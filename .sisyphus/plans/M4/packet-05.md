# packet-05
plan_id: M4
goal: RED — чек пишется на КАЖДЫЙ вызов, включая запрещённый, и worker не запускается при отказе.
files_allowed: [crates/stellwerk-task/src/runner.rs, crates/stellwerk-task/tests/vertical.rs, crates/stellwerk-task/src/lib.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --test vertical
acceptance: RED: тест компилируется, прогон падает на заглушке. Каждый сценарий отдельным `#[test] fn`.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [03, M2-09, M2-10, M2-11, M2-12, M2-01, M2-05, M2-07]
data_class: public
lane: m4/vertical

## Контекст

- **Это смысловой пакет M4.** До него federation описывала решения; здесь
  проверяется, что решение влияет на то, что реально произойдёт.
- `EventStore::append` уже есть. `evidence.rs` появляется в M2-12; этот пакет
  объявляет в `runner.rs` поверхность, которую пакет 06 наполняет.
- Песочница — `scripts/worker/sandbox.sh`, её контракт задан M2-07/08. В этом
  пакете песочница только **вызывается** в разрешённом пути, её устройство не
  трогается.

## Что сделать

1. `src/runner.rs` — объявить:
   - `pub struct RunRequest { pub task_id: String, pub call: Call, pub caller: PolicyLevel, pub workspace_ref: String }`;
   - `pub trait Worker { async fn exec(&self, workspace_ref: &str, call: &Call) -> Result<ExecOutcome, WorkerError>; fn spawned(&self) -> usize; }`
     — `spawned()` считает **фактические** запуски, и это то, чем тест ловит
     «worker запустился, хотя не должен был»;
   - `pub fn run_once(request: &RunRequest, worker: &dyn Worker, store: &dyn EventStore) -> Result<Verdict, RunError>` —
     **RED-заглушка**.
2. `tests/vertical.rs` — RED-сценарии:
   - `Deny` → worker **не** вызван: `spawned() == 0`, чек записан, в чеке причина;
   - `Deny` → в чеке причина **непустая** и содержит код правила;
   - `Unclassified` → `Deny`, worker не вызван (тот же сценарий, но другой вход:
     отсутствие данных не должно выглядеть как «пропустим»);
   - `R5` при `L5Owner` → `Deny`, worker не вызван;
   - `Allow` → worker вызван ровно один раз, чек записан;
   - **запрещённый вызов тоже виден в расследовании**: `list_by_task` возвращает
     событие и для `Deny`-вызова, и для `Allow`-вызова, и по записи видно, чем
     они различаются;
   - worker вернул ошибку → чек **всё равно** записан, статус `Failed`, причина
     есть. Неудача обязана быть видна так же, как отказ.

## Почему `spawned()` — отдельный счётчик

Иначе тест «worker не вызван» пришлось бы проверять по отсутствию события, а
отсутствие события неотличимо от «событие записать забыли». Счётчик отделяет
факт запуска от факта записи — иначе тест на отказ проходит и при молчащем
раннере, то есть не проверяет ничего.

## Правило объявления модуля

`src/lib.rs` добавлен в `files_allowed`, и ``pub mod runner;`` объявляется **в этом пакете**,
рядом с созданием ``runner.rs``, а не в следующем GREEN.

Интеграционный тест берёт типы через публичный API крейта: без объявления модуля
он не компилируется, а с `#[path]` скомпилировал бы вторую копию и оставил экспорт
непроверенным (F-14). Требование, висящее на пакете ПОСЛЕ зависимого от него RED,
делает RED несуществующим — так был заблокирован M2-05.

`lib.rs` — общий ресурс, а не частный файл пакета: на `stellwerk-task/src/lib.rs`
претендуют пакеты M2-01, 05, 07, 09, 11, 12, и порядок держится транзитивной
цепочкой. Пакеты M4 объявляют тех владельцев из M2, чей файл трогают, — это уже
существующий паттерн, а не новая связь между milestone (F-16).

## Evidence

- RED-вывод `cargo test -p stellwerk-task --test vertical`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
