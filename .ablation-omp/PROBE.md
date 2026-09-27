# OMP PROBE — установка, изоляция, совместимость провайдера

Дата: 2026-09-27. Цель шага 2.0: выяснить, возможен ли spike вообще, и как OMP
соотносится с Pi. Метод — чтение поставляемого кода и документации пакета;
реальные credential-файлы (`auth.json`, `models.json` в `~/.pi/agent/`) не открывались,
значения ключей не выводились и не попадали в отчёт.

## 2.0.1 Установка (изолированная, не глобальная)

```bash
mkdir -p .ablation-omp/omp-install
cd .ablation-omp/omp-install
printf '{"name":"omp-ablation-probe","private":true,"version":"0.0.0"}\n' > package.json
bun add oh-my-pi
```

| Параметр | Значение |
|---|---|
| Пакет | `oh-my-pi@0.2.0` |
| Репозиторий | `git+https://github.com/acidsugarx/oh-my-pi.git` |
| Self-description | «Enhancement framework for Pi CLI coding agent» |
| Peer dependency | `@earendil-works/pi-coding-agent` (в дереве `0.87.1`) |
| Прямых зависимостей | 1 |
| Установлено пакетов | 120 |
| Размер | **154M** (из них `@earendil-works` 42M, `openai` 17M, `@anthropic-ai` 14M, `@google` 11M, `@esbuild` 11M) |
| Postinstall | 2 заблокированы bun: `@google/genai` (preinstall = `echo no-op`) и `protobufjs` (нужен только gRPC) — на работу не влияют |
| Инструменты | bun 1.4.0, npm 12.0.2, node v22.23.2, pnpm 11.3.0 |

**Замечание о попытке:** `bun install --prefix .ablation-omp/omp-install oh-my-pi`
проигнорировал `--prefix` и попытался править `/home/izyabretatel/package.json`
(EACCES). Корректный способ для bun — создать `package.json` в целевой директории
и выполнить `bun add` внутри неё.

## 2.0.2 Ключевой факт: OMP — не движок, а расширение Pi

Это переопределяет формулировку фазы 2. OMP не является альтернативным исполнителем:

- `bin`: единственный бинарник `oh-my-pi` → `bin/oh-my-pi.js`;
- `package.json`: `dependencies` = 1, `peerDependencies` = `@earendil-works/pi-coding-agent`;
- официальный способ установки — `pi install oh-my-pi`, то есть Pi сам подхватывает пакет;
- `dist/` — **150K** TypeScript: `orchestrator/`, `agents/`, `skills/`, `features/`, `config/`;
- README: «replaces Pi's default system prompt with a Sisyphus-style orchestrator prompt»,
  «Built-in specialist agents (oracle, librarian, explore)», «Skill system»,
  «Graceful degradation — falls back to vanilla Pi on error».

То есть OMP добавляет **оркестраторный системный промпт + субагентов + скиллы**,
а вызов модели, инструменты, учёт usage, сессии и транспорт остаются от Pi.

Следствие для проекта: вопрос «заменяем ли Pi на OMP» некорректен. Корректные варианты —
Pi без OMP (сегодня), Pi + OMP-слой, или HYBRID с выборочным включением.

## 2.0.3 Изоляция профиля

| Механизм | Значение | Источник |
|---|---|---|
| `PI_CODING_AGENT_DIR` | каталог агента: `settings.json`, `models.json`, `auth.json`, `SYSTEM.md`, `APPEND_SYSTEM.md` | `docs/configuration.md` |
| `PI_CODING_AGENT_SESSION_DIR` | каталог сессий | `docs/environment-variables.md` |
| `--session-dir <dir>` | каталог сессий для конкретного запуска | `pi --help` |
| `--no-session` | эфемерный запуск, без записи сессии | `pi --help` |
| `--extension <path>` / `-e` | загрузить файл расширения | `pi --help` |
| `--no-extensions` | отключить автопоиск расширений | `pi --help` |
| `--offline` / `PI_OFFLINE=1` | запрет сетевых операций на старте | `pi --help` |

Аналог `PI_CODING_AGENT_DIR` — «профиль», изоляция достигается подменой этой переменной.
**Ключевое ограничение:** каталог агента отвечает и за `models.json`/`auth.json`,
и за `SYSTEM.md`. Полностью изолированный каталог = отсутствие credentials для
neuraldeep. Поэтому в spike используется гибрид: реальный `PI_CODING_AGENT_DIR`
(read-only, только чтение конфига провайдера) + отдельный `PI_CODING_AGENT_SESSION_DIR`
внутри `.ablation-omp/`. Глобальная установка OMP не выполнялась.

## 2.0.4 neuraldeep через custom OpenAI-compatible — СОВМЕСТИМ

Формат подтверждён по `docs/models.md` и `docs/custom-provider.md`:

```json
{ "providers": { "ollama": {
    "baseUrl": "http://localhost:11434/v1",
    "api": "openai-completions",
    "apiKey": "ollama",
    "models": [ { "id": "qwen2.5-coder:7b" } ] } } }
```

- `baseUrl`, `api`, `apiKey`, `models[]` — ровно те поля, что предполагал бриф;
- `api: "openai-completions"` — OpenAI-совместимый протокол;
- ключ разрешается из литерала, `$NAME`/`${NAME}` (переменная окружения) или `!command`;
- порядок разрешения ключа: runtime `--api-key` → `auth.json` → `apiKey` в `models.json` →
  переменные окружения → ambient credentials;
- `pi --list-models [search]` — проверка без запуска агента.

**Вывод: блокирующего фактора нет, spike возможен.** Дополнительно: neuraldeep уже
работает в проде (текущий воркер), то есть совместимость Pi с провайдером доказана
эксплуатацией, а не только документацией.

## 2.0.5 Метрики

`pi --help`: `--mode <mode>` — `text` (по умолчанию), `json`, `rpc`.
В `json`-режиме события идут в stdout, включая usage; это тот же режим, который уже
использует `stellwerk-dispatch` (`--mode json`) для подсчёта `context_input`.

Дополнительно поставляются `docs/usage.md`, `docs/json.md`, `docs/rpc.md`,
`docs/rpc-commands.md`, `docs/session-format.md` — извлечение метрик документировано,
включая offline-кэш каталога моделей.

## 2.0.6 Task isolation и наличие `.git`

| Возможность | Где искали | Результат |
|---|---|---|
| `task.isolation.enabled` | OMP `dist/config/schema.d.ts`, `dist/**` | **не найдено** — OMP не имеет понятия task-изоляции |
| sandbox / worktree в OMP | `dist/**` | совпадений нет (единственное вхождение слова — в `dist/extension.js`, без реализации изоляции) |
| Флаг изоляции у Pi | `pi --help` (полный) | **не найдено**: есть `--approve/--no-approve` (доверие проектным файлам), но нет `--isolation`/`--worktree` |
| Песочница в проекте | `scripts/worker/sandbox.sh` | есть, и она **наша**: `git archive` в `/tmp` без `.git` |

Вывод: ни OMP, ни Pi не дают изоляции задачи из коробки. Единственная изоляция в
проекте — наша (`scripts/worker/`). Это не аргумент за OMP и не против: изоляция
принадлежит dispatch-слою по архитектуре (см. `docs/PIVOT.md`).

## 2.0.7 Защита от repetition-loop

- OMP: собственной защиты нет. В схеме — `orchestrator.agentName`,
  `orchestrator.promptTemplate` (`sisyphus|minimal`), `agents{...}`, `skills`; полей
  вроде `loopGuard`/`maxRepeats` нет.
- Pi: защиты от повторяющихся tool-call в списке опций нет.
- Наш слой: `scripts/worker/loop_guard.py` (`--consec-limit`, `--total-limit`, наблюдение
  за PID) и `scripts/worker/validity_gate.py` (`--pi-exit`, `--timeout`). Защита
  реализована в dispatch, а не в агенте — сознательно: она должна работать одинаково
  для любого движка.

## 2.0.8 Конфигурация OMP

Файл проекта `.oh-my-pi.jsonc` (JSONC). Минимальный:

```jsonc
{ "orchestrator": { "agentName": "oh-my-pi", "promptTemplate": "sisyphus" },
  "agents": { "oracle": { "description": "...", "model": "..." } } }
```

Навыки: `.oh-my-pi/skills/<name>.md` с frontmatter `name`/`description`; тело файла
инжектится в оркестраторный промпт целиком. Диагностика: `/oh-my-pi doctor`.

## Итог PROBE

| Вопрос брифа | Ответ |
|---|---|
| Установка возможна? | Да, изолированно, 154M, 2 harmless postinstall |
| Изоляция профиля | `PI_CODING_AGENT_DIR` + `PI_CODING_AGENT_SESSION_DIR`; глобальная установка не нужна |
| neuraldeep совместим? | Да — `models.json`, `api: openai-completions`, ключ через `$NAME` |
| Метрики доступны? | Да — `--mode json` (usage в stdout) |
| task.isolation.enabled? | Отсутствует и в OMP, и в Pi |
| `.git` в изоляции | Наша песочница `git archive` — без `.git`; в OMP/Pi изоляции нет |
| Repetition-loop защита | В OMP/Pi нет; есть только в `scripts/worker/` |
| Размер зависимости | 154M (из них 42M — сам Pi) |

**Spike возможен. Раннего выхода (KEEP) по инфраструктуре не требуется.**
