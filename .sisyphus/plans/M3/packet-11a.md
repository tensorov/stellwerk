# packet-11a
plan_id: M3
goal: RED: поверхность сессии — типы, сигнатуры и различаемые состояния отказа.
files_allowed: [crates/stellwerk-gateway/src/session.rs, crates/stellwerk-gateway/tests/session_contract.rs, crates/stellwerk-gateway/src/lib.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test session_contract
acceptance: RED: `bash -c 'cargo build -p stellwerk-gateway && cargo test -p stellwerk-gateway --test session_contract --no-run && ! cargo test -p stellwerk-gateway --test session_contract'` → exit 0.
forbidden: [cargo add, cargo install, правка существующих тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [10]
data_class: public
lane: m3/session-surface-red

## Контекст

- **Виртуальный MCP-сервер.** Gateway инструменты не выполняет: он решает, что
  можно, спрашивает, когда надо, и пишет чек. Сессия — точка, где permission,
  gate, shaping и receipt встречаются в одном проходе.
- Крейт зависит от `stellwerk-core` и `stellwerk-store` по path. Новых крейтов нет;
  `cargo add` запрещён.
- Решения принимаются ТОЛЬКО через предикаты контракта. Своих копий правил не
  заводим.
- Ключи и секреты не читать и не логировать: `auth.json`, `models.json`,
  `env | grep KEY` запрещены (`AGENTS.md`).
- **Это первая часть разбиения.** M3-11 в исходном виде составил 375 строк при
  лимите 200, то есть это были два пакета, записанные подряд. Разбито по оси
  «поверхность / поведение»: здесь типы и различаемость состояний, в 11b и 11c —
  сценарии. Waiver не выдан: ослабил бы инвариант ровно там, где ревью дороже всего.
- Наработка исходного RED сохранена в `/tmp/opencode/m3-11-split/session.rs` — это
  файл ТЕСТА (242 строки, 8 сценариев), материал для разбиения, а не основание
  продолжать. Файл исходника при сохранении был заменён одноимённым, поэтому
  поверхность пишется заново по этому пакету.

## Правило объявления модуля

`src/lib.rs` добавлен в `files_allowed`, и `pub mod session;` объявляется **в этом
пакете**. Тест берёт типы публичным API: `#[path]` собрал бы вторую копию модуля и
оставил экспорт непроверенным (F-14). Требование, висящее на пакете ПОСЛЕ
зависящего от него RED, делает RED несуществующим.

## Что сделать

1. `src/session.rs`, объявления без поведения:
   - `Session` — `id`, `plan: MountPlan`, `caller: PolicyLevel`, `budget_tokens`,
     `calls`, и **два поля сверх исходного списка пакета**: `subagent_depth: u32` и
     `root: PathBuf`. Первое нужно, чтобы правило В3 (`risk >= R2` → глубина
     субагентов 0) было **наблюдаемым**, а не заявленным: без поля тест нечем
     проверить. Второе нужно, чтобы вопрос «куда писать `.stellwerk/inbox/<run_id>.json`»
     имел ответ и чтобы тест В4 прочитал файл.
   - `CallOutcome` — `verdict: Verdict`, `shaped: Option<Shaped>`,
     `receipt: Option<Receipt>`.
   - `ApprovalRequest` — `run_id`, `tool`, `risk: ToolRisk`, `args_hash`,
     `requested_at`, `expires_at`. Сериализуемый: запрос пишется в inbox и читается
     обратно человеком или следующим прогоном.
   - `ToolBackend` — трейт одного метода вызова инструмента. Это шов: тест подставляет
     подделку, поэтому живой процесс в тесте не нужен.
   - `ToolError`, `SessionError` — см. пункт 2.
2. **Состояния отказа различимы, и это предмет пакета, а не деталь реализации.**
   `SessionError::Tool` несёт чек внутри себя: при сигнатуре
   `Result<CallOutcome, SessionError>` чек при ошибке иначе просто исчезает, а
   потерянный чек означает вызов без следа, то есть несостоявшийся вызов.
   `SessionError::ApprovalTimeout` — отдельный вариант: истёкший вопрос это НЕ `Deny`,
   и причина отказа обязана называться. `ToolError` — newtype над чужим текстом:
   перечень причин чужого клиента стал бы вторым источником истины о том, как он падает.
3. **Переиспользовать, а не объявлять второй раз.** `ToolRisk` уже есть в
   `permission`, `Verdict` — в `gate`, `MountPlan` — в `mount`, `Receipt` — в
   `receipt`, `Shaped` — в `shaping`, `CallRequest` — в `gate`. Реэкспортируй их и
   скажи об этом комментарием. Два `ToolRisk` в крейте сделали бы риск, посчитанный
   при монтаже, и риск, проверенный в `decide`, разными типами, и перевод между
   ними рано или поздно появился бы с `unwrap_or(R0)`.
4. Тесты в `tests/session_contract.rs` — про состояния, без сценариев вызова:
   - чек доезжает внутри `SessionError::Tool`, а не теряется;
   - `ApprovalTimeout` и `Deny` — разные вещи: разные варианты, разный смысл;
   - `args_hash` переживает сериализацию в inbox и обратно без искажения;
   - `subagent_depth` наблюдаем и равен 0 у новой сессии;
   - подделка `ToolBackend` компилируется — это проверка шва, а не поведения.
5. `handle` — **RED-заглушка**; поведение в 11b и 11c.

## Evidence

- RED-вывод `cargo test -p stellwerk-gateway --test session_contract`: тесты
  компилируются и падают на заглушке.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
