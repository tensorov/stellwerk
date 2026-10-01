# packet-09 — СЕМАНТИКА cargo test: FILTER ≠ TARGET, ПРОВЕРКА ЧЕРЕЗ metadata

plan_id: INFRA
status: предложен
goal: научить инструмент различать позиционный TESTNAME-фильтр и имя test-target'а,
  и проверять существование package и target через `cargo metadata`, а не выводить имя
  файла из строки команды руками.
files_allowed: [scripts/packet_replay.py, xtask/tests/packet_replay_selftest.rs, xtask/src/main.rs]
max_diff_lines: 200
test: cargo test -p xtask --test packet_replay_selftest
acceptance: `cargo test -p xtask --test packet_replay_selftest` → 0 failed, 0 filtered out;
  `measure` в `cargo test -p stellwark-registry measure` классифицирован как FILTER,
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
   Сейчас среди приёмочных команд есть `bash scripts/worker/tests/run.sh`,
   `python3 scripts/plan-lint.py --self-test`, `cargo xtask plan-lint`,
   `cargo build -p stellwerk-task`. Ни одна из них не запускается этим пакетом.
6. Зарегистрировать в закрытой таблице `TASKS` (`xtask/src/main.rs`) ровно две задачи:
   `packet-replay` и `packet-replay-selftest`. Обе запускают `scripts/packet_replay.py` с
   режимом из закрытого enum. `xtask` не принимает ни путь, ни команду от вызывающего:
   путь к скрипту и режим зашиты в исходник, иначе вызывающий выбирал бы, что именно
   проверяется, а аттестуемость требует, чтобы список был в коде.

## Self-tests, обязательные

1. `cargo test -p pkg measure` → `measure` есть FILTER, а не target.
2. `cargo test -p pkg --test workspace_policy` → `workspace_policy` есть TARGET.
3. `--test nope` → `INVALID_TARGET`.
4. Несуществующий package → `INVALID_PACKAGE`.
5. `bash -c`, `python3`, `just`, pipeline и `&&` → `UNSUPPORTED_COMMAND`.
6. Лишние аргументы не перенаправляются в произвольный shell: команда с лишним флагом даёт
   `UNSUPPORTED_COMMAND`, а не молчаливый запуск через `sh -c`.

## Evidence

- `cargo test -p xtask --test packet_replay_selftest` → 0 failed, 0 filtered out.
- Каждая из шести проверок выше имеет отдельный тест с известным входом.
- Ни одна команда не исполнена этим пакетом.