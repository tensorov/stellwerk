# RESULT — control-run 20260927-004804

**Этот файл написан интегратором, а не воркером.** Воркер `RESULT.md` не создал:
в формате dispatch вердикт лаунчера уходит в `validity.json`, а не в отдельный отчёт
пакета. Записано по фактическим артефактам прогона, без домысливания.

## Прогон

| поле | значение | источник |
|---|---|---|
| packet | `M1/packet-03` (`NetworkPolicy::allows`) | packet.md |
| lane | m1-core-net | лог диспатча |
| база | `cf120fdbbdd95763512f879d9106feffc0401bd0` (throwaway-база ночной сессии #1) | лог |
| sandbox | `/tmp/stellwerk-sandbox/m1-core-net-packet-20260927-004804` (.git отсутствовал) | sandbox.sh |
| VALIDITY GATE | **VALID** | validity.json |
| pi_exit / stop / timeout | 0 / `stop` / false | validity.json |
| tool_steps | 9 | validity.json |
| пик контекста | 13 087 из 46 112 (запас 72%) | metrics.json |

## Приёмка (выполнена интегратором, не по отчёту воркера)

```
cd /tmp/stellwerk-sandbox/m1-core-net-packet-20260927-004804
cargo test -p stellwerk-core --test workspace_policy
```
→ **8 passed; 0 failed**

В `main` на момент сессии #2 тот же тест даёт **4 passed; 4 failed** — контрольный
прогон **не был смержен**, веток `m1/*` нет.

## Дисциплина пакета

- дифф затрагивает РОВНО один файл `crates/stellwerk-core/src/contract.rs`
  (совпадает с `files_allowed`);
- `+24/−2` при бюджете 200 строк;
- `sandbox.files.base` == `sandbox.files.new` — ни один файл не создан и не удалён;
- главный репозиторий не тронут (`files_changed=0`);
- ослабления тестов не было: изменён только `contract.rs`, ни один `assert!` не тронут.

## Статус относительно main

**NOT MERGED.** Решение осталось в песочнице и было переиграно заново в Фазе 6 сессии #2.
Этот каталог — evidence того, что qwen-совместимые пакеты закрываются (подтверждено),
а не источник готового кода.

## Полный поток событий

`run.jsonl` (484K, 824 события) не скопирован в репозиторий намеренно: он весит
больше, чем вся остальная evidence вместе взятая, и воспроизводим из песочницы.
Сводные метрики — в `metrics.json`. Оригинал:
`/tmp/stellwerk-sandbox/m1-core-net-packet-20260927-004804.run/run.jsonl`
