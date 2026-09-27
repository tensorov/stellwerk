# PIVOT — границы проекта Stellwerk

Дата: 2026-09-26. Статус: канонический документ границ. Пересматривается только
осознанным решением владельца, не по мере появления новых инструментов.

Проект: control plane для агентной работы. Философия: **федерация, не государство**.
Модель вдохновлена немецким правовым государством: субъекты (агенты, люди, сервисы)
сохраняют идентичность и autonomy, а центр задаёт только закон — процедуры, доказательства
и границы полномочий. Stellwerk не сливает исполнителей в одно целое; он делает их
проверяемыми и заменяемыми.

## Что Stellwerk ЕСТЬ

| Слой | Содержание | Где живёт |
|---|---|---|
| **Закон (Amt)** | PolyForge ledger: ModelClaim → ToolAttestation → Validation → Gate. Единственный источник истины о «сделано» | `polyforge-mcp`, evidence-цепочки |
| **Измерение** | Сколько контекста стоит модуль: байты JSON-схем × калибровка, запись в `modules.lock` | `crates/stellwerk-registry/src/cmd_measure.rs` |
| **Реестр модулей** | `modules.toml` / `modules.lock`: источники (git/npm/cargo), риск, бюджет, политика обновления | `crates/stellwerk-registry` |
| **Контракт оркестрации** | Task / Workspace / Model / Run — типизированные сущности и переходы, общие для любого движка | `crates/stellwerk-core/src/contract.rs` |
| **Политика песочницы** | Как исполняется пакет: изоляция путей, loop-guard, validity gate, явный экспорт | `scripts/worker/`, `~/.local/bin/stellwerk-dispatch` |
| **Планирование** | OMO: Prometheus (план) → Metis (разбор) → Momus (критика) → Atlas (исполнение) | `.omo/omo.jsonc` |

## Что Stellwerk НЕ ЕСТЬ

- **Не ядро конкретного TUI-агента.** Исполнители сменные: сегодня Pi 0.87.0,
  кандидаты — OMP, OMP-подобные Enhancement-фреймворки, позже OMP/OMP. Любой движок,
  умеющий отдать JSON-события и tool-call'ы, подключается без правок ядра.
- **Не чужой control plane.** Kubernetes, Docker Swarm, Nomad, Temporal — не
  зависимости и не основа. Если завтра понадобится удалённый исполнитель, это будет
  наш собственный тонкий слой, а не «обёртка над K8s».
- **Не чужой UI.** Board / Inbox / Calendar / CRM — это проекции, а не ядро. Ядро не
  зависит ни от одного из них.
- **Не монолит и не оркестратор ради оркестрации.** Если очередь пуста — система должна
  тихо стоять, а не изобретать работу.

## Исполнители сменные. PolyForge — закон. Память — федерация

- **Исполнители сменные.** Движок — деталь реализации, а не архитектуры. Контракт
  задаёт, что движок обязан вернуть (события, usage, tool-call'ы, хвост ответа), и всё.
- **PolyForge — закон.** «Готово» = ToolAttestation от PolyForge, не текст модели.
  Это не договорённость, а инвариант: без цепочки ModelClaim → Verified → Gate PASS
  задача не считается закрытой ни в человеческом, ни в машинном чтении.
- **Память — федерация.** Typed stores с четкой принадлежностью (задаче, воркеру,
  проекту). Obsidian — проекция для человека, а не источник истины. Каждая проекция
  может быть пересобрана; ни одна не является первичной.

## Источники идей (не зависимости)

| Источник | Что берём | Что НЕ берём |
|---|---|---|
| **Yao Agents** | Идеи UI/слоёв: Board, Inbox, Robots, Tai Link | Зависимость, код, runtime |
| **AX (google/ax)** | Контракт Task/Workspace/Model, формы `apply/watch/suspend/resume`, идея sandbox-as-первокласс | K8s, Agent Substrate, Redis control plane, billions scale |
| **SoL-Pi (NVlabs)** | Методику измерения (A/B на реальном пакете, гейт активации) | Сам SoL-Pi — отвергнут, см. ниже |
| **OMO (oh-my-openagent)** | Роли планирования и ревью, модельная цепочка | Как рантайм для воркеров |

## Решения по конкретным технологиям

### OMP — кандидат в движок, решение после spike

Не принят и не отвергнут. Spike в `.ablation-omp/` сравнивает OMP с Pi по четырём
осям: overhead, эффективность на ограниченной задаче, изоляция, субагенты. Вердикт
пишется в `.ablation-omp/REPORT.md`. Пока вердикта нет — Pi 0.87.0 остаётся движком.

### SoL-Pi — отвергнут (NO_ADOPT)

Данные: `.ablation-solpi/REPORT.md` (commit `a3a2487`).
- W1 (bounded, n=3 на плечо): `total_input_cum` −17.3%, Action Fusion срабатывал
  (1/5/4 против 0/0/0 в baseline), эквивалентность полная → **PASS**.
- W2 (реальный packet-07a, n=3): `success_rate` 1/3 против 2/3 → **NO_ADOPT**.
- Дисперсия treatment-арма ×15 (один прогон 47 шагов / 663 656 токенов при `af_fired=1`).
- ObservationPack не компенсирует блуждание на длинных пакетах.
Вывод: выигрыш есть только там, где модель уже ведёт себя; на реальной работе
SoL-Pi хуже. Ценность эксперимента — не в SoL-Pi, а в методике и в трёх извлечённых
защитах (sandbox, loop-guard, validity gate), которые перенесены в воркер-слой.

### OMO — только planner

Никогда не рабочий агент: OMO-надбавка к контексту (строки C/D/E в
`docs/CONTEXT_BUDGET.md`) съедает окно воркера. Планирование требует ≥128k контекста,
поэтому OMO живёт на space-bunny / mimo / big-pickle, а воркер работает на qwen с
окном 46 112.

### Модельная цепочка

`.omo/omo.jsonc`:
- обычные агенты и категории: `space-bunny → mimo → big-pickle`;
- Momus / Metus (reviewers): `mimo → space-bunny → big-pickle`;
- Big Pickle — только public-данные (retention policy): старший fallback, который
  никогда не должен видеть internal/personal.

## Инфраструктурные решения (приняты)

- **Offline-first обязателен.** Интернет часто выключают. Всё, что не работает без
  сети, должно иметь локальный путь; сеть — ускорение, не условие.
- **ThinkPad** — основная машина; два сервера в другой квартире (homelab: Traefik +
  Authentik SSO, ansible, `github.com/tensorov/homelab`).
- **Синхронизация:** Syncthing для файлов, Litestream для SQLite-бэкапов.
- **RBAC:** Authentik + Casbin + Postgres RLS на сервере; локально — SQLite.
- **События:** NATS JetStream на сервере, leaf на ноутбуке, outbox/inbox на границе.

## Хронология решений

| Дата | Решение | Evidence |
|---|---|---|
| 2026-09-11 | Базовый workspace: `stellwerk-core` (contract), `stellwerk-registry` | репозиторий, `docs/M0.md` |
| 2026-09-20 | OMO запинен на `oh-my-openagent@4.19.4`; модельная цепочка разделена planner/worker | commit `ef994b3` |
| 2026-09-24 | SoL-Pi совместим с Pi 0.87.0 (`check-pi-compat` exit 0) | `.ablation-solpi/PROBE.md` |
| 2026-09-24 | **Инцидент:** `opencode debug config` вывел ключи провайдеров в transcript. Ключи ротированы владельцем | post-incident запись, `.ablation-solpi/STATE.md` |
| 2026-09-25 | Post-incident hygiene: запрет команд с секретами, deny-правило, `.gitignore` разблокирован | commit `1b0b9e0` |
| 2026-09-25 | Причина падений прогонов — сетевой маршрут, не провайдер (туннель `neko-tun`, fake-IP) | замеры DNS/TCP/HTTPS, `.ablation-solpi/STATE.md` F2a |
| 2026-09-26 | **SoL-Pi отвергнут**: W1 PASS, W2 NO_ADOPT | commit `a3a2487`, `.ablation-solpi/REPORT.md` |
| 2026-09-26 | Три защиты из ablation подняты в постоянный воркер-слой: sandbox, loop-guard, validity gate | commit `033eb93` |
| 2026-09-26 | M0: `measure` GREEN, калибровка `tokens_per_byte = 813/4034`, `update` minimal | `crates/stellwerk-registry`, `modules.lock` |
| 2026-09-26 | Ночная сессия: контракт, store, спецификация packets | `.night/STATE.md` |

## Правило изменения этого документа

Правка PIVOT.md — это отдельное решение с датой и обоснованием, а не рефакторинг
текста. Если технология не меняет границы (движок, UI, транспорт) — она в PIVOT.md
не появляется.
