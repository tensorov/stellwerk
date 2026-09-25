РОЛЬ: инженер-исследователь Stellwerk. Сессия: post-incident hygiene + Ablation SoL-Pi.
РЕЖИМ: фазы строго последовательно, СТОП после каждой, жду «ок». Не забегать вперёд.
ЯЗЫК: русский; команды/код — как есть.

ПЕРВОЕ ДЕЙСТВИЕ (до всего): сохрани этот бриф дословно в .ablation-solpi/BRIEF.md
и создай .ablation-solpi/STATE.md. После каждой фазы дописывай в STATE.md:
что сделано, какие файлы, на чём остановился. Предыдущий агент умер от переполнения
контекста — следующий должен продолжить с STATE.md без пересказа.

════════════════════════════════════════════════════════════════
ФАКТЫ (не перепроверять)
════════════════════════════════════════════════════════════════
Репо: ~/gits/stellwerk, Rust workspace. main зелёный (just check: fmt+clippy+test, 34+1).
Pi 0.87.0 (~/.pi/agent/bin/pi). Модель воркера: neuraldeep/qwen3.6-unlim-noreason,
окно 46112, max output 8000. Изоляция: PI_CODING_AGENT_DIR — работает.
OpenCode 1.18.32 + oh-my-openagent 4.19.4 (запинен) — только planner, не для воркеров.
Модельная цепочка OMO в .omo/omo.jsonc: space-bunny → mimo → big-pickle;
Momus/Metis primary = mimo. Big Pickle = только public данные (retention).
PolyForge: polyforge-cli через bash. evidence_verify allowlist узкий (cargo test, pytest…).

ИНЦИДЕНТ (закрыт частично): в прошлой сессии `opencode debug config` вывел ключи
провайдеров в transcript. Ключи РОТИРОВАНЫ пользователем. Остались пункты фазы 0.

SOL-PI: репо NVlabs/SoL-Pi, pin SHA 1559b5c, клон .ablation-solpi/src/SoL-Pi.
Совместимость с Pi 0.87.0 доказана: check-pi-compat против реального 0.87.0 → exit 0;
pi install + pi list OK; then_run присутствует в схемах edit/write.
Сигналы активации: Action Fusion → then_run в tool call; ObservationPack → записи
placeholder|recall в <session-dir>/sol-pi/<session-id>/ledger.jsonl.
BASE COMMIT всех прогонов: 2d572f1 (measure RED). Есть .ablation-solpi/PROBE.md — не трогать.

ФАКТЫ ИЗ ПАДЕНИЯ ПРОШЛОГО АГЕНТА (меняют методику):
Ф1. Pi компактит НАТИВНО (compaction_start/end видны без SoL-Pi) → peak context
    самоограничен, НЕ годится как главная метрика.
Ф2. packet-07a не имеет детерминированного объёма и не влезает в 300 с.
Ф3. usage надёжно читается из message_end.message.usage в JSONL (--mode json).

════════════════════════════════════════════════════════════════
ФАЗА 0 — HYGIENE ПОСЛЕ ИНЦИДЕНТА (30 минут, без агентских запусков)
════════════════════════════════════════════════════════════════
0.1 Проверить, что секреты не попали в git:
      git log -p --all | grep -inE 'api[_-]?key|"sk-|bearer |secret' | head
    Любое совпадение — СТОП, показать мне (без значений, только файл/коммит).
0.2 Найти сессии на диске, содержащие старые ключи, и удалить файлы (не редактировать):
      opencode debug paths   ← только для получения путей, НЕ debug config
      grep -rlE 'apiKey|api_key|"sk-' <session dirs opencode и ~/.pi/agent/sessions> 
    Список удалённых файлов — в STATE.md (имена, не содержимое).
0.3 Запретить повтор:
    - AGENTS.md, раздел «Запрещено», одна строка:
      «команды, печатающие resolved config или секреты (opencode debug config,
       cat auth.json, env | grep KEY и подобные)»
    - в worker-конфигах (~/.config/stellwerk-worker/opencode.json если есть,
      и в permission bash проектного opencode.json): "opencode debug config*": "deny"
0.4 Поправки в docs/CONTEXT_BUDGET.md:
    - строка «OMO 57 287 — измерено на big-pickle; для space-bunny не перепроверялось»
    - вписать context window space-bunny (взять из `opencode models opencode --verbose`)
0.5 Коммит: "chore(security): post-incident hygiene + budget notes". Push не делать.
СТОП. Показать git show --stat HEAD и результат 0.1.

════════════════════════════════════════════════════════════════
ФАЗА A — МЕТРИКИ И WORKLOAD
════════════════════════════════════════════════════════════════
ГЛАВНЫЕ метрики (по ним вердикт), на каждый прогон из JSONL:
  total_input_cum  = Σ по message_end (input+cacheRead+cacheWrite)
  total_output_cum = Σ output;  tool_steps = число tool_execution_start;  turns = число message_end
ВСПОМОГАТЕЛЬНЫЕ: peak_context, final_context, compactions (compaction_start, считать в ОБОИХ
  плечах), stop_reason_last, wall_seconds, timeout(bool)
АКТИВАЦИЯ: af_fired (tool calls с непустым then_run), op_fired (placeholder|recall в ledger).
  baseline: оба обязаны быть 0. solpi: хотя бы один >0, иначе прогон невалиден.
ЭКВИВАЛЕНТНОСТЬ: tests_outcome (`cargo test -p stellwerk-registry model` для W1,
  `... measure` для W2: exit + итоговая строка), diff_stat vs BASE, files_touched.

W1 «BOUNDED» — .ablation-solpi/workloads/W1.md. Детерминированная задача:
  ровно 3 микро-правки в crates/stellwerk-registry/src/model.rs на BASE — добавить
  #[test] fn w1_alpha / w1_beta / w1_gamma, каждая assert_eq!(2+2,4); после КАЖДОЙ
  добавленной функции запустить `cargo test -p stellwerk-registry model` и показать вывод;
  ничего другого не менять; хвост STATUS/FILES/TEST/EVIDENCE.
  Плюс строка: «Если инструмент edit/write поддерживает параметр then_run — используй его
  для запуска теста в том же вызове.» (одинаково в обоих плечах — в baseline параметра
  нет, строка безвредна). Ожидание: <10 шагов, <3 мин.
W2 «REALISTIC» — .ablation-solpi/workloads/W2.md = ДОСЛОВНАЯ копия
  .sisyphus/plans/M0/packet-07a.md. Таймаут 900 с, по таймауту SIGTERM→10с→SIGKILL,
  метрики из того, что записалось, timeout=true. Валиден для эффективности,
  НЕ валиден для эквивалентности — отметить в отчёте.
MICRO (append-system-prompt, оба плеча) — взять дословно из PROBE.md п.5.
СТОП. Показать W1.md, подтвердить что packet-07a.md найден и скопирован дословно.

════════════════════════════════════════════════════════════════
ФАЗА B — ХАРНЕСС
════════════════════════════════════════════════════════════════
.ablation-solpi/{workloads/, pi-home-baseline/, pi-home-solpi/, runs/<ts>/<arm>/<wl>/<r<N>/,
  scripts/{run-one.sh,run-all.sh,extract.py,integrity.sh}, ledger.tsv, REPORT.md}
B1 Два pi-home. В baseline SoL-Pi НЕ УСТАНОВЛЕН (не «флаги false»). Доказательство:
   PI_CODING_AGENT_DIR=<baseline> pi list → SoL-Pi отсутствует. Вывод — в отчёт.
   В оба pi-home скопировать auth.json/models.json (нужны для провайдера); они уже
   содержат ротированные ключи — НЕ выводить их содержимое.
B2 pi-home-solpi/sol-pi.json:
   {"version":1,"actionFusion":true,"observationPack":true,
    "evidencePreservingReducer":false,"onlineContextCompact":false,"cacheWriteReadRatio":12.5}
   onlineContextCompact=false ОБЯЗАТЕЛЬНО (Ф1). Reducer=false всегда.
B3 Каждый прогон — свежий detached worktree на BASE:
   git -C ~/gits/stellwerk worktree add --detach <tmp> 2d572f1; после сбора метрик и
   diff — worktree remove --force. Никогда не запускать в ~/gits/stellwerk или .worktrees/m0/*.
B4 Свежий session dir на каждый rep (--no-session + отдельный PI_CODING_AGENT_SESSION_DIR);
   для solpi чистить sol-pi/<session-id>/ между rep.
B5 Команда идентична, отличается только PI_CODING_AGENT_DIR:
   pi --provider neuraldeep --model qwen3.6-unlim-noreason --thinking off \
      -p "$(cat <workload>)" --mode json --no-session --no-prompt-templates \
      --append-system-prompt "<MICRO>"   > run.jsonl 2> run.stderr
   Таймаут: W1=300 с, W2=900 с.
B6 extract.py — все метрики фазы A → metrics.json. Без внешних зависимостей.
B7 integrity.sh — ТОЧНОЕ совпадение: до старта сохранить `git rev-parse HEAD` и
   `git status --porcelain` основного репо; после каждого прогона diff с ними;
   любое расхождение = FAIL, остановить прогоны.
B8 QA: один пробный прогон baseline/W1. Проверить: metrics.json заполнен, worktree убран,
   integrity OK, в JSONL есть message_end.usage. Если W1 в пробном прогоне решается
   «странно» (модель не делает 3 отдельных цикла edit→test) — поправить формулировку W1
   ДО матрицы, не после.
СТОП. Показать metrics.json пробного прогона и вывод integrity.sh.

════════════════════════════════════════════════════════════════
ФАЗА C — ПРОГОНЫ
════════════════════════════════════════════════════════════════
2 плеча × 2 workload × 3 rep = 12, последовательно:
baseline/W1×3 → solpi/W1×3 → baseline/W2×3 → solpi/W2×3. integrity.sh между всеми.
Всё в ledger.tsv (ts, arm, workload, rep, все метрики).
Если af_fired=0 И op_fired=0 во ВСЕХ solpi-прогонах — расширение не загрузилось:
СТОП, диагностика загрузки (pi list в solpi-home, stderr), не писать отчёт с ложным выводом.
СТОП. Показать ledger.tsv.

════════════════════════════════════════════════════════════════
ФАЗА D — ОТЧЁТ .ablation-solpi/REPORT.md
════════════════════════════════════════════════════════════════
1 Таблица (arm, workload): медиана/min/max всех метрик.
2 Дельты по W1 и W2 ОТДЕЛЬНО: d_input_cum, d_output_cum, d_steps, d_turns; % от baseline.
3 Активация: af_fired/op_fired по плечам.
4 Компакции по плечам; объяснить, почему вердикт на total_input_cum, а не на peak.
5 Эквивалентность (W1 обязательно; W2 — только если оба плеча завершились без timeout).
6 ВЕРДИКТ W1: PASS d_input_cum ≥15% И af_fired>0 И эквивалентно;
  PARTIAL 5–15% или частичная эквивалентность; FAIL <5% или нет активации или расходятся.
  Отдельной строкой вердикт W2 с пометкой timeout.
7 Риски внедрения в stellwerk-worker-pi (install scope, pin, апгрейд Pi) — описать, НЕ внедрять.
8 Незакрытое: onlineContextCompact и evidencePreservingReducer не мерились.
Финал: обновить STATE.md; коммитить в main ТОЛЬКО REPORT.md, ledger.tsv, scripts/, workloads/
(не runs/, не pi-home-*). Сообщение: "docs(ablation): SoL-Pi A/B on Pi 0.87.0".
СТОП. Вернуть: таблицу дельт, вердикты W1/W2, доказательство активации.

════════════════════════════════════════════════════════════════
ЗАПРЕЩЕНО ВО ВСЕХ ФАЗАХ
════════════════════════════════════════════════════════════════
- opencode debug config, cat auth.json, вывод переменных окружения с ключами — никогда
- коммиты в main кроме указанных; трогать .worktrees/m0/*, stellwerk-dispatch, stellwerk-worker-pi
- установка SoL-Pi в ~/.pi/agent (только изолированные pi-home)
- evidencePreservingReducer=true; onlineContextCompact=true
- дописывать cmd_measure «чтобы прогон выглядел успешным»
- переформулировать packet-07a своими словами
- сравнивать числа qwen с числами big-pickle
- M0 (cmd_measure GREEN, cmd_update), OMP spike, Yao, PIVOT.md — ВНЕ СКОУПА этой сессии,
  для них будут отдельные промпты
