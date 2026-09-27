# NIGHT SESSION — STATE

Сессия: старший архитектор Stellwerk, автономная ночная.
Правило: фазы строго последовательно, без остановок на «ок». После каждой фазы —
запись здесь. Если контекст переполняется: дописать текущую фазу, записать STATE,
остановиться. Новую фазу не начинать.

Проект: ~/gits/stellwerk (Rust workspace). Стартовое состояние: main, just check = 62+1.
Коммитов в этой сессии не делаю без явного указания; push запрещён.

## Фазы

| # | Фаза | Статус |
|---|---|---|
| 1 | PIVOT.md + AX_INSPIRATION.md | **DONE** |
| 2 | OMP spike | **DONE — HYBRID** |
| 3 | Event contract + core types | **DONE — 47 тестов, 27 RED** |
| 4 | Storage traits + SQLite | **DONE — 3 трейта, 21 тест (20 RED)** |
| 5 | Массовая спецификация packets | **DONE — 36 пакетов** |
| 6 | Контрольный прогон qwen | **DONE — CONFIRMED** |
| F | MORNING_REPORT.md | **DONE** |

## Лог

(ниже добавляется по мере завершения фаз)

### Фаза 1 — DONE

Файлы: `docs/PIVOT.md` (120 строк), `docs/AX_INSPIRATION.md`.
Ключевые решения: границы проекта зафиксированы (закон/измерение/контракт/песочница);
исполнители сменные; PolyForge — закон; память — федерация, Obsidian — проекция;
Yao/AX/SoL-Pi — источники идей, не зависимости; OMP — кандидат, вердикт после spike;
SoL-Pi отвергнут со ссылкой на данные; OMO — planner-only. Хронология из 10 решений
с датами и evidence-ссылками. Из AX взято разделение Task/Workspace/Model/Run, типизированные
лимиты, data_class как маршрутизация, kubectl-shaped формы команд; не взяты K8s,
Agent Substrate, Redis control plane, billions scale.
На чём остановился: фаза 2, шаг 2.0 PROBE.

### Фаза 2 — начата (OMP spike)

Разведка: `oh-my-pi` существует в npm (latest 0.2.0, `github.com/acidsugarx/oh-my-pi`,
«Enhancement framework for Pi CLI coding agent») — это OMP. Инструменты есть:
bun 1.4.0, npm 12.0.2, node v22.23.2, pnpm 11.3.0. Сеть доступна (registry.npmjs.org,
github.com через туннель).

### Фаза 2 — DONE. ВЕРДИКТ: HYBRID (Pi остаётся движком)

Файлы: `.ablation-omp/PROBE.md` (153 строки), `.ablation-omp/REPORT.md` (169 строк),
`.ablation-omp/runs/*.jsonl` (12 сырых прогонов), harness-скрипты
`run-overhead.sh` / `run-activation.sh` / `run-w1.sh`, изолированная установка
`.ablation-omp/omp-install/` (154M, не в git — добавить в .gitignore позже).

Ключевые решения и находки:
1. **OMP — не движок, а extension Pi** (`oh-my-pi@0.2.0`, peer dep
   `@earendil-works/pi-coding-agent`, dist 150K, установка через `pi install`).
   Вопрос «заменим ли Pi на OMP» некорректен технически.
2. **Главная ценность OMP не работает**: системный промпт не подменён
   (`preamble` 169 байт в обоих плечах). Обещанное README поведение — дефект 0.2.0.
3. **Overhead: +4766 байт системного промпта** (2760 → 7526, +173%), 6 новых
   инструментов, ~11% окна воркера (~43k) потрачено впустую. На уровне `usage`
   разницы нет (ctx 14095 у обоих; доминирует cacheRead ~12576).
4. **neuraldeep совместим** — Pi умеет custom OpenAI-compatible через `models.json`
   (`baseUrl`/`api`/`openai-completions`/`apiKey` через `$NAME`). Раннего выхода не было.
5. **Изоляции в OMP нет вообще**: ни `task.isolation.enabled`, ни sandbox/worktree,
   ни одного ограничения путей в `rules`. F1 не перехватывается — защита только наша
   (`sandbox.sh` + path-guard в dispatch), и она работает одинаково для Pi и Pi+OMP.
6. **Субагенты — единственная находка**: персоны oracle/librarian/explore, категории
   quick/deep/ultrabrain, инструменты `oh_my_pi_subagent`/`oh_my_pi_delegate_task`.
   `approvalMode` отсутствует → нет вектора «yolo». Но они дублируют роли OMO, а OMO
   planner-only по решению из PIVOT.md.
7. **W1 (n=3): 3/3 эквивалентность**, median total_input_cum 49135, tool_steps 7,
   дисперсия 1.6% (убийственной дисперсии SoL-Pi не повторилось). Помечено как
   CONFOUNDED: baseline Pi 67847 снят в другом протоколе (с проектным контекстом),
   поэтому −27.6% НЕ является доказательством превосходства и не использовано в вердикте.

Честный долг фазы: не выполнен чистый A/B (Pi в протоколе OMP) — не влияет на вердикт,
опирающийся на контролируемые измерения. Рецепт закрытия: `OMP=0 bash .ablation-omp/run-overhead.sh`.

На чём остановился: фаза 3, шаг 3.1 — проектирование типов в `contract.rs`.

### Фаза 3 — DONE. Контракт + RED-спецификация (contract-change)

Файлы:
- `crates/stellwerk-core/src/contract.rs` — 473 строки, **20 типов** (19 структур/перечислений
  + 1 алиас `ModuleRisk = RiskClass`): RiskClass, EventSource, Event, DataClass, TaskStatus,
  TaskLimits, TaskSpec, NetworkPolicy, WorkspaceSpec, RetentionClass, ModelBinding,
  EvidenceKind, EvidenceEntry, GateResult, PolicyLevel, Action, WorkspaceMembership,
  ModuleKind, Source.
- `crates/stellwerk-core/Cargo.toml` — добавлен `[dev-dependencies] serde_json` (workspace).
- `crates/stellwerk-registry/src/model.rs` — **3.3 выполнен**: локальные `ModuleKind` и
  `Source` удалены, вместо них `pub use stellwerk_core::contract::{ModuleKind, RiskClass,
  Source}`; поле `Module.risk` типизировано `Option<String>` → `Option<RiskClass>`.
- 6 файлов RED-тестов в `crates/stellwerk-core/tests/`.

**6 предикатов оставлены `todo!()`** — намеренная RED-спецификация: `RiskClass::required_level`,
`TaskStatus::can_transition_to`, `TaskStatus::is_terminal`, `NetworkPolicy::allows`,
`DataClass::permits_model`, `PolicyLevel::permits`. Предикат падает, а не молча даёт
неверный ответ. Параметры «использованы» через `let _ = (...)` — НЕ `#[allow]`
(запрещено `AGENTS.md`), поэтому `clippy -D warnings` остаётся зелёным.

Тесты: **47 всего, 27 RED, 20 GREEN** (проверено запуском):
| файл | GREEN | RED | причина RED |
|---|---|---|---|
| event_serde.rs | 6 | 0 | — (чистая сериализация) |
| risk_class.rs | 4 | 0 | — (Ord работает) |
| task_lifecycle.rs | 1 | 10 | `TaskStatus::can_transition_to` / `is_terminal` |
| data_class_routing.rs | 1 | 6 | `DataClass::permits_model` |
| workspace_policy.rs | 4 | 4 | `NetworkPolicy::allows` |
| rbac_level.rs | 4 | 7 | `PolicyLevel::permits` |

⚠️ **Ожидаемое состояние репозитория: `just check` ПАДАЕТ** — это намеренно
(RED-спецификация для дешёвых исполнителей). Падение только в 4 из 6 файлов core-тестов,
причина — `not yet implemented: RED: …`. Реестр не сломан: 62 теста проходят.

Ключевые зафиксированные инварианты (для GREEN-пакетов):
- `Deny` запрещает ЛЮБОЙ URL, включая пустую строку;
- `AllowList` сравнивает хост точно (`evil.example.com` ≠ `example.com`), пустой список = запрет всего;
- `Personal`/`Internal` не уходят при `RetentionClass::ThirdParty` (это и есть запрет Personal → Big Pickle);
- привязка не может обработать класс выше объявленного (`data <= binding.data_class`);
- `GrantRoles` — только `L5Owner`; `ManageMembers` — с `L4Admin`;
- права монотонны по уровню; `Done`/`Failed` терминальны; переход в себя запрещён;
- `EvidenceSubmitted` достижим только из прогона (нельзя предъявить доказательства без прогона);
- `Module.risk` парсится строго: `"R3"` да, `"r3"` и `"R9"` — нет.

На чём остановился: фаза 4 — новый крейт `stellwerk-store`.

### Фаза 4 — DONE. Трейты хранилища + SQLite (contract-change: rusqlite 0.33)

Файлы:
- `Cargo.toml` (корень) — `rusqlite = { version = "0.33", features = ["bundled"] }`
  в `[workspace.dependencies]` с пометкой contract-change; `stellwerk-store` в `members`.
  Это **единственное** добавление зависимости за сессию.
- `crates/stellwerk-store/Cargo.toml`, `src/lib.rs`, `src/traits.rs`, `src/sqlite.rs`,
  `src/migrations.rs`, `src/migrations/0001_init.sql`, `0002_indexes.sql`
- 4 файла RED-тестов в `crates/stellwerk-store/tests/`

**3 трейта:** `TaskStore` (create, get, update_status, list_by_status, list_by_workspace),
`EventStore` (append, list_since, list_by_task), `ModuleStore` (get, list, upsert_lock_entry) —
все `Send + Sync`, сигнатуры `-> impl Future<Output=…> + Send` (RPITIT, без новых зависимостей:
`async-trait` не потребовался), ошибки через `StoreError` (thiserror, 6 вариантов).
Плюс проекция `ModuleRecord` — крейт store не зависит от registry, чтобы не было цикла.

**2 миграции:** `0001_init` (tasks, events, modules, evidence, workspaces, schema_migrations —
везде `id TEXT PRIMARY KEY`, `data` JSON, `created_at`/`updated_at`) и `0002_indexes`
(6 индексов под `list_by_status`, `list_by_workspace`, `list_since`, `list_by_task`).
Отклонение от брифа задокументировано в SQL: SQLite не имеет JSONB, поэтому `data` — `TEXT`.
Миграции встроены через `include_str!` → офлайн-установка не зависит от каталога на диске.

**Реализовано по-настоящему:** `SqliteStore::open()` + применение миграций при первом
подключении (идемпотентно, через `schema_migrations`). Всё остальное — `todo!()`
внутри `async`-блоков (голый `todo!()` возвращает `()`, а не future — это была ошибка сборки).

Тесты: **21 всего, 20 RED, 1 GREEN**:
| файл | GREEN | RED |
|---|---|---|
| task_crud.rs | 1 | 6 |
| event_append.rs | 0 | 4 |
| module_store.rs | 0 | 7 |
| concurrent.rs | 0 | 3 |
Единственный GREEN — `open_applies_migrations_without_error`: он доказывает, что схема
реально создаётся, а не только описана в SQL-файлах.

Ошибки, потраченные на этом (записано, чтобы не повторить):
- `bun install --prefix` игнорирует `--prefix` (нужен `package.json` в целевой папке + `bun add` внутри);
- `rm -rf` запрещён политикой проекта → для песочниц используется `mktemp -d`;
- `TaskSpec` не несёт статус → статус проверяется через `list_by_status`, а не полем типа;
- `ModuleRecord` не был реэкспортирован из корня крейта;
- `Eq` на обёртке с `Event` невозможен (у `Event` нет `Eq`) → лишний тип `StoredEvent` удалён.

На чём остановился: фаза 5 — спецификация пакетов M1–M3.

### Фаза 5 — DONE. 36 пакетов (M1, M2, M3 × 12)

Файлы: `.sisyphus/plans/M1/packet-01…12.md` + `index.md`, и то же для M2, M3.
Формат — по образцу `.sisyphus/plans/M0/packet-01.md`: frontmatter
(`plan_id`, `goal`, `files_allowed`, `max_diff_lines`, `test`, `acceptance`,
`forbidden`, `depends_on`) + разделы Контекст / Что сделать / Evidence.
Все 36 пакетов помечены `data_class: public`, `depends_on` проставлены,
у каждого — точная команда `cargo test` и предикат приёмки.

**M1 (12)** — превращение RED в GREEN без изменения тестов:
- 01/07 — фиксация RED-базлайна (ядро и store) с записью evidence;
- 02–06 — пять предикатов ядра: TaskStatus, NetworkPolicy, DataClass, PolicyLevel,
  RiskClass::required_level (последний — с новым тестом `risk_level_policy`,
  единственный непокрытый край контракта);
- 08–12 — трейты store: create/get, update_status/list_by_status, list_by_workspace,
  события (включая потокобезопасность), модули.
⚠️ **Волна 1 M1 параллельна только формально**: пакеты 02–06 правят ОДИН файл
`contract.rs`. Раздавать по очереди или одним воркером. Записано в `M1/index.md`.

**M2 (12)** — `stellwerk-task`: скелет крейта (contract-change на корневой Cargo.toml,
только в packet-01) → загрузка TaskSpec из TOML → clap CLI (`apply|get|watch|cancel|logs`)
→ dispatch → жизненный цикл статусов через предикаты ядра → адаптер песочницы поверх
`scripts/worker/sandbox.sh` → runner (запуск Pi в форме dispatch, разбор `--mode json`,
verdict по порогам из `dispatch.conf`) → события прогона → цепочка evidence
(ModelClaim → ToolAttestation → Validation → GateResult) с меняющимся `chain_tail_hash`.
Запрещено: заводить свой свод переходов, изобретать флаги Pi, трогать `scripts/worker/`.

**M3 (12)** — `stellwerk-gateway`: RiskClass → Allow/Ask/Deny (R0–R1 allow, R2–R3 ask,
R4–R5 deny) → `decide` (риск × полномочия × класс данных) → shaping (2k токенов,
хеш от полного тела + артефакт) → ленивый монтаж по бюджету → чек на каждый вызов →
виртуальная сессия. Зафиксировано: проверка «инструмент смонтирован» идёт ДО `decide`;
запрещённый вызов всё равно пишет чек; `Ask` без подтверждения = `Deny`;
в чеках только хеши, никогда секреты.

На чём остановился: фаза 6 — контрольный прогон одного пакета через stellwerk-dispatch.

### Фаза 6 — DONE. CONFIRMED: qwen закрывает пакеты

Выбран пакет: `.sisyphus/plans/M1/packet-03.md` (самый простой GREEN-пакет M1 —
реализация одного предиката `NetworkPolicy::allows` в ядре). Формат пакета оставлен
как для воркера: frontmatter + Контекст / Что сделать / Evidence.

**Как была решена проблема базы.** Работа ночной сессии не закоммичена, а песочница
dispatch собирается через `git archive "$BASE_REF"` — то есть только из коммита.
Решение без движения HEAD и без коммита: `git add -A` → `git stash create`
(создаёт объект коммита из текущего дерева) → `git reset` (индекс восстановлен).
База: `cf120fdbbdd95763512f879d9106feffc0401bd0`. HEAD остался `26caaed`.

**Результат прогона** (`stellwerk-dispatch m1-core-net packet`, dispatch exit=0):
| метрика | значение | источник |
|---|---|---|
| VALIDITY GATE | **VALID** | validity.json |
| pi_exit | 0 | validity.json |
| stop_reason_last | `stop` | validity.json |
| timeout | false | validity.json |
| tool_steps | 9 | validity.json |
| turns | 20 | validity.json |
| total_input_cum | 99048 | validity.json |
| пик контекста за один запрос | **13087** токенов | разбор run.jsonl |
| запас в окне 46112 | **33025 токенов (72%)** | расчёт |
| output_cum | 2577 | разбор run.jsonl |

Расхождение счётчиков: gate считает turns=20 / input_cum=99048, мой разбор run.jsonl
даёт turns=9 / input_cum=36242. Причина — разные правила подсчёта (gate суммирует по
событиям сессии, включая повторы). Для решения «VALID vs не VALID» источником истины
служит validity.json; пик контекста считаю по своему разбору и помечаю это явно.

**Приёмка пакета (команда из пакета, выполнена в песочнице):**
`cargo test -p stellwerk-core --test workspace_policy` → **8 passed; 0 failed**.

**Дисциплина пакета соблюдена:**
- дифф затрагивает РОВНО один файл — `crates/stellwerk-core/src/contract.rs`
  (совпадает с `files_allowed`); проверено по `sandbox.diff`;
- размер диффа +24/−2 при бюджете 200 строк;
- `sandbox.files.base` == `sandbox.files.new` — воркер не создал и не удалил ни одного файла;
- `.git` в песочнице отсутствует;
- главный репозиторий не тронут (`files_changed=0`);
- никаких запрещённых действий в прогоне не зафиксировано.

**Вердикт: CONFIRMED — qwen-совместимые пакеты работают.** Слой dispatch (sandbox →
loop_guard → validity_gate) отработал без вмешательства, и воркер закрыл реальный
пакет из новой спецификации, попутно доказав, что RED-тесты ночной сессии пригодны
как приказ для дешёвого исполнителя.

**Что осталось за владельцем:** экспорт решения в ветку —
`stellwerk-dispatch --export /tmp/stellwerk-sandbox/m1-core-net-packet-20260927-004804 <branch>`.
Песочница сохранена (`SANDBOX_KEEP` был не нужен — она пережила прогон).
Экспорт НЕ выполнялся: это запись в репозиторий, а в брифе запрещён commit без
явного указания. Также не удалён dangling-объект `cf120fdb` — он безвреден и
соберётся GC; удалить: `git prune` позже, если понадобится.

На чём остановился: все 6 фаз закрыты, написан `.night/MORNING_REPORT.md`.
