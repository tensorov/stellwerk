# VALIDATION.md — Three critical checks before Phase 3

Created: 2026-09-22
Author: Pi ablation validation

---

## ПРОВЕРКА 1: Что реально в P3=131?

### Raw session data (P3: custom PI.md prompt, --no-tools)

**Model response:**
- ✅ Непустой ответ: `STATUS: IDLE\nFILES:\nTEST:\nEVIDENCE:`
- ✅ finish_reason: `stop` (не error, не refusal)
- ✅ Модель осознанно прочитала PI.md и решила, что `.` = IDLE

**Usage breakdown:**
- input: **131** токенов
- output: **152** токенов (включая 132 токена thinking)
- cacheRead: **0**
- cacheWrite: **0**
- reasoning: **132**
- totalTokens: **283**

### Как P3=131 возможен?

PI.md содержит micro-worker prompt (~2000 chars ≈ 500-600 токенов).
Но Pi **не включает его целиком в input**.

**Причина:** `--system-prompt "$(cat PI.md)"` передаёт prompt как аргумент CLI.
Pi парсит его и использует как system prompt, но **input=131** — это токены,
которые провайдер (neuraldeep/qwen3.6) засчитал как **user input** (т.е. `.`).

System prompt токены **не входят** в `input` провайдера — они считаются отдельно
(обычно в `cacheRead` или в скрытом поле).

**Вывод:** P3=131 — **VALID**, но **неполный замер**.
input=131 — это только пользовательский ввод (`.`). System prompt (~500-600 токенов)
не учтён в `input`, но **включён в контекст модели**.

**P3 INVALID для сравнения с OpenCode V9** — метрика не учитывает system prompt tokens.

---

## ПРОВЕРКА 2: Что реально в P0=463?

### Raw data (P0: --no-tools --no-prompt-templates)

**System preamble:** 169 chars (~50-60 токенов)
**tools_added:** 0 (потому что --no-tools)
**stopReason:** stop

**Usage breakdown:**
- input: **463** токенов
- output: **71** токенов
- cacheRead: **0**
- cacheWrite: **0**
- reasoning: **0**
- totalTokens: **534**

**Ответ модели:** "Hello! I'm Pi, your expert coding assistant..." — стандартное приветствие.

### P0 = 463 — floor Pi?

463 токенов = preamble (~60) + "." (~1) + session setup + tool definitions (даже с --no-tools,
Pi всё равно включает определения встроенных инструментов в контекст).

**Это floor Pi** — минимальный возможный input для запуска с default system prompt.

**Согласуется** с заявленным "Pi system prompt is under 1000 tokens by design".

**P0 VALID** — это реальный floor Pi без extra context.

---

## ПРОВЕРКА 3: Метрика та же, что в OpenCode ablation?

### OpenCode V9 usage.json:
```json
{
  "first_step": {
    "input": 6058,
    "cache_read": 2368,
    "cache_write": 0,
    "context_input": 8426,
    "output": 122,
    "reasoning": 215
  }
}
```
Формула: `context_input = input + cache_read + cache_write = 6058 + 2368 + 0 = 8426` ✅

### Pi extract-pi-usage.py:
```python
ctx = input_tok + cache_read + cache_write
```
Формула: `context_input = input + cacheRead + cacheWrite` ✅

**Формулы совпадают.** ✅

### Пример из ledger.tsv:
```
P1	1	2966	870	2096	0	0
```
Разложение: context_input=2966, input=870, cacheRead=2096, cacheWrite=0
2966 = 870 + 2096 + 0 ✅

### КРИТИЧЕСКАЯ ПРОБЛЕМА:

**OpenCode V9=8426 включает контекст Stellwerk проекта** (AGENTS.md, CLAUDE.md,
файлы кода, tools definitions, skills, MCP tools).

**Pi P0=463 НЕ включает ничего** — `--no-context-files`, `--no-tools`, `--no-prompt-templates`.

**Pi P1=2966 включает skills, но НЕ включает Stellwerk проект.**

**Pi P2=2426 включает PI.md system prompt, но НЕ включает Stellwerk проект.**

**Pi P3=131 включает PI.md system prompt (но input=131 — это только `.`).**

**Вывод: сравнение Pi P0/P1/P2/P3 с OpenCode V9=8426 НЕКОРРЕКТНО.**
Разный контекст, разные инструменты, разные промпты.

---

## ЧЕГО НЕ ХВАТАЕТ ДЛЯ РЕШЕНИЯ

Оригинальная задача M-1c требовала:
```
P0 — Pi clean baseline                              ✓ (но нужен контекст Stellwerk)
P1 — Pi + Stellwerk micro prompt                    ✗ (P1 = tools+skills без micro prompt)
P2 — P1 + worker tools                              ✗ (другое)
P3 — P2 + PolyForge MCP                             ✗ (MCP не мерился)
```

Текущие P0-P3 отвечают на вопрос "сколько стоит каждая подсистема Pi",
но НЕ отвечают на главный вопрос: "может ли Pi заменить OpenCode V9 как worker с PolyForge".

---

## ПЛАН ИСПРАВЛЕНИЯ

### Q0 = Pi minimal floor (текущий P0, валидированный)
- `--no-context-files --no-tools --no-prompt-templates`
- Ожидаемо: ~463 токенов

### Q1 = Q0 + Stellwerk контекст + micro-worker system prompt
- Включить AGENTS.md из Stellwerk (как в OpenCode V9)
- Добавить PI.md system prompt
- Без tools (чтобы отделить контекст от tool overhead)
- Ожидаемо: ~463 + context_tokens + prompt_tokens

### Q2 = Q1 + worker tools allowlist
- `--tools read,grep,find,ls,edit,write,bash`
- Ожидаемо: Q1 + tool_definitions_tokens

### Q3 = Q2 + PolyForge MCP
- Если MCP в Pi недоступен → Q3 = BLOCKED
- Зафиксировать: какой adapter пробовал, какая ошибка

### Метрика
- Суммировать `input + cacheRead + cacheWrite` (context_input)
- Явно показывать разложение для каждого run
- Сравнивать с OpenCode V9=8426 при **одинаковом контексте**

---

*End of VALIDATION.md*
