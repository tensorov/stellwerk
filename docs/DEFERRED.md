# DEFERRED — всё, что не M0

Сюда Prometheus записывает вопросы и идеи, влияющие только на M1+. Не планировать.

- audit для skill-модулей (SKILL.md)
- точный токенизатор qwen
- stellwerk-gateway, stellwerk-bus, stellwerk-sync, stellwerk-nag, stellwerk-hooks
- kanban / Hermes / OpenResearch / CRM / Obsidian-проекция / IoT / TTS

## Отложено после SoL-Pi ablation (2026-09-26)

Решение по SoL-Pi: **не внедрять** (ни AF-only). Обоснование — `.ablation-solpi/REPORT.md`,
commit `a3a2487`. В постоянный воркер-слой перенесены только три измерения, не сам SoL-Pi:
sandbox, loop-guard, validity gate (`scripts/worker/`).

- **W2-lite / OP-only study** — сократить packet-07a до измеримого за 300 с, чтобы
  отдельно измерить ObservationPack. Нынешний W2 не имеет разрешающей способности:
  он измеряет надёжность qwen, а не SoL-Pi. `onlineContextCompact` и
  `evidencePreservingReducer` не мерились вовсе.
- **bubblewrap / firejail namespace-изоляция** — сейчас только path-guard. Он не
  является защитой на уровне syscall: воркер технически может записать по
  абсолютному пути, и мы это ловим post-flight (доказано тестом F1), а не запрещаем.
  Настоящее закрытие требует namespace-изоляции.
- **Разбор дисперсии qwen на длинных packet** — в m3 solpi/W1 один прогон из трёх
  ушёл в 47 шагов / 663 656 токенов при `af_fired=1`: разброс 15× при медиане, которая
  выглядит убедительно. Пока не понятно, это вариативность модели, вариативность
  использования опциональной способности, или артефакт тренировки.
- **`LOOP_TOTAL_LIMIT`** в `scripts/worker/dispatch.conf` по умолчанию `0` (выключен):
  ловится только явное повторение подряд. Порог по сумме за прогон — ручка, которую
  стоит подобрать после накопления статистики.
