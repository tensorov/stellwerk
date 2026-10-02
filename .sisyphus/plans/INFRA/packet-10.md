# packet-10 — ПЕСОЧНИЦА ПРОГОНА ИЗ `git archive HEAD`

plan_id: INFRA
status: предложен
goal: дать прогону acceptance-команды отдельную копию дерева репозитория, в которой нет
  `.git`, нет ссылки на рабочее дерево и нет ни одной возможности изменить исходники.
files_allowed: [scripts/packet_replay.py, scripts/packet_replay_execute.py, xtask/tests/packet_replay_execute.rs]
max_diff_lines: 200
test: cargo test -p xtask --test packet_replay_execute
acceptance: `cargo test -p xtask --test packet_replay_execute` → 0 failed, 0 filtered out;
  песочница построена из настоящего `git archive HEAD`; в ней нет `.git`; она содержит
  корень workspace и каталог крейтов; она не является symlink; она лежит вне рабочего
  дерева; рабочее дерево после прогона содержит только три файла этого пакета.
forbidden: [cargo add, правка корневого Cargo.toml, shell=True, eval, bash -c, cut, awk,
  копирование `.git` в песочницу, песочница внутри рабочего дерева, правка packet-файлов,
  переписывание исторического текста леджера]
depends_on: [09, 11]
data_class: public
lane: infra/replay

## Контекст

Прогон acceptance-команды в основном checkout — это право испортить то, что потом
аттестуется. Значит исполнять её там нельзя вовсе.

`cp -r` для копии не годится: он тащит `.git` целиком, и команда получает возможность
дописать в историю. `git archive HEAD` выгружает только отслеживаемые файлы, поэтому
история не попадает в песочницу физически, а не по забывчивости.

## Что сделать

1. Создать каталог вне рабочего дерева через `tempfile.mkdtemp(prefix="stellwerk-replay-")`.
2. Выгрузить `git archive --format=tar HEAD` и распаковать его в этот каталог.
3. Напечатать путь машиночитаемо: `SANDBOX=<путь>`. Разбирать путь из прозаического
   вывода нельзя — вызывающий не должен вычислять то, что инструмент уже знает.
4. Ни одной внешней командой не пользоваться shell-строкой: только список аргументов.
5. Каталог после распаковки не убирать: уборка — дело вызывающего, потому что только он
   знает, когда проверка закончилась.

## Границы пакета

Пакет выделен по предмету: песочница — это про извлечение состояния репозитория, а не про
исполнение. Дальше по цепочке:

- **packet-12** — исполнитель: строгий argv, последовательный прогон, таймаут, discovery
  через libtest `--list`, и `selected_by_list` как отдельное состояние;
- **packet-13** — отчёт `.night/PACKET_REPLAY_REPORT.md` и `.night/packet-replay.json`
  и регистрация задач `packet-replay` и `packet-replay-selftest` в `TASKS`.

Пакет разделён не по размеру, а по лимиту ADR-0003: три файла на пакет, логика ≤200
логических строк на файл, тесты ≤250 полных. Совмещённый пакет не помещался ни в одну из
этих границ.

## Self-tests, обязательные

1. Песочница построена, в ней нет `.git`.
2. В песочнице есть корень workspace (`Cargo.toml`) и каталог `crates/`.
3. Песочница — каталог, а не symlink.
4. Песочница лежит вне рабочего дерева.

## Evidence

- `cargo test -p xtask --test packet_replay_execute` → 0 failed, 0 filtered out.
- Наборы предыдущих пакетов не сломаны: `packet_replay_selftest` 9, `packet_replay_validate`
  5, `packet_replay_aggregate` 8 — все 0 failed.
- Ни одна acceptance-команда этим пакетом не исполнена: выполняются только `git archive`
  и `tar` над выгрузкой.