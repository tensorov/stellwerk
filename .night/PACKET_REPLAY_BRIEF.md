# PACKET_REPLAY — бриф

Дата: 2026-10-01
Роль: инженер верификации Stellwerk

## Проблема

Три ручных аудита подряд дали ложные результаты:

1. `cut -c1:44` обрезал `workspace_policy` до `workspace`;
2. awk прочитал только первую строку `test result:` из нескольких test binaries;
3. переменная с `\n` попала под glob-expansion zsh.

Текущий ручной вывод противоречив: `cargo test -p stellwerk-registry measure` описан
одновременно как «22 passed, 49 filtered out» и как «1 выполненный тест из 71». Ни одна
из этих цифр не принимается как факт до packet-replay.

## Цель

Для каждого канонического packet определить:

- есть ли поле `test:`;
- можно ли безопасно разобрать команду;
- существует ли указанный package/target;
- сколько тестов реально выбрано и выполнено;
- прошла ли команда на текущем HEAD;
- PASS, FAIL или INCONCLUSIVE.

Packet-replay проверяет **текущую** воспроизводимость acceptance-команды. Он НЕ доказывает
исторический RED и НЕ создаёт evidence-маркеры задним числом.

## Источник истины

Только `.sisyphus/plans/**/packet-*.md`. Не читать `.omo/plans/` — это зеркало. Пропускать
`_archive/`.

Ориентиры «около 96 / 89 / 7» — **исходная гипотеза, не хардкод**. Итоговые числа даёт
инструмент.

## Фаза 1 — inventory и парсер

`scripts/packet_replay.py` — Python stdlib, без новых зависимостей. Фиксированные задачи
`cargo xtask packet-replay` и `packet-replay-selftest`. xtask не принимает произвольный путь
или команду от вызывающего — пути и режимы в закрытом enum/match.

Поле `test:` искать по структуре файла пакета, а не по произвольному вхождению слова `test`
в прозе или code block. Статусы: `PARSED`, `NO_COMMAND`, `PARSE_ERROR`.

Обязательные self-tests: однострочное `test:`; команда в backticks; похожее слово в prose/code
block не считается полем; пакет без `test:` → `NO_COMMAND`; `_archive/` не сканируется;
`.omo/plans/` не сканируется; `workspace_policy` сохраняется полностью; ни `cut`, ни shell
word-splitting, ни zsh glob.

## Фаза 2 — семантика `cargo test`

Версия 1 исполняет ТОЛЬКО безопасно распознанные команды семейства `cargo test`. Остальные —
`UNSUPPORTED_COMMAND`, не исполняются. Без `shell=True`, `bash -c`, eval и конкатенации.
argv через `shlex.split`.

Критично: `cargo test -p stellwark-registry measure` — здесь `measure` это TESTNAME filter, а
НЕ имя test binary. А `cargo test -p stellwark-core --test workspace_policy` — здесь
`workspace_policy` integration-test target, его существование проверяется.

Проверка targets через `cargo metadata --no-deps --format-version 1`. Имя файла из строки
команды вручную НЕ выводится.

Поддержать и различать: `-p`/`--package`, `--lib`, `--bin NAME`, `--bins`, `--test NAME`,
`--tests`, `--doc`, позиционный TESTNAME filter, аргументы после `--`.

Статусы: `VALID_COMMAND`, `INVALID_PACKAGE`, `INVALID_TARGET`, `UNSUPPORTED_COMMAND`,
`PARSE_ERROR`.

## Фаза 3 — executor в песочнице

Acceptance-команды не исполняются в основном checkout. Для каждого пакета: свежая песочница
через `git archive HEAD` в `/tmp/stellwerk-replay-*`; без `.git` и без symlink на основной
репозиторий; CWD — корень песочницы; строго argv через `subprocess.run`, `shell=False`;
последовательно, без параллели; timeout 300с по умолчанию; общий run-local `CARGO_TARGET_DIR`
вне исходного дерева, очищается после отчёта. После команды убедиться, что HEAD/status
основного репозитория не изменились.

Перед точным запуском — discovery через эквивалент cargo test с libtest `--list`. Учитывать
существующие аргументы после `--`, не собирать строку через shell. Discovery определяет число
выбранных тестов, но не заменяет фактический прогон.

## Фаза 4 — агрегация

Парсить **ВСЕ** строки `test result:`. Не брать первую или последнюю отдельно. Суммировать по
всем binaries и doc-tests: `passed_total`, `failed_total`, `ignored_total`, `measured_total`,
`filtered_total`, `result_lines`, `selected_by_list`, `exit_code`, `timeout`.

Классификация:

```
PASS:                      exit 0, failed 0, passed > 0, команда валидна
FAIL:                      exit != 0 ЛИБО failed > 0 ЛИБО timeout
INCONCLUSIVE_ZERO_EXECUTED: exit 0, failed 0, passed 0, при filtered > 0 ИЛИ selected_by_list == 0
INCONCLUSIVE_NO_RESULT:     exit 0, но нет ни одной строки test result
NO_COMMAND / INVALID_PACKAGE / INVALID_TARGET / UNSUPPORTED_COMMAND / PARSE_ERROR
```

Важно:

- `filtered_total > 0` НЕ дефект, если `passed_total > 0`.
- `22 passed + 49 filtered` → PASS.
- `0 passed + 49 filtered` → INCONCLUSIVE_ZERO_EXECUTED.
- Исторический RED packet на текущем GREEN HEAD может дать PASS; это не переписывает историю.
- Не сравнивать `passed_total` с общим числом тестов крейта как требование полного покрытия.

Self-tests на искусственных output fixtures: первая строка `0 passed; 49 filtered` и вторая
`22 passed; 0 filtered` → PASS с passed_total=22; только `0 passed; 49 filtered` →
INCONCLUSIVE_ZERO_EXECUTED; `22 passed; 49 filtered` → PASS; одна FAILED среди ok → FAIL; exit
101 без result line → FAIL; exit 0 без result lines → INCONCLUSIVE_NO_RESULT; несколько
binaries суммируются; timeout → FAIL.

## Фаза 5 — регрессия на реальной команде

Отдельный интеграционный тест на `cargo test -p stellwark-registry measure`. **Не хардкодить
ожидаемые числа.** Утверждать: команда распознана как cargo test; `measure` распознан как
filter, а не binary; package существует; `result_lines >= 1`; `passed_total > 0`;
`failed_total == 0`; классификация PASS. Если получено 22 — записать 22; другое — записать
измеренное.

Также проверить `cargo test -p stellwerk-core --test workspace_policy`: target найден через
metadata, выполнен ≥1 тест.

## Фаза 6 — полный replay и отчёт

Коммит инструмента отдельным INFRA-коммитом. Запуск `cargo xtask packet-replay` на текущем
HEAD. Создать `.night/PACKET_REPLAY_REPORT.md` и `.night/packet-replay.json`. В отчёте: total
packets; PARSED/NO_COMMAND/PARSE_ERROR; VALID/INVALID_TARGET/UNSUPPORTED; PASS/FAIL/INCONCLUSIVE;
по milestone; полный список каждого не-PASS с причиной; точная агрегированная статистика
`registry measure`.

Не исправлять packets автоматически в этой же сессии. Каждый дефект — FINDING с packet id и
машинным результатом. Не создавать PolyForge markers задним числом. Не переаттестовывать 96
пакетов. Не объявлять milestone незакрытым только из-за NO_COMMAND. Отчёт отвечает только на
вопрос: «Команда acceptance воспроизводима на текущем HEAD или нет?»

## Фаза 7 — независимое ревью

Read-only reviewer другой модели проверяет: парсер не обрезает; cargo filter не перепутан с
target; суммируются все result lines; 0 executed не становится PASS; команды не идут через
shell; main не изменялся; отчёт не делает выводов о RED/coverage, которых данные не доказывают.
Дефект → RED→GREEN тест и повтор replay.

## Приёмка

```
cargo xtask packet-replay-selftest → PASS
cargo xtask packet-replay          → отчёт создан
cargo test --workspace --no-fail-fast → GREEN
cargo fmt --all -- --check          → GREEN
cargo clippy --workspace --all-targets -- -D warnings → GREEN
git status clean
```

Evidence: ModelClaim на инструмент и отчёт; ToolAttestation на разрешённую cargo-команду, если
allowlist и xtask dispatcher допускают; `gate_evaluate`. Если конкретная команда не
поддерживается allowlist — **не подменять ручной проверкой**, записать BLOCKED_VERIFICATION.

## Запрещено

`cut` для команд и имён; awk только по первой result-строке; `shell=True`, eval, `bash -c`;
zsh glob для списка команд; позиционный TESTNAME как binary; `filtered > 0` как автоматический
дефект; менять packet-файлы по отчёту в этой же сессии; открывать/читать/удалять
`opencode.db` или `opencode.log`; push; переписывать историю; выдавать ручную классификацию за
измерение.