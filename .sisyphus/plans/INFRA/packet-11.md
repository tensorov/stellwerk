# packet-11 — АРИФМЕТИКА ПРОГОНА: сумма строк `test result` и вердикт

plan_id: INFRA
status: предложен
goal: научить инструмент считать вердикт прогона по всем строкам `test result` сразу и
  различать три разные причины отсутствия результата, не подменяя одну другой.
files_allowed: [scripts/packet_replay.py, scripts/packet_replay_aggregate.py, xtask/tests/packet_replay_aggregate.rs]
max_diff_lines: 200
test: cargo test -p xtask --test packet_replay_aggregate
acceptance: `cargo test -p xtask --test packet_replay_aggregate` → 0 failed, 0 filtered out;
  `cargo test -p xtask --test packet_replay_validate` → 0 failed (семантика из packet-09 не сломана);
  `cargo test -p xtask --test packet_replay_selftest` → 0 failed (разбор из packet-08 не сломан);
  три строки 5 + 7 + 10 дают 22 passed и 3 filtered, а не 10; `0 passed; 49 filtered`
  даёт INCONCLUSIVE_ZERO_EXECUTED; `exit 0` без строк результата даёт INCONCLUSIVE_NO_RESULT
  и `SELECTED_BY_LIST=null`; таймаут и ненулевой код дают FAIL.
forbidden: [cargo add, правка корневого Cargo.toml, cut, awk, shell=True, eval, bash -c,
  переписывание исторического текста леджера, правка packet-файлов]
depends_on: [09]
data_class: public
lane: infra/replay

## Контекст

Ручные числа «22 passed» и «1 из 71» противоречили друг другу не из-за ошибки подсчёта,
а из-за выбора строки: бралась то первая, то последняя `test result:` из одного и того же
вывода. Три строки одного прогона — это три разных бинарника, и сумма по ним единственный
честный итог.

Вторая ловушка — «зелёный» вывод без единого выполненного теста. `exit 0` означает, что
**процесс** завершился успешно, а не то, что что-то проверили.

## Что сделать

1. Брать **все** строки `test result:` и суммировать `passed`, `failed`, `ignored`,
   `measured`, `filtered`. Ни первую, ни последнюю отдельно.
2. `filtered > 0` сам по себе не ослабляет результат: команда, отобравшая подмножество и
   выполнившая часть, проверяла именно это подмножество.
3. `passed == 0` при ненулевом `filtered` → `INCONCLUSIVE_ZERO_EXECUTED`: ничего не
   выполнено, и «зелёный» код возврата ничего не доказывает.
4. Ненулевой код возврата и таймаут → `FAIL`, независимо от чисел в выводе.
5. `SELECTED_BY_LIST` — отдельное состояние. В этом пакете discovery не выполнялся вовсе,
   поэтому честный ответ `null`, а не `0`. Подставлять выведенное из строк число значило бы
   выдать догадку за измерение.

## Границы пакета

Пакет выделен по смыслу, а не по размеру: здесь нет ни argv, ни пакетов, ни песочницы —
только строки вывода и правило, по которому из них получается вердикт. Разбор команд
живёт в packet-09, исполнение и песочница — в packet-10.

## Self-tests, обязательные

1. Три строки `5`, `7` и `10` → `PASSED=22`, `FILTERED=3`, `RESULT_LINES=3`.
2. `0 passed; 49 filtered`, затем `22 passed` → сумма 22 и `PASS`, а не ложный ноль.
3. Только `0 passed; 49 filtered` → `INCONCLUSIVE_ZERO_EXECUTED`.
4. `exit 0` без строк результата → `INCONCLUSIVE_NO_RESULT`, `RESULT_LINES=0`.
5. `exit 101` без строк результата → `FAIL`.
6. `--timed-out` → `FAIL` даже при `3 passed` в выводе.
7. Вывод без строк результата → `SELECTED_BY_LIST=null`, и **не** `INCONCLUSIVE_ZERO_EXECUTED`.

## Evidence

- `cargo test -p xtask --test packet_replay_aggregate` → 0 failed, 0 filtered out.
- Каждая из семи проверок выше имеет отдельный тест с известным входом.
- Семантика из packet-09 не сломана: `packet_replay_validate` → 0 failed.
- Разбор из packet-08 не сломан: `packet_replay_selftest` → 0 failed.
- Ни одна acceptance-команда не исполнена этим пакетом: суммируются переданные строки.