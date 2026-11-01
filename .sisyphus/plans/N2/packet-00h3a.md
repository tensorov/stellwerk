# N2-00h3a — Build разрешён или назван отказом: происхождение, версия, снимок

**label:** — · **mutation_gate:** no · **depends_on:** [N2-00h2b, 00h2b]

## Почему пакет разбит

00h3 в заявленном виде («Pi в bwrap + deny-гейт + `--mode json` + приём `planner == host`») не
влезает в C2: сборка воркера, копия дерева, два канала, гейт и приёмка — это три разных
предмета с тремя разными наборами тестов. Поэтому:

- **00h3a (этот пакет)** — чистое решение «можно ли исполнять этот план» и отказы,
  различимые в статусе. Ни сети, ни песочницы, ни процессов.
- **00h3b** — исполнение: копия дерева без `.git`, `SandboxWorker`, `pi` с `--mode json`,
  `ModelProxy` + `GateChannel`, один ход, запись факта.
- **00h3c** — живая приёмка на одной небольшой задаче и одном запуске (N2-00a).

Пока 00h3b не сделан, нажатие `b` на утверждённом плане честно говорит, что исполнения ещё
нет, — но **отказы по существу уже настоящие**: неутверждённый план, чужой снимок, план,
сделанный мимо транспорта, отвергаются здесь.

## Цель

Один вопрос, на который пакет даёт ответ вместо догадки: **можно ли этот план исполнять?**

`BuildRequest` собирается только из утверждённого плана, чьё происхождение записано как
`host` и чей снимок совпадает с текущим. Всё остальное — `BuildRefusal` с отдельным вариантом
на каждую причину: «никто не утверждал», «план правили после утверждения», «репозиторий
сдвинулся», «происхождение не записано», «план сделан вне транспорта» — это пять разных
ответов, и статус не имеет права их сливать (закон 6).

## files_allowed

- `crates/stellwerk-cli/src/plan.rs`
- `crates/stellwerk-cli/tests/plan.rs`

## Что делать

1. `pub struct BuildRequest { goal, allowed_changes, acceptance, checks }` — поля закрыты,
   конструктор один (`build_request`), поэтому запрос нельзя собрать в обход проверок.
   Отдельного поля `planner` нет: допуск `host` доказывается ветвью `Some("host")` в
   конструкторе, а константа, которая может иметь лишь одно значение, была бы украшением.
2. `pub fn build_request(session: &PlanSession, current_snapshot: &str) -> Result<BuildRequest, BuildRefusal>`
   — **чистая**, без `git`, без диска: снимок приходит аргументом, поэтому проверяется без
   репозитория. Порядок проверок фиксирован и назван в коде:
   1. `draft.is_none()` → `NoPlan`;
   2. `approval_required()` → `Unapproved { version, approved }`; если `approved_version ==
      Some(version)`, но дайджест не совпал → `DigestChanged` (текст правили в обход счётчика);
   3. `session.snapshot != current_snapshot` → `SnapshotDrift { planned, current }`
      (план утверждён для другого состояния дерева — это не «мелкое расхождение»);
   4. `planner`: `None` → `PlannerNotRecorded`; `Some("opencode")` →
      `PlannerBypassedTransport { recorded }`; `Some(other)` → `PlannerUnknown { recorded }`.
3. `impl fmt::Display for BuildRefusal` — по строке на вариант, без общих слов: человек
   должен понять, что именно починить (утвердить версию N, вернуться к снимку, перезапустить
   с `--planner host`).
4. TUI, `Stage::Approved`, клавиша `b`:
   - `build_request` вернул `Ok(request)` → показать сводку (`цель`, число изменений,
     проверки) и строку «исполнение: появится в N2-00h3b»;
   - `Err(refusal)` → показать `Display` отказа, а не общий текст.
   `BUILD_UNAVAILABLE_REASON` остаётся в `render_plan` как есть: экран не обещает того, чего
   нет. Её текст меняется не здесь, а в 00h3b, когда канал действительно появится.
5. `git_head(repo)` вызывается в обработчике, чтобы получить `current_snapshot`; сама
   `build_request` про git не знает.

## Тесты (`tests/plan.rs`), 7 новых

- `build_requires_an_approved_plan` — плана нет → `NoPlan`; черновик без утверждения →
  `Unapproved { version: 1, approved: None }`.
- `an_edit_after_approval_invalidates_the_request` — `approve()`, затем `set_draft` →
  `Unapproved { version: 2, approved: Some(1) }`.
- `a_text_edit_without_a_version_bump_is_caught_by_the_digest` — `approve()`, затем правка
  `draft.checks` напрямую (версия та же) → `DigestChanged { version }`: дайджест ловит то, что
  счётчик версий пропустил.
- `a_plan_approved_for_another_snapshot_is_refused` — утверждён на `abc`, спрашиваем про
  `def` → `SnapshotDrift { planned, current }`; оба значения в тексте отказа видны.
- `an_unrecorded_planner_is_not_host` — `planner: None` → `PlannerNotRecorded`, и это **не**
  `PlannerBypassedTransport`.
- `opencode_is_named_as_bypassing_the_transport` — `planner: Some("opencode")` →
  `PlannerBypassedTransport`; `Some("турбо")` → `PlannerUnknown`.
- `an_approved_host_plan_becomes_a_request` — утверждённый план с `planner: Some("host")` и
  совпадающим снимком даёт запрос с той же целью, тем же набором проверок и тем же набором
  допустимых изменений.

## Приёмка

```
cargo test -p stellwerk-cli --test plan
cargo test -p stellwerk-cli
cargo clippy -p stellwerk-cli --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-cli -- --check
```

C2: `src` ≤ 200 логических строк, `tests` ≤ 250, 2 файла.

## Запрещено

Запускать процессы, монтировать песочницу, ходить в сеть, писать `build.json` (это 00h3b);
считать пустой `planner` равным `host`; «нормализовать» расхождение снимка; менять текст
`BUILD_UNAVAILABLE_REASON`; трогать `host_model.rs`, `planner_host.rs`,
`stellwerk-task/src/cli.rs`.
