# M2-index — карта packets

> Контент пакетов — в `.sisyphus/plans/M2/`. Все пакеты `data_class: public`.
> Зависимость: M1 закрыт (предикаты ядра и трейты хранилища GREEN).

## Зачем этот milestone

`stellwerk task apply|get|watch|cancel|logs` — kubectl-shaped интерфейс поверх
контракта и хранилища, плюс runner: песочница → Pi → события → evidence → статус.
Смысл — связать три слоя, которые пока существуют по отдельности.

## Карта packet → lane → depends_on

| packet | lane | depends_on | test | состояние |
| --- | --- | --- | --- | --- |
| packet-01 | m2/skeleton | — | `--test taskfile` | RED + contract-change (workspace) |
| packet-02 | m2/taskfile | 01 | `--test taskfile` | GREEN: загрузка TOML |
| packet-03 | m2/cli-red | 02 | `--lib cli` | RED: clap-скелет |
| packet-04 | m2/cli-green | 03 | `--lib cli` | GREEN: dispatch apply/get |
| packet-05a | m2/store-contract-red | 04, M1-08…12 | `--test status_transition` | RED: contract-change `status_of` + CAS |
| packet-05b | m2/store-contract | 05a, M1-08…12 | `--test status_transition` | GREEN: чтение статуса и условная запись |
| packet-05 | m2/lifecycle-red | 05b | `--test lifecycle` | RED: статусы |
| packet-06 | m2/lifecycle | 05 | `--test lifecycle` | GREEN: advance (3 аргумента) |
| packet-07 | m2/sandbox-red | 06 | `--test sandbox` | RED: песочница |
| packet-08 | m2/sandbox | 07 | `--test sandbox` | GREEN: scripts/worker/sandbox.sh |
| packet-09 | m2/runner-red | 08 | `--test runner` | RED: разбор потока Pi |
| packet-10 | m2/runner | 09 | `--test runner` | GREEN: запуск Pi |
| packet-11 | m2/events-red | 10 | `--test events` | RED: события прогона |
| packet-12 | m2/evidence | 11 | `cargo test -p stellwerk-task` | GREEN: события + цепочка evidence |

## Волны

```
01 → 02 → 03 → 04        — скелет, формат файла, CLI, dispatch
05a → 05b                 — contract-change трейта: читать статус, писать условно
05  → 06                  — жизненный цикл статусов
07 → 08                   — песочница
09 → 10                   — runner
11 → 12                   — события и evidence (сердце закона)
```

Пакеты **строго последовательны**: крейт новый, файлов мало, каждый следующий
опирается на предыдущий. Параллельная раздача бессмысленна и конфликтна.

## Точки конфликта

1. `Cargo.toml` (корень) — правится ТОЛЬКО в packet-01 (contract-change, метка в goal).
2. `crates/stellwerk-task/src/lib.rs` — правится в packet-01 (создание) и в 09, 11, 12
   (добавление `pub mod`). Добавлять **после** существующих `pub mod`, не в середину.
3. `src/cli.rs` — пакеты 03 и 04. При RED→GREEN追加 не переставляй чужие блоки.
4. `scripts/worker/` — **не трогать ни одним пакетом**. Песочница вызывается, не меняется.
   `~/.local/bin/stellwerk-dispatch` — то же самое.

## Инварианты, которые пакеты обязаны сохранить

- статусы ведутся ТОЛЬКО через `TaskStatus::can_transition_to` (единственный свод переходов);
- форма запуска Pi совпадает с `stellwerk-dispatch` (иначе сломается совместимость);
- ни одного вызова `rm` в крейте; уборкой занимается `sandbox.sh`;
- ключи не читаются и не печатаются;
- gate проходит ТОЛЬКО при `Verdict::Valid` + наличии ToolAttestation + Validation.

## Предикат приёмки milestone

```bash
cargo test -p stellwark-task
```

0 failed. Плюс ручная проверка: `apply` на temp-базе печатает `applied <id>`, exit 0.
