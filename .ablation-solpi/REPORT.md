# REPORT — SoL-Pi A/B на Pi 0.87.0 (модель qwen3.6-unlim-noreason)

Дата: 2026-09-26. Протокол: `.ablation-solpi/BRIEF.md`. Состояние: `.ablation-solpi/STATE.md`.

BASE всех прогонов: `2d572f1`. Плечо solpi: SoL-Pi pin `1559b5c`, изолированный pi-home.

`net_path` **одинаков во всех прогонах матрицы**: `fake-ip:198.18.0.103`, tun `neko-tun`.


## 1. W1 «BOUNDED» — вердикт по микропакетам

n=3 на плечо; все 6 прогонов `valid=true`, `equivalence_ok=true`, `base_intact=true`, 11 passed.


| Метрика | baseline | solpi | Δ | % |
|---|---:|---:|---:|---:|
| `total_input_cum` медиана | 67847 | **56108** | -11739 | **-17.3%** |
| `total_input_cum` min–max | 34115–75933 | 43033–663656 | — | — |
| `tool_steps` медиана | 8 | **7** | -1 | -12.5% |
| `tool_steps` min–max | 4–9 | 5–47 | — | — |
| `af_fired` по прогонам | [0, 0, 0] | **[1, 5, 4]** | — | — |
| `op_fired` | [0, 0, 0] | [0, 0, 0] | — | — |
| `peak_context` max | 11879 | 24028 | — | — |
| компакции native (сумма) | 0 | 0 | — | — |

**ВЕРДИКТ W1: PASS** — `d_input_cum` = −17.3% (порог ≥15%), `af_fired`>0 в solpi (1/5/4) и 0 в baseline, эквивалентность подтверждена.


⚠ Хвост: solpi/r1 вырвался в 47 шагов и 663 656 токенов. Разброс solpi 43 033–663 656 против baseline 34 115–75 933 — медиана устойчива, но хвост 15×.


## 2. W2 «REALISTIC» — вердикт по реальному пакету

n=3 на плечо. `success` = уложился в 900 с ∧ measure GREEN ∧ ни один BASE-тест не удалён.


| arm | прогон | уложился | steps | passed | verdict | removed | added | success |
|---|---|---|---:|---:|---|---:|---:|---|
| baseline | smoke-w2b | да | 39 | 18 | partial | 0 | 6 | **да** |
| baseline | m3 | **НЕТ** | 78 | — | partial | 0 | 6 | **нет** |
| baseline | m3 | да | 39 | 18 | partial | 0 | 6 | **да** |
| solpi | smoke-w2 | да | 44 | 18 | partial | 0 | 6 | **да** |
| solpi | pair-w2 | **НЕТ** | 166 | — | partial | 0 | 4 | **нет** |
| solpi | pair-w2b | **НЕТ** | 88 | 17 | partial | 0 | 5 | **нет** |

**success_rate: baseline 2/3, solpi 1/3**


| Метрика | baseline | solpi |
|---|---|---|
| `tool_steps` медиана | 39 | 88 |
| `total_input_cum` медиана | 922953 | 2487241 |
| `tool_steps` min–max | 39–78 | 44–166 |
| `op_fired` | [0, 0, 0] | [3, 0, 0] |
| `af_fired` | [0, 0, 0] | [0, 0, 0] |
| `peak_context` max | 35059 | 36068 |
| компакции native (сумма) | 25 | 34 |

**ВЕРДИКТ W2: NO_ADOPT** — `success(solpi)=1/3` < `success(baseline)=2/3`: SoL-Pi ухудшает успех на сложных задачах независимо от W1.


## 3. Активация
- **AF (W1)**: 1/5/4 в solpi, 0/0/0 в baseline; детектор структурный (`tool_execution_start.args.then_run`), проза workload-а исключена.
- **OP (W2)**: `op_fired=3`, `op_removed_tokens=11 523`, `ledger_files=1` — только в `smoke-w2/solpi` (472 с). В остальных solpi OP=0.
- **`epr_records=0`, `solpi_compactions=0` во всех прогонах** — Reducer и Online Context Compact не участвовали.


## 4. Эквивалентность W2: разбор 18 vs 17
`removed_vs_base=0` во всех шести прогонах — ни один из 12 BASE-тестов не удалён. 18 vs 17 — это разное число *новых* тестов (6 против 5), а не потеря. Все прогоны `partial`: область `lock_tempdir` не обнаружена по именам; ограничение метода — покрытие по именам не видит доработку существующих тестов.


## 5. peak_context против лимита 46 112
Максимум по всем 12 прогонам: **36068**, запас **10044**. Ни один прогон не упирался в окно. Вердикт на `total_input_cum`, а не на peak: Pi компактит нативно (Ф1), peak самоограничен.


## 6. qwen repetition loop
Частота 1 из 6 W2 (17%) до защиты; 0 из 3 W2 в m3 (guard активен). Триггер — поиск отсутствующих pinning-констант. Требования к `stellwerk-dispatch`: `.stellwerk/FINDINGS.md` F-2026-09-26-01.


## 7. Методология и ограничения
- Песочница `git archive 2d572f1 | tar -x` вне репозитория, `.git` отсутствует; diff через `dir_diff.py`.
- loop-guard активен только в m3; исторические W2 (smoke-w2b, smoke-w2, pair-w2, pair-w2b) сняты без него.
- `qa-b8-v5` (W1-smoke, `af_fired=5`, −30.7%) **не засчитан** как rep1: другой `net_path`, W1.md без «Границ работы», общий `CARGO_TARGET_DIR`, worktree с `.git` на хост-репо, guard отсутствовал.
- Baseline-член W2 (`smoke-w2b`) взят как rep1 по вашему указанию, несмотря на иной механизм песочницы.


## 8. Незакрыто
`onlineContextCompact` и `evidencePreservingReducer` не мерились (выключены по условию). Риски внедрения в `stellwerk-worker-pi` не описывались.


## 9. Итог
**W1 = PASS** (−17.3% входа, AF подтверждён, эквивалентность полная). **W2 = NO_ADOPT** (успех 1/3 против 2/3). Внедрение SoL-Pi в `stellwerk-worker-pi` **не рекомендовано**.

