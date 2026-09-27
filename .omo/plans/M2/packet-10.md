# packet-10
plan_id: M2
goal: Реализовать runner: запуск Pi в песочнице, разбор `--mode json`, вычисление verdict.
files_allowed: [crates/stellwerk-task/src/runner.rs, crates/stellwerk-task/src/sandbox.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --test runner
acceptance: `cargo test -p stellwerk-task --test runner` → все тесты runner зелёные.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [09]
data_class: public
lane: m2/runner

## Контекст

- **Новый крейт `stellwerk-task`.** Ничего из `stellwerk-core` и `stellwerk-store`
  не редактируется: крейт только вызывает их публичные API.
- Крейт зависит от `stellwerk-core` (контракт) и `stellwerk-store` (трейты) по path.
  Зависимости — только из `[workspace.dependencies]`. `cargo add` запрещён.
- Команда kubectl-shaped: `stellwerk task apply|get|watch|cancel|logs`.
  Один глагол — одно действие; комбо-команд нет.
- Задача описывается TOML-файлом (`toml` уже в workspace) — YAML-крейт не нужен.
- Статусы ведутся ЧЕРЕЗ предикаты ядра (`TaskStatus::can_transition_to`). Не заводи
  собственный свод переходов: единственный источник истины — контракт.
- Уже закрытые пакеты M1 (packet-02…12) — их предикаты и трейты считай готовыми.

## Что сделать

1. Форма запуска Pi — ровно та, что уже работает в `~/.local/bin/stellwerk-dispatch`:
   `--provider <p> --model <m> --thinking off -p <packet> --mode json --no-prompt-templates`.
   Не изобретай новые флаги: расхождение с рабочим dispatch ломает совместимость.
2. Путь к бинарю Pi — из переменной окружения `STELLWERK_PI` (то же имя, что в dispatch).
   НИКОГДА не пиши api-key в аргументы и не читай `auth.json`: ключ берёт сам Pi.
3. Таймаут — `spec.limits.timeout_seconds`; по истечении убивай процесс и возвращай
   `StopReason::Timeout`. Убийство — через `Child::kill`, НЕ через `kill -9` в shell.
4. Разбор потока: `type == "message_update"` → usage (input/output),
   `type == "turn_end"` → turns++ и tool_steps += число элементов в `toolResults`,
   `message.stopReason` → StopReason. Неизвестные типы событий игнорируй, а не падай:
   формат потока расширяется.
5. `verdict_for` — чистая функция, без ввода-вывода: её покрывают тесты.
6. **`verdict_for` обязан различать «отказано политикой» и «ждёт человека»** (В4).
   Сегодня у прогона два исхода — успех и неуспех. Когда придёт M3, у неинтерактивного
   Run появится третий: gateway уводит вызов в `TaskStatus::Suspended` и кладёт
   `ApprovalRequest` в `.stellwerk/inbox/<run_id>.json` (M3-04, M3-11).

   Если `verdict_for` схлопнёт «ждёт решения» в ошибку или в отказ, разрешение
   станет неотличимо от запрета — ровно тот дефект, который В4 запрещает: «Deny
   не самостоятельный исход, а дефолт по таймауту», «запрещено политикой» и «никто
   не ответил» обязаны быть различимы в статусе и в evidence. Истечение таймаута
   ожидания — это тоже **не** отказ.

   Поэтому уже сейчас заведи отдельный вариант исхода (например
   `AwaitingApproval`) и тест на него. Значения M3 он не знает и знать не должен —
   достаточно, чтобы состояние «прогон ждёт человека» не имело куда схлопнуться.
7. Никакой записи в хранилище: это packet-11.

## Evidence

- `cargo test -p stellwerk-task --test runner` → passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
