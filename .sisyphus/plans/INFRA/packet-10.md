# packet-10 — ИСПОЛНЕНИЕ В ПЕСОЧНИЦЕ, АГРЕГАЦИЯ И ОТЧЁТ

plan_id: INFRA
status: предложен
goal: исполнить только валидно распознанные acceptance-команды в отдельной песочнице,
  просуммировать ВСЕ строки `test result` и выпустить отчёт, который отвечает ровно на
  вопрос «воспроизводима ли команда на текущем HEAD».
files_allowed: [scripts/packet_replay.py, xtask/tests/packet_replay_selftest.rs]
max_diff_lines: 200
test: cargo test -p xtask --test packet_replay_selftest
acceptance: `cargo test -p xtask --test packet_replay_selftest` → 0 failed, 0 filtered out;
  `cargo xtask packet-replay` создаёт `.night/PACKET_REPLAY_REPORT.md` и
  `.night/packet-replay.json`; основной checkout не изменён (HEAD и status совпадают до и
  после).
forbidden: [cargo add, правка корневого Cargo.toml, shell=True, eval, bash -c,
  исполнение acceptance-команд в основном checkout, параллельный запуск, правка packet-файлов
  на основе отчёта, создание PolyForge markers задним числом, выводы об историческом RED или
  coverage, переписывание исторического текста леджера]
depends_on: [08, 09]
data_class: public
lane: infra/replay

## Контекст

Разбор и семантика готовы; осталось исполнить и **не обмануть себя на агрегации**. Ручной
опыт уже показал все три способа это сделать: взять первую строку `test result` (теряются
остальные бинарники), назвать зелёным прогон с нулём выполненных тестов, и сложить выводы
разных прогонов в одну цифру.

## Что сделать

1. **Песочница, а не основной checkout.** Для каждого пакета: свежая копия через
   `git archive HEAD` в `/tmp/stellwerk-replay-*`. В песочнице нет `.git` и нет symlink на
   основной репозиторий. CWD команды — корень песочницы. Это гарантирует, что прогон
   acceptance-команды не может изменить рабочее дерево, и после прогона HEAD/status
   основного репозитория сверяются с исходными.
2. **Строго argv.** `subprocess.run` со списком аргументов, `shell=False`. Последовательно,
   без параллели. Timeout 300с по умолчанию.
3. **Discovery перед точным запуском:** эквивалент той же cargo test с libtest `--list`, чтобы
   знать `selected_by_list`. Discovery не заменяет фактический прогон, а сообщает, сколько
   тестов команда вообще собирается выполнить — это и есть разница между «зелёным» и
   «проверенным».
4. **Агрегация по ВСЕМ строкам `test result`.** Парсить каждую, суммировать `passed`,
   `failed`, `ignored`, `measured`, `filtered`. Считать `result_lines`. Ни первую, ни последнюю
   строку отдельно.
5. Общий run-local `CARGO_TARGET_DIR` **вне исходного дерева**, очищаемый после отчёта:
   переиспользование ускоряет 96 прогонов, а дерево остаётся чистым.
6. **Классификация строго по брифу.** `filtered_total > 0` при `passed_total > 0` — это PASS,
   а не дефект: `22 passed; 49 filtered` означает, что команда отбирала подмножество, и это
   законно. Дефектом это становится только при `passed_total == 0`.
7. Отчёт: total, статусы разбора, статусы валидности, PASS/FAIL/INCONCLUSIVE, разбивка по
   milestone, полный список каждого не-PASS с причиной, точная статистика `registry measure`.

## Границы, которые пакет не пересекает

- Ответ отчёта — только «воспроизводима ли команда на текущем HEAD». Не «был ли исторический
  RED», не «покрыт ли крейт полностью».
- `NO_COMMAND` не делает milestone незакрытым.
- Отчёт **не правит** packet-файлы. Каждый дефект — FINDING с packet id и машинным
  результатом.
- Маркеры PolyForge задним числом не создаются.

## Self-tests, обязательные

1. Первая строка `0 passed; 49 filtered`, вторая `22 passed; 0 filtered` → PASS,
   `passed_total == 22`.
2. Только `0 passed; 49 filtered` → INCONCLUSIVE_ZERO_EXECUTED.
3. `22 passed; 49 filtered` → PASS.
4. Одна строка FAILED среди нескольких ok → FAIL.
5. Exit 101 без строки результата → FAIL.
6. Exit 0 без строк результата → INCONCLUSIVE_NO_RESULT.
7. Несколько бинарников суммируются, а не перезаписывают друг друга.
8. Timeout → FAIL с признаком timeout.

## Evidence

- `cargo test -p xtask --test packet_replay_selftest` → 0 failed, 0 filtered out.
- Регрессия на реальной команде `cargo test -p stellwerk-registry measure`: команда
  распознана как cargo test, `measure` распознан как FILTER, package существует,
  `result_lines >= 1`, `passed_total > 0`, `failed_total == 0`, итог PASS. Ожидаемые числа
  **не хардкодить** — записать измеренные.
- Регрессия на `cargo test -p stellwerk-core --test workspace_policy`: target найден через
  metadata, выполнен ≥1 тест.
- `HEAD` и `git status` основного репозитория совпадают до и после replay.
- `.night/PACKET_REPLAY_REPORT.md` и `.night/packet-replay.json` созданы.