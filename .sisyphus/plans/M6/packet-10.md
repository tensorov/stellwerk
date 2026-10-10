# packet-10 — overlap_waiver в пакетах M5/21, M5/22, M5/23
plan_id: M6
status: выполнен
goal: GREEN — правило C9 смонтировано в тела пакетов, а не только описано в AGENTS.md.
files_allowed: [.sisyphus/plans/M5/packet-21.md, .sisyphus/plans/M5/packet-22.md, .sisyphus/plans/M5/packet-23.md]
max_diff_lines: 200
test: grep -q overlap_waiver .sisyphus/plans/M5/packet-21.md && grep -q overlap_waiver .sisyphus/plans/M5/packet-22.md && grep -q overlap_waiver .sisyphus/plans/M5/packet-23.md
acceptance: в каждом из трёх пакетов есть строка `overlap_waiver: C9`; линтер её принимает и M5 остаётся «37 пакетов, нарушений нет».
forbidden: [добавлять больше трёх файлов, менять содержание пакетов кроме строки waiver, затрагивать пакеты M0/M3/INFRA (исторический долг), обновлять зеркало .omo/plans]
depends_on: []
mutation_threshold: 0
data_class: public
lane: docs

## Контекст

`AGENTS.md` описывает условный waiver для C9, но пакеты 21/22/23 его поле
`overlap_waiver:` не несут: правило есть, механика не смонтирована. Это ровно
состояние «описано, но не проверяется».

## Что сделать

1. По одной строке `overlap_waiver: C9` в тела пакетов 21, 22, 23.
2. Ничего кроме этой строки в пакетах не меняется — они уже исполнены, и I9 запрещает
   дописывать задним числом как будто проверка была раньше.

## Почему именно три файла

Первая редакция этого пакета требовала шесть файлов (три пакета M5 + `CONTEXT_BUDGET.md`
+ `FINDINGS.md` + `AGENTS.md`), что превышает лимит ADR-0003 в три файла. Две остальные
задачи не потеряны, они переехали туда, где им место: устаревшие строки в
`docs/CONTEXT_BUDGET.md` чистит **packet-07**, который уже владеет этим файлом, а
формулировка F-80 («буферизация» → «наблюдаемый timeout, причина не воспроизведена»)
**уже исправлена 2026-10-06** и в этот пакет не входит вовсе.

## Проверка

Три `grep` в `test:` дают exit 0 только когда строки на месте во всех трёх пакетах.
`just plan-lint` как тест здесь не годится: он никогда не даёт 0 из-за исторического
долга C1/C8/C9 в M0/M3/INFRA, поэтому пакет с ним не завершился бы никогда.

## Приёмка

**Коммит реализации:** `0143b09`
**Тест пакета:** `grep -q overlap_waiver .sisyphus/plans/M5/packet-21.md && grep -q overlap_waiver .sisyphus/plans/M5/packet-22.md && grep -q overlap_waiver .sisyphus/plans/M5/packet-23.md` → exit 0 (ВЫПОЛНЕН)
**В main:** да, слит до 5512378.
**Примечание:** `just check` на main → exit 0 после освобождения `/tmp`.
До очистки `/tmp` прогон падал с ENOSPC — следствие F-99, а не пакета.
