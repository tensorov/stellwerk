# packet-09 — СЕМАНТИКА cargo test: FILTER ≠ TARGET, ПРОВЕРКА ЧЕРЕЗ metadata

plan_id: INFRA
status: предложен
goal: научить инструмент различать позиционный TESTNAME-фильтр и имя test-target'а,
  и проверять существование package и target через `cargo metadata`, а не выводить имя
  файла из строки команды руками.
files_allowed: [scripts/packet_replay.py, scripts/packet_replay_validate.py, xtask/tests/packet_replay_validate.rs]
max_diff_lines: 200
test: cargo test -p xtask --test packet_replay_validate
acceptance: `cargo test -p xtask --test packet_replay_validate` → 0 failed, 0 filtered out;
  `cargo test -p xtask --test packet_replay_selftest` → 0 failed (разбор из packet-08 не сломан);
  `measure` в `cargo test -p stellwerk-registry measure` классифицирован как FILTER,
  `workspace_policy` в `--test workspace_policy` — как TARGET; несуществующие package и
  target дают INVALID_PACKAGE и INVALID_TARGET; ни одна команда не исполнена.
forbidden: [cargo add, правка корневого Cargo.toml, cut, awk, shell=True, eval, bash -c,
  вывод имени файла из строки команды руками, переписывание исторического текста леджера,
  правка packet-файлов]
depends_on: [08]
data_class: public
lane: infra/replay

## Контекст

Здесь живёт ловушка, из-за которой «зелёный» вывод не значит «проверено»:

```
cargo test -p stellwerk-registry measure
```

`measure` — **позиционный TESTNAME-фильтр**, а не имя бинарника. Cargo запускает все
тесты крейта, отбирая имя, содержащее `measure`. Если такого теста нет, не выбирается
**ничего**, команда завершается `exit 0`, и это выглядит как успех. Именно поэтому ручные
«22 passed» и «1 из 71» противоречат друг другу: первое число — из одной строки вывода, второе
— из другой строки того же вывода.

И наоборот:

```
cargo test -p stellwerk-core --test workspace_policy
```

Здесь `workspace_policy` — имя **integration-test target'а**, и его существование обязано
быть проверено, а не предположено.

## Что сделать

1. Разбор argv через `shlex.split`. Без `shell=True`, без eval, без склейки строки.
2. Поддержать и **различать**: `-p`/`--package`, `--lib`, `--bin NAME`, `--bins`,
   `--test NAME`, `--tests`, `--doc`, позиционный TESTNAME-фильтр, аргументы после `--`.
   Позиционный фильтр и `--test` — разные сущности, и путать их нельзя.
3. Проверка package и target через `cargo metadata --no-deps --format-version 1`. Это
   единственный источник правды о том, что существует; имя файла из строки команды не
   выводится руками — именно этот приём дал ложную находку про M1.
4. Статусы: `VALID_COMMAND`, `INVALID_PACKAGE`, `INVALID_TARGET`, `UNSUPPORTED_COMMAND`,
   `PARSE_ERROR`.
5. Всё, что не семейство `cargo test`, — `UNSUPPORTED_COMMAND`, и **не исполняется**.

## Границы пакета

Пакет разделён по смысловой границе, а не по размеру файла: смешивать разбор команд с
арифметикой прогона нельзя, иначе невозможно проверить первое, не запустив второе.

- **этот пакет** — семантика команды: `scripts/packet_replay_validate.py` плюс её тесты;
- **packet-11** — арифметика прогона: суммирование строк `test result` и вердикт;
- **packet-10** — sandbox, исполнение, отчёт и регистрация задач в `xtask/src/main.rs`.

Разделение вынужденное, а не вкусовое: ADR-0003 отделяет бюджет логики (≤200 логических
строк на файл) от бюджета тестов (≤250 полных строк на файл) и держит лимит в 3 файла на
пакет. Семантика и арифметика вместе давали 427 логических строк в одном файле и 312
тестовых в другом — двойное превышение по обоим бюджетам сразу.

## Self-tests, обязательные

1. `cargo test -p pkg measure` → `measure` есть FILTER, а не target.
2. `cargo test -p pkg --test workspace_policy` → `workspace_policy` есть TARGET.
3. `--test nope` → `INVALID_TARGET`.
4. Несуществующий package → `INVALID_PACKAGE`.
5. `bash -c`, `python3`, `just`, pipeline и `&&` → `UNSUPPORTED_COMMAND`.
   Лишние аргументы не перенаправляются в произвольный shell: команда с лишним флагом
   даёт `UNSUPPORTED_COMMAND`, а не молчаливый запуск через `sh -c`.

## Evidence

- `cargo test -p xtask --test packet_replay_validate` → 0 failed, 0 filtered out.
- Каждая из пяти проверок выше имеет отдельный тест с известным входом.
- Разбор из packet-08 не сломан: `packet_replay_selftest` → 0 failed, файл побайтно
  не тронут этим пакетом.
- Ни одна команда не исполнена этим пакетом.