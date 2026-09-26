# STATE — post-incident hygiene + Ablation SoL-Pi

Репо: `~/gits/stellwerk` (Rust workspace, main зелёный).
Протокол: `.ablation-solpi/BRIEF.md` — **дословно**, читать оттуда, не пересказывать.
Правило сессии: фазы строго последовательно, СТОП после каждой, ждать «ок».
Предыдущий агент умер от переполнения контекста; его хвост: `.ablation-solpi/PROBE.md`
(не трогать), `harness.sh`, `pi-home/`, `reports/`, `results/`, `src/`, `wt/`.

## Где остановились

**Фаза C — ПРЕРВАНА integrity-гейтом на 7-м прогоне из 12. Вердикт невозможен.**
Оставшиеся 5 прогонов (baseline/W2 ×2, solpi/W2 ×3) НЕ запускались.
Коммита нет. Отчёт Фазы D не пишется — данных недостаточно, ложный вердикт запрещён.

### Внедрено 2026-09-26 (ждёт отката репо для смоука)

- **Гейт валидности** в `extract.py`: `valid=false`, если `stop_reason_last == "error"`
  **или** `total_input_cum == 0` **или** `tool_steps == 0` **или** (`timeout` и `turns < 2`).
  Exit 0 от Pi больше не признак успеха. Пишутся `valid`, `invalid_reason`, `attempt`, `retry_of`.
- **`scripts/retry-matrix.sh`** (заменяет run-all.sh для Фазы C): ≤3 попытки на rep,
  пауза **45 с** между ЛЮБЫМИ прогонами, invalid не удаляется, `rep_status=FAILED`
  после исчерпания попыток,матрица продолжается; **>2 FAILED из 12 → СТОП** с отчётом.
- **Ранняя integrity-проверка** в `run-one.sh` сразу после выхода агента (до teardown):
  ловит запись вне worktree за секунды, код выхода 2 →матрица останавливается.
- **Ledger-схема расширена**: `attempt, retry_of, valid, invalid_reason, rep_status`.
  Старая m1-ledger перемещена в `runs/m1/ledger.tsv` — 2 валидных прогона оттуда
  в финальный ledger НЕ переносятся (методика изменилась).
- **Guard-фраза добавлена в оба workload** (W1.md и W2.md), симметрично для обоих плечей,
  без абсолютных путей: «Работай только внутри текущего каталога… не найден → STATUS: BLOCKED…
  не ищи в других каталогах».
- **`scripts/w2_preflight.sh` — ПРОЙДЕН:** на чистом BASE все 4 файла packet-07a на месте
  (`cmd_measure.rs` 221 строка, `lib.rs` объявляет `pub mod cmd_measure;`),
  `cargo test -p stellwerk-registry measure` → **exit 101, `6 passed; 6 failed` = ожидаемый RED**.
  **Фикстура не нужна.** sha256 `cmd_measure.rs`: BASE `624bb0603922b2016c44a99e6d2a381f4dae1a9f6d8e2092cbee652dc4e0eb20` (221),
  `.worktrees/m0/measure` `5358fe4c772a390b6ebdc4b27ea4eed3e1e928de6f073e7b3e5940bea76db524` (514).

### Требует решения владельца (блокирует перезапуск)

1. **Откатить основной репозиторий** — команды ниже; `rm` и `git checkout --` запрещены
   политикой проекта, выполнить их не могу.
2. **Provider timeouts:** гейт валидности + повтор прогона + пауза между прогонами.
3. **W2-премисса:** `cmd_measure.rs` отсутствует в git — фикстура в worktree или смена BASE.

### Фаза C — ПРЕРВАНА (2026-09-26): прогонов 7 из 12

ledger.tsv (ts=m1) — **валидных строк 2 из 7**:

| arm/WL/rep | input_cum | steps | af | ВАЛИДНОСТЬ |
|---|---:|---:|---:|---|
| baseline/W1/1 | 76 854 | 8 | 0 | OK |
| baseline/W1/2 | 24 622 | 5 | 0 | **INVALID** stop_reason=error (таймаут) |
| baseline/W1/3 | 0 | 0 | 0 | **INVALID** stop_reason=error, usage=0 |
| solpi/W1/1 | 0 | 0 | 0 | **INVALID** stop_reason=error, usage=0 |
| solpi/W1/2 | 0 | 0 | 0 | **INVALID** stop_reason=error, usage=0 |
| solpi/W1/3 | 118 996 | 14 | **7** | OK |
| baseline/W2/1 | 1 483 127 | 59 | 0 | **INVALID** работа вне worktree |

`integrity.sh` сработал верно, поймав `M cli.rs`, `M lib.rs`, `?? cmd_measure.rs` (529 строк).

**F1 (блокирующий): W2-агент САМ ушёл из worktree в основной репозиторий.**

⚠ Моя первая версия F1 была НЕВЕРНОЙ: я написал, что `cmd_measure.rs` «никогда не
коммитился», приняв собственный label «(пусто = …)» за результат `git log`. Факты:
- `git show 2d572f1:…/cmd_measure.rs` = **221 строка**, BASE-овый `lib.rs` содержит
  `pub mod cmd_measure;` → премисса packet-07a на BASE **выполнена**, фикстура не нужна;
- `git merge-base --is-ancestor 2d572f1 HEAD` → **НЕТ**: линии разошлись, main законно
  не содержит measure-кода;
- причина выхода — абсолютные пути: 12 `edit` в `/home/izyabretatel/gits/stellwerk/crates/
  stellwerk-registry/src/cmd_measure.rs`, 4 в `cli.rs`, 4 в `lib.rs`, 2 в `model.rs`,
  3 `read` в `.worktrees/m0/measure/…` и **0 путей в собственном worktree**;
- 27 bash-команд, из них `find /home/izyabretatel/gits/stellwerk -type f -name "*.rs"`
  и многократно `cd /home/izyabretatel/gits/stellwerk && cargo test …` — агент ушёл из
  каталога работы по абсолютным путям;
- diff его worktree — 0 байт, `tests_exit=101`.

Вывод: это не «BASE сломан», а отсутствие песочницы. Мера владельца (guard-фраза в
workload) бьёт точно в этот путь; добавлю также раннюю integrity-проверку сразу после
выхода агента, чтобы ловить заражение за секунды, а не после teardown.

Откат выполняет владелец (мне `rm` и `git checkout --` запрещены):
```bash
git checkout -- crates/stellwerk-registry/src/cli.rs crates/stellwerk-registry/src/lib.rs
rm -f crates/stellwerk-registry/src/cmd_measure.rs
```

**F2 (блокирующий): provider timeouts убили 4 прогона из 6.**
`auto_retry_start.errorMessage = "Request timed out."`, 3 попытки (backoff 4/8 с),
`stopReason=error`, при этом **Pi завершается кодом 0** → харнесс записал их успешными.
Нужен `invalid` при `stop_reason_last == "error"` либо `total_input_cum == 0` либо `tool_steps == 0`.

**F2a: ПРИЧИНА — СЕТЕВОЙ МАРШРУТ, А НЕ ПРОВАЙДЕР. Гипотеза владельца подтверждена
2026-09-26; моя прежняя атрибуция провайдеру была ОШИБКОЙ.**

⚠ Раньше я написал «это НЕ интернет, эндпойнт нестабилен» и обосновал это сравнением
с контролем. Обоснование было неверным: контроль шёл к Cloudflare (близко), провайдер —
к российскому хосту напрямую из украинской сети. Разница могла целиком быть в маршруте.

Прямое доказательство — сменился сетевой путь, и проблема исчезла:

| замер | тогда (прямой маршрут) | сейчас (через туннель) |
|---|---|---|
| DNS `api.neuraldeep.ru` | `85.193.87.107` (реальный IP) | **`198.18.0.12` (fake-IP)** |
| DNS `opencode.ai` | `172.65.90.21` | `198.18.0.6` |
| TCP 443 провайдера | 0.38-0.77 с, выбросы 3.42 / 5.58 с | **0.00 с ×5** |
| HTTPS провайдера | **9.67 с** | **0.79-1.23 с ×3** |

`198.18.0.0/15` — RFC2544, в публичном интернете не маршрутизируется: это fake-IP
прокси. Активен TUN `neko-tun` (172.19.0.1/28), DNS перехватывается туннелем.

Функциональное подтверждение:
- 3/3 пробных вызова Pi: `stop=stop`, **0 ретраев**, 2.2 / 3.3 / 2.2 с;
- перезапуск РОВНО ТОГО ЖЕ прогона, который умер (`baseline/W2`):
  было `valid=false, stop_reason=error, 9 шагов, tests_exit=101`;
  стало `valid=true, stop=stop, 39 шагов, tests_passed=18, wall=439 с, 0 ретраев`.

Итог: `APIConnectionTimeoutError` был следствием плохого прямого маршрута до
`api.neuraldeep.ru`. Эндпойнт исправен. Провайдер как таковой не при чём.
Оговорка: точный механизм маршрута A/B-не проверен (нельзя одновременно снять
ту же цель с туннелем и без него), но факт причинной связи установлен измерением.

**F4 (моя ошибка в харнессе, исправлено):** ранняя integrity-проверка всегда давала
ложный FAIL, потому что сравнивала снимок «до прогона» с состоянием, где worktree
прогона зарегистрирован по устройству. Дополнительно `grep -v` по
`git worktree list --porcelain` резал многострочную запись построчно и оставлял
сиротские `HEAD`/`detached`. Исправлено фильтрацией ЗАПИСИ через awk; проверено
тремя тестами (флаг=1 → OK, флаг=0 → FAIL, 7 посторонних worktree не скрыты).

**F5 (решено, изоляция путей работает):** guard-фраза в W1.md/W2.md («работай только
внутри текущего каталога… не найден → STATUS: BLOCKED») дала 0 выходов наружу во всех
прогонах после её добавления, против 12 edit в основной репо в m1. Плюс path-guard
до запуска (симлинки + отсутствие абсолютных путей в промптах). Ограничение остаётся:
изоляция путей, не namespace (bubblewrap/firejail отложены решением владельца).

**F6 (наблюдение для интерпретации W2):** на длинных прогонах нативная компакция Pi
работает — `compactions` = 6 у успешного baseline/W2 против 0 у оборванного. Значит
вердикт по `peak_context` невозможен в принципе (Ф1), только `total_input_cum`.

**F3: n=1 на плечо не даёт вердикта.** Валидные прогоны: baseline 76 854 против solpi
118 996 — направление ПРОТИВОПОЛОЖНО QA-прогону (69 152 против 47 936). Усреднять нельзя.

### Фаза B — ВЫПОЛНЕНА (2026-09-25)

**base_preflight (B-PREFLIGHT):** detached worktree на `2d572f1`, без Pi:
`cargo test -p stellwerk-registry model` → **exit 0**,
`test result: ok. 8 passed; 0 failed; ... 22 filtered out`.
Приёмка W1 валидна на BASE. Worktree удалён.

**Секреты в pi-home:** `umask 077`; копии `auth.json`/`models.json`/`models-store.json`
правами 600; оба pi-home `IGNORED` (проверено `git status --ignored` → `!!`);
содержимое ни разу не выводилось. `~/.pi/agent` не тронут (SoL-Pi туда не ставился).

**B1:** baseline-home — `pi list` → «No packages installed.» (SoL-Pi реально отсутствует,
не «флаги false»); solpi-home — SoL-Pi из локального клона, `git rev-parse` = `1559b5c`,
установлен user-wide (`settings.json: packages: ["../src/SoL-Pi"]`), т.е. в worktree
ничего не попадает и `files_touched` не загрязняется.
**B2:** `pi-home-solpi/sol-pi.json` — ровно по брифу, `onlineContextCompact=false`,
`evidencePreservingReducer=false`. В baseline-home `sol-pi.json` отсутствует.

### МЕТОДИКА ИЗМЕНЕНА ПО РЕЗУЛЬТАТАМ B8 (записано по требованию владельца)

1. **`--no-session` снят** в обоих плечах. Причина: `run.stderr` solpi-плеча показал
   `Extension error: SoL-Pi requires a persistent Pi session directory` (13 раз) —
   расширение не грузилось вообще. Команда в обоих плечах идентична, отличается только
   `PI_CODING_AGENT_DIR`; session-dir уникален на каждый rep и лежит в ignored `runs/`.
2. **Детектор AF переведён на структурный.** Наивный поиск `then_run` где угодно дал
   `af_fired=6` при нуле реальных слияний: строка «then_run» есть в прозе workload-а.
   Теперь считается только `tool_execution_start.args.then_run` и `toolCall.arguments.then_run`.
   На старых прогонах регресс даёт `af_fired=0` — то есть прежние 6 были ложью.
3. **Добавлен гейт загрузки расширения:** `solpi_extension_errors` (строки
   `Extension error … SoL-Pi`) и `arm_valid`; прогон solpi с непустым `arm_valid` —
   невалиден и не должен попадать в отчёт.
4. **W1 переформулирован ДО матрицы** (по смыслу B8), в обоих плечах одинаково:
   - явный якорь вставки («последняя строка — `}`, вставляй перед ней, не заменяя якорь»);
   - аддитивность запрещена явно: ни один из 8 базовых тестов нельзя изменять/опустошать;
   - `then_run` сделан обязательным при наличии в схеме (в baseline параметра нет —
     фраза инертна, как и предполагает бриф).
   Эффект: `tool_steps` 14 → 7, эквивалентность стала корректной.
5. **Добавлен `verify_w1.py` (аддитивная эквивалентность).** Проверка по именам тестов
   оказалась недостаточной: первый QA-прогон оставил сьют «зелёной» (11 passed), но
   агент заменил тело `source_roundtrip_crate` на пустое (diff −4). Теперь файл
   прогона минус блоки `w1_*` сравнивается с BASE по значимым строкам:
   `base_intact`, `delta_bytes`. Условие эквивалентности теперь включает
   `base_intact && delta_bytes==0 && !base_tests_missing`.
6. **Двойной счёт usage закрыт:** `message_end` и `turn_end` дают идентичные суммы
   (проверено: 29/29 событий, совпадение до байта). Суммируется строго `message_end`
   (Ф3), `turn_end` — только cross-check (`usage_match`).
7. **`CARGO_TARGET_DIR` общий на ts** (в ignored `runs/<ts>/cargo-target`) — иначе
   каждый прогон компилирует workspace заново и `wall_seconds` загрязняется компиляцией.
   Одинаково в обоих плечах.

### B8a / B8b (qa-b8-v5, финальный harness)

| Метрика | baseline/W1 | solpi/W1 |
|---|---:|---:|
| `total_input_cum` | 69 152 | **47 936** |
| `total_output_cum` | 1 221 | 1 386 |
| `tool_steps` | 7 | **6** |
| `turns` | 8 | 7 |
| `compactions` | 0 | 0 |
| `peak_context` | 12 103 | 10 438 |
| `wall_seconds` | 35 | 33 |
| `af_fired` | 0 | **5** |
| `op_fired` | 0 | 0 |
| `solpi_extension_loaded` | true | true |
| `tests_passed / failed` | 11 / 0 | 11 / 0 |
| `base_intact` / `delta_bytes` | true / 0 | true / 0 |
| `equivalence_ok` / `arm_valid` | true / true | true / true |

Это **один** прогон на плечо —方向ление, не вердикт. Вердикт Фазы D считается по 3 rep.

**OP не сработал — и это не артефакт `--no-session`.** Доказательства: (а) при
`--no-session` расширение вообще не грузилось; (б) при persisted-сессии SoL-Pi
загружен и **advertise'нул** `then_run` в схемах `edit`/`write` плюс новый тул
`obs_recall` — то есть OP активен; (в) ledger не создан, потому что в W1 нет
идентичного повторного результата >4 KB: каждый `cargo test` даёт РАЗНЫЙ вывод
(8 → 9 → 10 тестов), а OP срабатывает на 3-м одинаковом re-send. Значит OP —
кандидат на W2 (повторные чтения одних и тех же файлов), а не на W1.

**Не тронуто (наследие прошлого агента, вне скоупа):** `.worktrees/.ablation/baseline-r2`
(2d572f1) и `.worktrees/.ablation/test-wt` (prunable, gitdir отсутствует). Не удалял:
`rm` запрещён политикой, а `worktree prune` меняет `.git` вне задач Фазы B.

**Проверка секретов в results/ прошлого агента:** `.ablation-solpi/results/**/*.jsonl` и
`harness.sh` — 0 значений ключей, только совпадения паттернов (0 совпадений).

### Харнесс обновлён 2026-09-26 (решения владельца)

**`sol-pi.json` (плечо solpi) — дословно:**
```json
{"version":1,"actionFusion":true,"observationPack":true,
    "evidencePreservingReducer":false,"onlineContextCompact":false,"cacheWriteReadRatio":12.5}
```
- `onlineContextCompact=false` — подтверждено (bool) и эмпирически: в session-дереве
  прошлого solpi-прогона `online-context-compact/` содержит **0 файлов**;
- `evidencePreservingReducer=false` — подтверждено (bool) и эмпирически:
  `evidence-preserving-reducer/` содержит **0 файлов**; `extract.py` теперь считает
  `epr_records` и `solpi_compactions` отдельными полями, чтобы это было доказуемо в ledger;
- в `session/sol-pi/<id>/` реально присутствует только `observation-pack/` (3 файла);
- в `pi-home-baseline/sol-pi.json` **отсутствует** (SoL-Pi не установлен вовсе, B1).

**Компакции разделены:** `native_compactions` = события `compaction_start` в JSONL
(нативная компакция Pi), `solpi_compactions` = записи в `online-context-compact/`
(0 при текущих флагах), `compactions` = сумма.

**`net_path` в ledger (снятие ДО прогота):** резолв `api.neuraldeep.ru`
(`fake-ip:198.18.x` | `real-ip:<ip>`) + наличие tun-интерфейса (`neko-tun`).
`retry-matrix.sh` проверяет согласованность: если `net_path` меняется внутри матрицы,
прогоны разных путей не сравниваются.

**Песочница переведена на `git archive BASE | tar -x` ВНЕ репозитория**
(`/tmp/opencode/ablation-sandbox/<ts>/…`): `.git` отсутствует, значит ничто не указывает
на хост-репо. Следствие: `git diff` внутри песочницы невозможен → добавлен
`dir_diff.py` (numstat по двум деревьям, формат совместим с прежним парсером), а строка
acceptance в W1.md переформулирована без зависимости от git.
`CARGO_TARGET_DIR` тоже вне репозитория и удаляется после метрик; сохраняются
`run.jsonl`, `run.stderr`, `tests.txt`, `metrics.json`, `diff.txt`, `session/`
(в session лежит ledger.jsonl ObservationPack — доказательство активации).

**Решение по повторному использованию прогонов: НЕ ПЕРЕИСПОЛЬЗУЮТЬ → 8 новых прогонов.**
Основания (ваш критерий совместимости: тот же BASE, MICRO, isolation, сеть/туннель):
1. `qa-b8-v5` шёл по **старому прямому маршруту**, `smoke-w2b` — по **туннелю**
   (net_path различается, а вы сами сделали это условием сравнимости);
2. `W1.md` получил раздел «Границы работы» **после** `qa-b8-v5` — промпт различается;
3. `CARGO_TARGET_DIR` был общим на ts во время `qa-b8-v5` и стал per-run позже.
Плюс валидный `solpi/W2` из smoke-w2 — тоже старый маршрут.
Итого: 2×2×2 = 8 новых прогонов, m1 и smokes в финальный ledger не входят.

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

### Фаза A — ВЫПОЛНЕНА (2026-09-25), ждём «ок» на Фазу B

Проверка живости после Фазы 0: `opencode --version` = `1.18.32`, `just check` зелёный,
`opencode.db` читается (20 таблиц). Файлы с ключами не удалялись (политика), но на
живость OpenCode не повлияло.

**Файлы:**
- `.ablation-solpi/workloads/W1.md` — создан (62 строки), trackable, untracked.
- `.ablation-solpi/workloads/W2.md` — **дословная копия** `.sisyphus/plans/M0/packet-07a.md`:
  sha256 обоих `17ce98d88847af1e810a1c33867fd79ebd383006131ab38e66fe9a1f21e496c4`,
  `cmp` = identical, 70 строк / 6916 байт. Источник не изменён.

**W1 заземлён на BASE, а не на main:** `git show 2d572f1:crates/stellwerk-registry/src/model.rs`
= 232 строки, `#[cfg(test)] mod tests` на 122–123, закрывающая `}` на 232.
Между BASE и HEAD файл отличается на 1 строку (`#[serde(default)]` над
`pub module`) — на W1 не влияет, прогоны идут в worktree на BASE.
Приёмка W1: `cargo test -p stellwerk-registry model` → exit 0 + три имени тестов.
Коммит в Фазе A не делается: `workloads/` коммитится в Финале Фазы D.

**MICRO (дословно, PROBE.md п.5, задаётся через `--append-system-prompt` в оба плеча):**
```
Ты micro-worker Stellwerk. Выполни ровно один packet ниже. Не читай другие packets и планы.
Хвост ответа строго: STATUS/FILES/TEST/EVIDENCE.
```

**Метрики extract.py — зафиксированы (полный список в ответе фазы).**
Главные: `total_input_cum` = Σ(message_end: input+cacheRead+cacheWrite),
`total_output_cum` = Σ output, `tool_steps` = count(tool_execution_start),
`turns` = count(message_end).
Вспомогательные: `peak_context`, `final_context`, `compactions` = count(compaction_start)
(оба плеча), `stop_reason_last`, `wall_seconds`, `timeout`.
Активация: `af_fired` = tool-call с непустым `args.then_run`,
`op_fired` = записи `event ∈ {placeholder, recall}` в `<session>/sol-pi/<id>/ledger.jsonl`;
дополнительно `op_removed_tokens` = Σ `removedTokens` (PROBE.md §3.2).
Эквивалентность: `tests_exit`, `tests_last_line`, `w1_names_present`,
`diff_numstat` vs BASE, `files_touched`.
Baseline: `af_fired` и `op_fired` обязаны быть 0.

**ОТКРЫТЫЙ РИСК ФАЗЫ B (проверяется первым шагом B8):** приёмка W1 требует
`cargo test -p stellwerk-registry model` → exit 0 на BASE. Тела model-тестов между
BASE и HEAD не менялись, но фактический прогон на чистом BASE не выполнялся.
B8 обязан начать с этой проверки (без агента); если красный — правка W1 ДО матрицы.

## Что нужно от владельца

- «ок» на переход к следующей фазе (после каждой фазы — СТОП).
- Ротация ключей провайдеров — **сделана владельцем** (инцидент закрыт частично).
