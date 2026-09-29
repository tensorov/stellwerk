# packet-12b
plan_id: M3
goal: GREEN: вопрос оператору и его истечение, сбой инструмента, усечение вывода по бюджету.
files_allowed: [crates/stellwerk-gateway/src/session.rs, crates/stellwerk-gateway/tests/session_failure.rs, crates/stellwerk-gateway/tests/session_contract.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test session_failure --test session_contract
acceptance: `cargo test -p stellwerk-gateway --test session_failure --test session_contract` → exit 0. `cargo test -p stellwerk-core` и `-p stellwerk-store` не меняются.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки, выдуманный default при отсутствии данных, цикл прогона в gateway]
depends_on: [12a, 11c]
data_class: public
lane: m3/session-question-green

## Контекст

- Это GREEN-половина 11c плюс доведение до зелёного контрактных сценариев 11a.
  11c написал RED в `tests/session_failure.rs`; 11a оставил
  `tests/session_contract.rs` (199 строк, 5 сценариев: чек внутри ошибки, истечение
  не есть отказ, круг через inbox, отсутствие субагентов, достижимость шва). Твоя
  задача — довести `handle` до состояния, в котором зелёные **эти** тесты.
- 12a уже реализовал линейный проход: монтаж → `decide` → вызов → shaping → чек,
  и оставил тебе три именованные заглушки:
  - `M3/packet-12b: Ask в неинтерактивном Run`
  - `M3/packet-12b: сбой инструмента не теряет чек`
  - `M3/packet-12b: вывод не поместился в бюджет`
  Именно они — твоя работа. Список заглушек проверяется: молчаливую оставленную
  заглушку вскроет только чужой тест.
- Поверхность, которую ты заполняешь, уже объявлена и менять её нельзя:
  - `Verdict::{Allow, Ask, Suspended, Deny { reason }}`;
  - `SessionError::{Tool { source, receipt: Box<Receipt> }, ApprovalTimeout}`;
  - `CallOutcome { verdict, shaped: Option<Shaped>, receipt: Option<Receipt> }`;
  - `ApprovalRequest { run_id, tool, risk, args_hash, requested_at, expires_at }`.
- `Session` несёт `root: PathBuf` — корень рабочего места, откуда пишется
  `.stellwerk/inbox/<run_id>.json`. Без него вопросу некуда лечь, и «записали» не
  отличить от «не записали». `root` уже есть именно для этого пакета.
- Ключи и секреты не читать и не логировать: `auth.json`, `models.json`,
  `env | grep KEY` запрещены (`AGENTS.md`).

## Что сделать

1. **`Ask` в неинтерактивном Run — это `Verdict::Suspended`, а НЕ `Deny`** (В4).
   Раньше черновик пакета предписывал превращать нерешённый вопрос в отказ с
   причиной `ask_unresolved`. Это прямо противоречило RED-тесту 11a, который
   требовал `Suspended` и inbox: такой тест падал бы на своей же реализации.

   ```
   Ask в неинтерактивном Run:
     → Verdict::Suspended
     → ApprovalRequest { run_id, tool, risk, args_hash, requested_at, expires_at }
     → запись в .stellwerk/inbox/<run_id>.json
     → receipt: None, shaped: None  (вызова не было — и это состояние, а не «не посчитали»)
   ```
   Причина непустая обязательна в каждом `Deny`, но здесь `Deny` не участвует.

2. **Таймаут 24ч, для R4 — 4ч.** `expires_at` обязан пережить круг через inbox без
   искажения: `args_hash` и `expires_at` читаются обратно человеком, и
   `Unclassified` обязан остаться словом `unclassified`, а не превратиться в `R0`
   по дороге (провод `risk_wire` уже это делает — не обходи его).

3. **`R5` не suspend'ится никогда:** `Deny` немедленно, с непустой причиной, **без
   записи в inbox**. Задавать вопрос по тому, что спрашивать нельзя, — значит
   ждать ответа, которого не будет.

4. **Истечение окна — `SessionError::ApprovalTimeout`, а НЕ `Verdict::Deny`.**
   На вопрос не ответили, но никто не отказывал; в расследовании это разные вещи,
   и сливать их нельзя. Контрактный сценарий 11a называется
   `an_expired_question_is_not_a_policy_denial` — он требует ровно этого, и он
   зелёный после 12a, а не раньше. Слово «`Deny`» в черновике пакета было ошибкой
   формулировки: `Deny` здесь означал бы «политика запретила», а истекло окно.

5. **Сбой инструмента не теряет чек.** Порядок обязателен: сначала сформируй
   `Receipt` с хешами того, что успело уйти, потом верни
   `SessionError::Tool { source, receipt: Box<Receipt> }`. При
   `Result<CallOutcome, _>` чек иначе исчез бы, а вызов без следа неотличим от
   вызова, которого не было. `Box<Receipt>`, а не `Receipt`: в коробке — чек на
   208 байт против пустого `ApprovalTimeout`, и clippy справедливо требует
   индирекции. `source` — чужой текст дословно, без разбора причин: опись того,
   как чужой клиент падает, стала бы вторым источником истины о нём самом.

6. **Вывод не поместился в бюджет — усечение, а не отказ.** `Shaped.text` несёт
   начало вывода с маркером обреза, `truncated = true`,
   `artifact_path = Some(..)`, а `sha256` и `Receipt.output_hash` — по **полному**
   телу. `artifact_path` — `None` при `truncated = false`: путь на несуществующий
   файл — тихий сбой, который вскроется через месяц. Артефакт пишется рядом с
   `root`; содержимое артефакта — это вывод чужого инструмента, и в чек оно не
   идёт, только хеш.

7. Ни один путь в шейпинге и в чеке не печатает секреты. В `Receipt` попадают
   только хеши. Это проверяется тестами 10/14 — здесь просто не сломай.

## Граница: чего в этом пакете НЕТ

Исходы ответа (`Approve` → возобновить, `Deny` → продолжить, `ApproveForTask` →
кэш на `task_id`) описывают **цикл прогона**, а не сессию. У крейта нет `tokio`, у
`CallOutcome` и `SessionError` нет поля «возобновить», и петля возобновления жила
бы здесь вторым источником истины о прогоне. Это работа тела запуска в M2-10.
Gateway отвечает за то, что он умеет: приостановить с вопросом, записать вопрос в
inbox, различить истечение и отказ, не потерять чек при сбое.

**Не заводи цикл прогона в gateway.** Если тебе кажется, что сценарий требует
возобновления, — это M2-10, заведи FINDING и не расширяй скоуп.

## Evidence

- RED: вывод падающего `cargo test -p stellwerk-gateway --test session_failure
  --test session_contract` ДО кода.
- GREEN: та же команда → exit 0.
- Бюджет по ADR-0003: src ≤200 логических строк (без пустых и `//`), каждый файл
  тестов ≤250 полных строк. Считается добавленный диф, не полный размер файла.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
