# packet-07
plan_id: M4
goal: GREEN — `Ask` в неинтерактивном прогоне даёт `TaskStatus::Suspended` и кладёт `ApprovalRequest` в `.stellwerk/inbox/<run_id>.json`.
files_allowed: [crates/stellwerk-task/src/approval.rs, crates/stellwerk-task/tests/inbox.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --test inbox
acceptance: `cargo test -p stellwerk-task --test inbox` → 0 failed. Задача в БД имеет `TaskStatus::Suspended`, а не `Failed`.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [03, M2-06]
data_class: public
lane: m4/approval

## Контекст

- **Этот пакет закрывает остаток F-11.** M3-04 доказал, что `decide` различает
  `Suspended` и `Deny` на уровне `Verdict`. Но различение обязано дойти до
  **статуса задачи в БД** — иначе «запрещено политикой» и «никто не ответил»
  неразличимы в расследовании, а это буквально то, что запрещает В4.
  До сих пор `TaskStatus::Suspended` в контракте есть, но ни один пакет не
  требовал маппинга. Требую здесь.
- `M2-10` вводит раннер-исход `AwaitingApproval` с тестом, чтобы «ждёт человека»
  не имело куда схлопнуться. Этот пакет **смыкает** его с `TaskStatus`.
- Правило В4: `Deny` не самостоятельный исход, а дефолт по таймауту.

## Что сделать

1. **`src/approval.rs` — отдельный файл, а не `runner.rs`.** Раннер и approval
   отвечают за разное: раннер решает «можно ли», approval — «кого спросить и куда
   положить вопрос». Общий файл означал бы, что пакет 05 и этот пакет не могут
   идти параллельно, а они могут.
2. `pub struct ApprovalRequest { run_id, task_id, tool, arguments, risk, answerable_by, window_hours, expires_at }`
   и `pub fn park(request: &ApprovalRequest, inbox: &Path) -> Result<(), ApprovalError>`.
3. `pub fn next_status(verdict: &Verdict, risk: RiskClass) -> TaskStatus` — маппинг
   вердикта в статус:
   - `Ask` в неинтерактивном прогоне → `TaskStatus::Suspended`, **через предикат
     `TaskStatus::can_transition_to`**, без собственного свода переходов;
   - `Deny` → `TaskStatus::Failed`;
   - `Suspended` → `TaskStatus::Suspended`;
   - `Allow` → без смены статуса.
4. `park` пишет `.stellwerk/inbox/<run_id>.json`: `run_id`, `task_id`, инструмент,
   аргументы, риск, кто может ответить (`L5Owner` из контракта), окно ответа и
   срок (`R4` — 4ч, иначе 24ч).
2. Тесты обязаны утверждать **статус в БД**, а не только код возврата:
   - `Ask` неинтерактивно → `TaskStatus::Suspended` в БД, inbox-файл существует;
   - в JSON есть `run_id`, `tool`, `risk`, `expires_at`;
   - **`R5` при `L5Owner` → `Deny`, а НЕ `Suspended`, и inbox-файла нет.**
     R5 необратимо, его нельзя ждать и нельзя делегировать: ждать человека по
     вопросу, который человек всё равно не сможет разрешить, — это ловушка;
   - `Unclassified` в неинтерактивном прогоне → `Deny`, а не `Suspended`:
     вопрос задать некому, а `Suspended` ждал бы ответа, которого не будет.
     `Suspended` допустим **только** когда вопрос действительно кому-то задаётся.
3. Путь inbox — из `STELLWERK_INBOX` с умолчанием `.stellwerk/inbox`, чтобы тесты
   не писали в рабочий каталог. Отсутствие переменной и её пустое значение —
   разные случаи: пустое значение должно давать ошибку, а не молчаливый
   умолчательный путь.

## Evidence

- `cargo test -p stellwerk-task --test inbox` → 0 failed.
- clippy `-D warnings` и `fmt --check` чисто.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
