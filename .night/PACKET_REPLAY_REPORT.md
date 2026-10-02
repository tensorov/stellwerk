# packet-replay — отчёт о проходе

- Проверенный commit: `148f53fac976a90c2fb1c9eede7c2241b3802d5f`
- Инструмент: `packet-replay/1.0`
- Длительность прохода: 135.3 с

## Toolchain

- cargo: `cargo 1.98.1 (797e8a9bc 2026-08-05) (Arch Linux rust 1:1.98.1-2)`
- rustc: `rustc 1.98.1 (48a229cea 2026-09-01) (Arch Linux rust 1:1.98.1-2)`
- python3: `Python 3.14.7`
- git: `git version 2.56.0`

## Что найдено в плане

Пакетов в каноне: **102**; с командой: **95**; без команды: **7**; с ошибкой разбора: **0**.
Числа фактические и меняются вместе с планом; ожидаемых значений у отчёта нет.

## Итог прохода

- Записей: **95**
- Реально выполнено: **90**
- Не выполнено: **5**

| Вердикт | Пакетов |
| --- | --- |
| `FAIL` | 5 |
| `PASS` | 85 |
| `UNSUPPORTED_COMMAND` | 5 |

## Неподдержанные формы команд — 5

| Пакет | Команда | Вердикт | Причина |
| --- | --- | --- | --- |
| INFRA/01 | `cargo xtask plan-lint` | `UNSUPPORTED_COMMAND` | не семейство cargo test |
| INFRA/02 | `python3 scripts/plan-lint.py --self-test` | `UNSUPPORTED_COMMAND` | не семейство cargo test |
| INFRA/03 | `bash scripts/worker/tests/run.sh` | `UNSUPPORTED_COMMAND` | не семейство cargo test |
| M3/01 | `cargo test -p stellwerk-gateway --test permission && cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` | `UNSUPPORTED_COMMAND` | shell-оператор или перевод строки в команде |
| M4/01 | `cargo build -p stellwerk-task` | `UNSUPPORTED_COMMAND` | не семейство cargo test |

## Пустые выборки и неоднозначные результаты — 0

Пусто.

## Ошибки команд — 5

| Пакет | Команда | Вердикт | Причина |
| --- | --- | --- | --- |
| INFRA/04 | `cargo test -p xtask` | `FAIL` |  |
| INFRA/05 | `cargo test -p xtask` | `FAIL` |  |
| INFRA/07 | `cargo test --workspace --no-fail-fast` | `FAIL` |  |
| INFRA/10 | `cargo test -p xtask --test packet_replay_execute` | `FAIL` |  |
| M4/07 | `cargo test -p stellwerk-task --test inbox` | `FAIL` |  |

## Про все пакеты

| Пакет | Команда | argv | Target | Фильтр | Selected | Passed | Failed | Exit | Timeout | Вердикт |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| INFRA/01 | `cargo xtask plan-lint` | `-` | - | - | null | - | - | None | нет | `UNSUPPORTED_COMMAND` |
| INFRA/02 | `python3 scripts/plan-lint.py --self-test` | `-` | - | - | null | - | - | None | нет | `UNSUPPORTED_COMMAND` |
| INFRA/03 | `bash scripts/worker/tests/run.sh` | `-` | - | - | null | - | - | None | нет | `UNSUPPORTED_COMMAND` |
| INFRA/04 | `cargo test -p xtask` | `cargo test -p xtask` | - | - | 44 | 10 | 4 | 101 | нет | `FAIL` |
| INFRA/05 | `cargo test -p xtask` | `cargo test -p xtask` | - | - | 44 | 10 | 4 | 101 | нет | `FAIL` |
| INFRA/06 | `cargo test -p xtask --test ledger_encoding` | `cargo test -p xtask --test ledger_encoding` | ledger_encoding | - | 2 | 2 | 0 | 0 | нет | `PASS` |
| INFRA/07 | `cargo test --workspace --no-fail-fast` | `cargo test --workspace --no-fail-fast` | - | - | 378 | 373 | 5 | 101 | нет | `FAIL` |
| INFRA/08 | `cargo test -p xtask --test packet_replay_selftest` | `cargo test -p xtask --test packet_replay_selftest` | packet_replay_selftest | - | 9 | 9 | 0 | 0 | нет | `PASS` |
| INFRA/09 | `cargo test -p xtask --test packet_replay_validate` | `cargo test -p xtask --test packet_replay_validate` | packet_replay_validate | - | 5 | 5 | 0 | 0 | нет | `PASS` |
| INFRA/10 | `cargo test -p xtask --test packet_replay_execute` | `cargo test -p xtask --test packet_replay_execute` | packet_replay_execute | - | 4 | 0 | 4 | 101 | нет | `FAIL` |
| INFRA/11 | `cargo test -p xtask --test packet_replay_aggregate` | `cargo test -p xtask --test packet_replay_aggregate` | packet_replay_aggregate | - | 8 | 8 | 0 | 0 | нет | `PASS` |
| INFRA/12 | `cargo test -p xtask --test packet_replay_run` | `cargo test -p xtask --test packet_replay_run` | packet_replay_run | - | 6 | 6 | 0 | 0 | нет | `PASS` |
| INFRA/13 | `cargo test -p xtask --test xtask_cli` | `cargo test -p xtask --test xtask_cli` | xtask_cli | - | 10 | 10 | 0 | 0 | нет | `PASS` |
| M0/01 | `cargo test -p stellwerk-registry model` | `cargo test -p stellwerk-registry model` | - | model | 8 | 8 | 0 | 0 | нет | `PASS` |
| M0/02 | `cargo test -p stellwerk-registry model` | `cargo test -p stellwerk-registry model` | - | model | 8 | 8 | 0 | 0 | нет | `PASS` |
| M0/03 | `cargo test -p stellwerk-registry cli` | `cargo test -p stellwerk-registry cli` | - | cli | 12 | 12 | 0 | 0 | нет | `PASS` |
| M0/04 | `cargo test -p stellwerk-registry cli` | `cargo test -p stellwerk-registry cli` | - | cli | 12 | 12 | 0 | 0 | нет | `PASS` |
| M0/05 | `cargo test -p stellwerk-registry add` | `cargo test -p stellwerk-registry add` | - | add | 20 | 20 | 0 | 0 | нет | `PASS` |
| M0/06 | `cargo test -p stellwerk-registry measure` | `cargo test -p stellwerk-registry measure` | - | measure | 22 | 22 | 0 | 0 | нет | `PASS` |
| M0/07a | `cargo test -p stellwerk-registry measure` | `cargo test -p stellwerk-registry measure` | - | measure | 22 | 22 | 0 | 0 | нет | `PASS` |
| M0/07b | `cargo test -p stellwerk-registry measure` | `cargo test -p stellwerk-registry measure` | - | measure | 22 | 22 | 0 | 0 | нет | `PASS` |
| M0/08 | `cargo test -p stellwerk-registry audit` | `cargo test -p stellwerk-registry audit` | - | audit | 10 | 10 | 0 | 0 | нет | `PASS` |
| M0/09 | `cargo test -p stellwerk-registry audit` | `cargo test -p stellwerk-registry audit` | - | audit | 10 | 10 | 0 | 0 | нет | `PASS` |
| M0/09b | `cargo test -p stellwerk-registry` | `cargo test -p stellwerk-registry` | - | - | 88 | 88 | 0 | 0 | нет | `PASS` |
| M0/09c | `cargo test -p stellwerk-registry` | `cargo test -p stellwerk-registry` | - | - | 88 | 88 | 0 | 0 | нет | `PASS` |
| M0/09d | `cargo test -p stellwerk-registry` | `cargo test -p stellwerk-registry` | - | - | 88 | 88 | 0 | 0 | нет | `PASS` |
| M0/09e | `cargo test -p stellwerk-core --test event_risk` | `cargo test -p stellwerk-core --test event_risk` | event_risk | - | 5 | 5 | 0 | 0 | нет | `PASS` |
| M0/09f | `cargo test -p stellwerk-core -p stellwerk-gateway -p stellwerk-store -p stellwerk-task` | `cargo test -p stellwerk-core -p stellwerk-gateway -p stellwerk-store -p stellwerk-task` | - | - | 246 | 246 | 0 | 0 | нет | `PASS` |
| M0/10 | `cargo test -p stellwerk-registry update` | `cargo test -p stellwerk-registry update` | - | update | 12 | 12 | 0 | 0 | нет | `PASS` |
| M0/11 | `cargo test -p stellwerk-registry update` | `cargo test -p stellwerk-registry update` | - | update | 12 | 12 | 0 | 0 | нет | `PASS` |
| M1/01 | `cargo test -p stellwerk-core` | `cargo test -p stellwerk-core` | - | - | 60 | 60 | 0 | 0 | нет | `PASS` |
| M1/02 | `cargo test -p stellwerk-core --test task_lifecycle` | `cargo test -p stellwerk-core --test task_lifecycle` | task_lifecycle | - | 11 | 11 | 0 | 0 | нет | `PASS` |
| M1/03 | `cargo test -p stellwerk-core --test workspace_policy` | `cargo test -p stellwerk-core --test workspace_policy` | workspace_policy | - | 8 | 8 | 0 | 0 | нет | `PASS` |
| M1/04 | `cargo test -p stellwerk-core --test data_class_routing` | `cargo test -p stellwerk-core --test data_class_routing` | data_class_routing | - | 7 | 7 | 0 | 0 | нет | `PASS` |
| M1/05 | `cargo test -p stellwerk-core --test rbac_level` | `cargo test -p stellwerk-core --test rbac_level` | rbac_level | - | 11 | 11 | 0 | 0 | нет | `PASS` |
| M1/06 | `cargo test -p stellwerk-core --test risk_level_policy` | `cargo test -p stellwerk-core --test risk_level_policy` | risk_level_policy | - | 8 | 8 | 0 | 0 | нет | `PASS` |
| M1/07 | `cargo test -p stellwerk-store` | `cargo test -p stellwerk-store` | - | - | 27 | 27 | 0 | 0 | нет | `PASS` |
| M1/08 | `cargo test -p stellwerk-store --test task_crud` | `cargo test -p stellwerk-store --test task_crud` | task_crud | - | 7 | 7 | 0 | 0 | нет | `PASS` |
| M1/09 | `cargo test -p stellwerk-store --test task_crud` | `cargo test -p stellwerk-store --test task_crud` | task_crud | - | 7 | 7 | 0 | 0 | нет | `PASS` |
| M1/10 | `cargo test -p stellwerk-store --test task_crud list_by_workspace` | `cargo test -p stellwerk-store --test task_crud list_by_workspace` | task_crud | - | 1 | 1 | 0 | 0 | нет | `PASS` |
| M1/11 | `cargo test -p stellwerk-store --test event_append` | `cargo test -p stellwerk-store --test event_append` | event_append | - | 5 | 5 | 0 | 0 | нет | `PASS` |
| M1/12 | `cargo test -p stellwerk-store --test module_store` | `cargo test -p stellwerk-store --test module_store` | module_store | - | 7 | 7 | 0 | 0 | нет | `PASS` |
| M1/13 | `cargo test -p stellwerk-core --test risk_level_policy` | `cargo test -p stellwerk-core --test risk_level_policy` | risk_level_policy | - | 8 | 8 | 0 | 0 | нет | `PASS` |
| M2/01 | `cargo test -p stellwerk-task --test taskfile` | `cargo test -p stellwerk-task --test taskfile` | taskfile | - | 4 | 4 | 0 | 0 | нет | `PASS` |
| M2/02 | `cargo test -p stellwerk-task --test taskfile` | `cargo test -p stellwerk-task --test taskfile` | taskfile | - | 4 | 4 | 0 | 0 | нет | `PASS` |
| M2/03 | `cargo test -p stellwerk-task --lib cli` | `cargo test -p stellwerk-task --lib cli` | - | cli | 12 | 12 | 0 | 0 | нет | `PASS` |
| M2/04 | `cargo test -p stellwerk-task --lib cli` | `cargo test -p stellwerk-task --lib cli` | - | cli | 12 | 12 | 0 | 0 | нет | `PASS` |
| M2/05 | `cargo test -p stellwerk-task --test lifecycle` | `cargo test -p stellwerk-task --test lifecycle` | lifecycle | - | 7 | 7 | 0 | 0 | нет | `PASS` |
| M2/05a | `cargo test -p stellwerk-store --test status_transition` | `cargo test -p stellwerk-store --test status_transition` | status_transition | - | 4 | 4 | 0 | 0 | нет | `PASS` |
| M2/05b | `cargo test -p stellwerk-store --test status_transition` | `cargo test -p stellwerk-store --test status_transition` | status_transition | - | 4 | 4 | 0 | 0 | нет | `PASS` |
| M2/06 | `cargo test -p stellwerk-task --test lifecycle` | `cargo test -p stellwerk-task --test lifecycle` | lifecycle | - | 7 | 7 | 0 | 0 | нет | `PASS` |
| M2/07 | `cargo test -p stellwerk-task --test sandbox` | `cargo test -p stellwerk-task --test sandbox` | sandbox | - | 3 | 3 | 0 | 0 | нет | `PASS` |
| M2/08 | `cargo test -p stellwerk-task --test sandbox` | `cargo test -p stellwerk-task --test sandbox` | sandbox | - | 3 | 3 | 0 | 0 | нет | `PASS` |
| M2/09 | `cargo test -p stellwerk-task --test runner` | `cargo test -p stellwerk-task --test runner` | runner | - | 15 | 15 | 0 | 0 | нет | `PASS` |
| M2/10 | `cargo test -p stellwerk-task --test runner` | `cargo test -p stellwerk-task --test runner` | runner | - | 15 | 15 | 0 | 0 | нет | `PASS` |
| M2/11 | `cargo test -p stellwerk-task --test events` | `cargo test -p stellwerk-task --test events` | events | - | 6 | 6 | 0 | 0 | нет | `PASS` |
| M2/12 | `cargo test -p stellwerk-task` | `cargo test -p stellwerk-task` | - | - | 88 | 88 | 0 | 0 | нет | `PASS` |
| M2/12b | `cargo test -p stellwerk-task --test evidence` | `cargo test -p stellwerk-task --test evidence` | evidence | - | 4 | 4 | 0 | 0 | нет | `PASS` |
| M2/12c | `cargo test -p stellwerk-task` | `cargo test -p stellwerk-task` | - | - | 88 | 88 | 0 | 0 | нет | `PASS` |
| M2/13 | `cargo test -p stellwerk-task --test runner` | `cargo test -p stellwerk-task --test runner` | runner | - | 15 | 15 | 0 | 0 | нет | `PASS` |
| M2/14 | `cargo test -p stellwerk-task --test runner` | `cargo test -p stellwerk-task --test runner` | runner | - | 15 | 15 | 0 | 0 | нет | `PASS` |
| M3/01 | `cargo test -p stellwerk-gateway --test permission && cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` | `-` | - | - | null | - | - | None | нет | `UNSUPPORTED_COMMAND` |
| M3/02 | `cargo test -p stellwerk-gateway --test permission` | `cargo test -p stellwerk-gateway --test permission` | permission | - | 9 | 9 | 0 | 0 | нет | `PASS` |
| M3/03a | `cargo test -p stellwerk-gateway --test gate` | `cargo test -p stellwerk-gateway --test gate` | gate | - | 11 | 11 | 0 | 0 | нет | `PASS` |
| M3/03b | `cargo test -p stellwerk-gateway --test gate_ask` | `cargo test -p stellwerk-gateway --test gate_ask` | gate_ask | - | 7 | 7 | 0 | 0 | нет | `PASS` |
| M3/04 | `cargo test -p stellwerk-gateway --test gate` | `cargo test -p stellwerk-gateway --test gate` | gate | - | 11 | 11 | 0 | 0 | нет | `PASS` |
| M3/05 | `cargo test -p stellwerk-gateway --test shaping` | `cargo test -p stellwerk-gateway --test shaping` | shaping | - | 6 | 6 | 0 | 0 | нет | `PASS` |
| M3/06 | `cargo test -p stellwerk-gateway --test shaping` | `cargo test -p stellwerk-gateway --test shaping` | shaping | - | 6 | 6 | 0 | 0 | нет | `PASS` |
| M3/07 | `cargo test -p stellwerk-gateway --test mount` | `cargo test -p stellwerk-gateway --test mount` | mount | - | 8 | 8 | 0 | 0 | нет | `PASS` |
| M3/08 | `cargo test -p stellwerk-gateway --test mount` | `cargo test -p stellwerk-gateway --test mount` | mount | - | 8 | 8 | 0 | 0 | нет | `PASS` |
| M3/09 | `cargo test -p stellwerk-gateway --test receipt` | `cargo test -p stellwerk-gateway --test receipt` | receipt | - | 12 | 12 | 0 | 0 | нет | `PASS` |
| M3/10 | `cargo test -p stellwerk-gateway --test receipt` | `cargo test -p stellwerk-gateway --test receipt` | receipt | - | 12 | 12 | 0 | 0 | нет | `PASS` |
| M3/11 | `cargo test -p stellwerk-gateway --test session` | `cargo test -p stellwerk-gateway --test session` | session | - | 4 | 4 | 0 | 0 | нет | `PASS` |
| M3/11a | `cargo test -p stellwerk-gateway --test session_contract` | `cargo test -p stellwerk-gateway --test session_contract` | session_contract | - | 5 | 5 | 0 | 0 | нет | `PASS` |
| M3/11b | `cargo test -p stellwerk-gateway --test session` | `cargo test -p stellwerk-gateway --test session` | session | - | 4 | 4 | 0 | 0 | нет | `PASS` |
| M3/11c | `cargo test -p stellwerk-gateway --test session_failure` | `cargo test -p stellwerk-gateway --test session_failure` | session_failure | - | 9 | 9 | 0 | 0 | нет | `PASS` |
| M3/12 | `cargo test -p stellwerk-gateway` | `cargo test -p stellwerk-gateway` | - | - | 71 | 71 | 0 | 0 | нет | `PASS` |
| M3/12a | `cargo test -p stellwerk-gateway --test session` | `cargo test -p stellwerk-gateway --test session` | session | - | 4 | 4 | 0 | 0 | нет | `PASS` |
| M3/12b | `cargo test -p stellwerk-gateway --test session_failure --test session_contract` | `cargo test -p stellwerk-gateway --test session_failure --test session_contract` | session_contract | - | 14 | 14 | 0 | 0 | нет | `PASS` |
| M3/12c | `cargo test -p stellwerk-gateway --test session_failure` | `cargo test -p stellwerk-gateway --test session_failure` | session_failure | - | 9 | 9 | 0 | 0 | нет | `PASS` |
| M3/12d | `cargo test -p stellwerk-gateway --test session_failure` | `cargo test -p stellwerk-gateway --test session_failure` | session_failure | - | 9 | 9 | 0 | 0 | нет | `PASS` |
| M3/13 | `cargo test -p stellwerk-gateway --test shaping` | `cargo test -p stellwerk-gateway --test shaping` | shaping | - | 6 | 6 | 0 | 0 | нет | `PASS` |
| M3/14 | `cargo test -p stellwerk-gateway --test receipt` | `cargo test -p stellwerk-gateway --test receipt` | receipt | - | 12 | 12 | 0 | 0 | нет | `PASS` |
| M3/15 | `cargo test -p stellwerk-gateway --test receipt` | `cargo test -p stellwerk-gateway --test receipt` | receipt | - | 12 | 12 | 0 | 0 | нет | `PASS` |
| M3/16 | `cargo test -p stellwerk-gateway --test shaping` | `cargo test -p stellwerk-gateway --test shaping` | shaping | - | 6 | 6 | 0 | 0 | нет | `PASS` |
| M3/17 | `cargo test -p stellwerk-gateway` | `cargo test -p stellwerk-gateway` | - | - | 71 | 71 | 0 | 0 | нет | `PASS` |
| M3/18 | `cargo test -p stellwerk-gateway` | `cargo test -p stellwerk-gateway` | - | - | 71 | 71 | 0 | 0 | нет | `PASS` |
| M4/01 | `cargo build -p stellwerk-task` | `-` | - | - | null | - | - | None | нет | `UNSUPPORTED_COMMAND` |
| M4/02 | `cargo test -p stellwerk-task --test permission_wire` | `cargo test -p stellwerk-task --test permission_wire` | permission_wire | - | 10 | 10 | 0 | 0 | нет | `PASS` |
| M4/03 | `cargo test -p stellwerk-task --test permission_wire` | `cargo test -p stellwerk-task --test permission_wire` | permission_wire | - | 10 | 10 | 0 | 0 | нет | `PASS` |
| M4/04 | `cargo test -p stellwerk-task --test cli_watch` | `cargo test -p stellwerk-task --test cli_watch` | cli_watch | - | 3 | 3 | 0 | 0 | нет | `PASS` |
| M4/05 | `cargo test -p stellwerk-task --test vertical` | `cargo test -p stellwerk-task --test vertical` | vertical | - | 12 | 12 | 0 | 0 | нет | `PASS` |
| M4/06 | `cargo test -p stellwerk-task --test vertical` | `cargo test -p stellwerk-task --test vertical` | vertical | - | 12 | 12 | 0 | 0 | нет | `PASS` |
| M4/07 | `cargo test -p stellwerk-task --test inbox` | `cargo test -p stellwerk-task --test inbox` | inbox | - | 9 | 8 | 1 | 101 | нет | `FAIL` |
| M4/08 | `cargo test -p stellwerk-task --test vertical` | `cargo test -p stellwerk-task --test vertical` | vertical | - | 12 | 12 | 0 | 0 | нет | `PASS` |

## Разбор registry/measure

| Фильтр | Пакетов | Selected | Passed | Failed | Строк result |
| --- | --- | --- | --- | --- | --- |
| `-` | 3 | 88 | 88 | 0 | 6 |
| `add` | 1 | 20 | 20 | 0 | 5 |
| `audit` | 2 | 10 | 10 | 0 | 5 |
| `cli` | 2 | 12 | 12 | 0 | 5 |
| `measure` | 3 | 22 | 22 | 0 | 5 |
| `model` | 2 | 8 | 8 | 0 | 5 |
| `update` | 2 | 12 | 12 | 0 | 5 |

Числа выше — измерение этого прохода, а не перенос прежнего ручного подсчёта.
Прежнее утверждение «один тест из 71» этим прогоном **не воспроизводится**: фактические selected и passed для фильтра `measure` указаны в таблице выше, и они равны между собой. Расхождение старых чисел было следствием выбора разных строк вывода, а не свойством команд.
Строка `result` считает бинарники: их несколько, и каждый даёт свою строку, поэтому число строк больше, чем число отфильтрованных тестов.

## Ограничения отчёта

- Отвечает только на вопрос «воспроизводима ли команда на этом commit». Ни «был ли исторический RED», ни «покрыт ли крейт полностью».
- `PASS` команды не доказывает, что acceptance пакета выполнен полностью: команда может проверять подмножество.
- Проход не воспроизводит историческую стадию RED автоматически. Если команда проверяла ожидаемое падение, а сегодня зелено, фактический результат и историческое ожидание показаны раздельно.
- Сумма `passed` по пакетам не есть число уникальных тестов проекта: разные команды многократно запускают одни и те же тесты.
- Числа фактические. Ожидаемых констант у отчёта нет: новый пакет обязан менять эти числа, а не опровергать зашитое значение.
