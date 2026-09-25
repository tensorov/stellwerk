# STATE — post-incident hygiene + Ablation SoL-Pi

Репо: `~/gits/stellwerk` (Rust workspace, main зелёный).
Протокол: `.ablation-solpi/BRIEF.md` — **дословно**, читать оттуда, не пересказывать.
Правило сессии: фазы строго последовательно, СТОП после каждой, ждать «ок».
Предыдущий агент умер от переполнения контекста; его хвост: `.ablation-solpi/PROBE.md`
(не трогать), `harness.sh`, `pi-home/`, `reports/`, `results/`, `src/`, `wt/`.

## Где остановились

**Фаза 0 (post-incident hygiene) — ВЫПОЛНЕНА, ждём «ок» на Фазу A.**

Осталось решить владельцу (блокеры, не мешают старту Фазы A):
1. Удаление 2 файлов с реальными ключами — заблокировано политикой `rm *`: deny в
   `opencode.json`. Точные команды ниже.
2. Судьба `opencode.db` (1289 значений) и живого `opencode.log` (42 значения).

## Инварианты (не нарушать)

- Никогда: `opencode debug config`, `cat auth.json`, `env | grep KEY` и любой вывод
  resolved config/секретов. Только `opencode debug paths` для получения путей.
- Коммиты в main — только те, что указаны брифом (0.5 и финал Phase D).
- Не трогать `.worktrees/m0/*`, `stellwerk-dispatch`, `stellwerk-worker-pi`.
- SoL-Pi ставить только в изолированные pi-home, никогда в `~/.pi/agent`.
- `evidencePreservingReducer=false`, `onlineContextCompact=false` — всегда.
- M0 (cmd_measure GREEN, cmd_update), OMP spike, Yao, PIVOT.md — вне скоупа.
- BASE commit всех прогонов: `2d572f1` (measure RED).

## Лог фаз

### Setup (перед Фазой 0)
- Файлы: `.ablation-solpi/BRIEF.md` (создан, дословно), `.ablation-solpi/STATE.md` (этот файл).
- Ничего не коммитилось. `git status` на старте: чисто, HEAD = `4c823a9`.
- Известно: `.ablation-solpi/` содержал `harness.sh`, `pi-home/`, `PROBE.md`, `reports/`,
  `results/`, `src/`, `wt/` — наследие прошлого агента, в Фазе 0 не тронуты.
- ВАЖНО ДЛЯ 0.1: `BRIEF.md` содержит в тексте сами grep-паттерны (`apiKey`, `api_key`,
  `"sk-`, `secret`). После коммита 0.5 повторный `git log -p --all | grep ...` даст
  ложные срабатывания на `BRIEF.md` — это ожидаемо, не секрет.

### Фаза 0 — ВЫПОЛНЕНА (2026-09-25), ждём «ок» на Фазу A

**0.1 секреты в git — ЧИСТО.**
- `git log -p --all` по паттерну `api[_-]?key|"sk-|bearer |secret` → `NO_MATCHES`
  (скан с подавлением значений: `/tmp/opencode/0_1_scan.py`, печатал только commit+file).
- Дополнительно: `git grep -l -i -E <тот же паттерн>` по отслеживаемому дереву →
  `NO_MATCHES_TRACKED`.
- ВНИМАНИЕ: после коммита BRIEF.md повторный скан даст ложное срабатывание на самом
  BRIEF.md/STATE.md — там лежит текст паттернов. Это не секрет.

**0.2 сессии на диске — ОБСЛЕДОВАНО, УДАЛЕНИЕ ЗАБЛОКИРОВАНО ПОЛИТИКОЙ.**
Метод: `opencode debug paths` (только пути) + скан по `apiKey|api_key|"sk-`, затем
классификация «паттерн» vs «значение» (`sk-<20+>`, `Bearer <tok>`, `"apiKey": "<...>"`).
Ни одно значение не печаталось в transcript.

Найдено 15 файлов с паттерном; из них **реальные значения** в 4:

| Файл | Значений | Статус |
|---|---:|---|
| `~/.local/share/opencode/tool-output/tool_0d5527249001XM3b0hEawx95lO` | 6 | НЕ УДАЛЁН — `rm *` deny |
| `~/.pi/agent/sessions/--home-izyabretatel-gits-stellwerk--/2026-09-24T08-03-56-596Z_01a0d271-5033-76c2-ad9d-69168d623d79.jsonl` | 3 | НЕ УДАЛЁН — `rm *` deny |
| `~/.local/share/opencode/log/opencode.log` | 42 | ЖИВОЙ (открыт процессом opencode, 2 fd) |
| `~/.local/share/opencode/opencode.db` | 1289 | ЖИВОЕ хранилище сессий, 894 МБ |

Остальные 11 файлов — только текст паттернов (значений 0), оставлены как есть:
9 файлов `tool-output/`, 2 сессии Pi, `~/.local/state/opencode/prompt-history.jsonl`.

Команды удаления (выполнить вручную или разрешить `rm`):
```bash
rm -f ~/.local/share/opencode/tool-output/tool_0d5527249001XM3b0hEawx95lO
rm -f "$HOME/.pi/agent/sessions/--home-izyabretatel-gits-stellwerk--/2026-09-24T08-03-56-596Z_01a0d271-5033-76c2-ad9d-69168d623d79.jsonl"
```
`opencode.db`/`opencode.log`: удаление необратимо уничтожит всю историю сессий
и, вероятно, оборвёт текущую сессию → решение за владельцем.
Ключи уже ротированы, поэтому остаточная опасность — «строка старого ключа лежит
локально», а не действующий доступ.

**0.3 запрет повтора — СДЕЛАНО.**
- `AGENTS.md` §Запрещено: добавлена строка про команды, печатающие resolved config/секреты.
- `opencode.json` → `permission.bash`: `"opencode debug config*": "deny"`.
- `~/.config/stellwerk-worker/opencode.json` — **не существует**, пункт N/A.
- `.gitignore`: `/.ablation*/` глушил весь каталог, из-за чего Phase B/D были бы
  невыполнимы (BRIEF/STATE/workloads/scripts/ledger/REPORT не коммитятся, а
  `pi-home*/auth.json` рисковал попасть в git). Исправлено: `!/.ablation-solpi/`
  + явные ignore для `harness.sh`, `pi-home*/`, `runs/`, `src/`, `wt/`, `reports/`,
  `results/`. Проверено 7 trackable + 8 ignored путей.

**0.4 docs/CONTEXT_BUDGET.md — СДЕЛАНО.**
- Добавлен раздел с окнами planner из `opencode models opencode --verbose`:
  space-bunny-free `1 048 576` (input 524 288 / output 524 288),
  mimo-v2.6-flash-free `200 000`, big-pickle `200 000`,
  qwen3.6-unlim-noreason `46 112` (worker).
- Зафиксировано: строки C/D/E — это prompt-стоимость OMO на big-pickle, для
  space-bunny не перепроверялась; переносить числа big-pickle на space-bunny нельзя.

**0.5 коммит** — `chore(security): post-incident hygiene + budget notes`, push не делался.

**FINDING (не чинил, вне скоупа):** в `docs/CONTEXT_BUDGET.md` остался плейсхолдер
«Дата: <впиши>» в шапке замера.

**FINDING (бюджет пакета):** BRIEF.md — 158 строк по прямому требованию брифа, плюс
STATE.md → diff пакета больше лимита AGENTS.md (200 строк). Причина — обязательный
артефакт, не код.

## Что нужно от владельца

- «ок» на переход к следующей фазе (после каждой фазы — СТОП).
- Ротация ключей провайдеров — **сделана владельцем** (инцидент закрыт частично).
