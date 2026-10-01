# packet-07 — ИСПОЛНЕН 2026-10-01, marker 36be0be, F-08 closed. Text below is kept as it was at distribution.

plan_id: INFRA
status: выполнен
goal: перевести pin модуля `polyforge` на коммит с записью `cargo xtask` в allowlist, чтобы
  `worker-tests` и `plan-lint` стали аттестуемыми через `evidence_verify` — и закрыть F-08.
files_allowed: [modules.toml, modules.lock]
preconditions:
  - владелец смержил PR в tensorov/polyforge и выпустил тег 0.4.1;
  - `polyforge-cli --version` и `polyforge-mcp` на хосте отдают 0.4.1;
  - запись `cargo xtask` присутствует в allowlist **этого** тега, а не только в ветке PR.
test: cargo test --workspace --no-fail-fast
forbidden: [ручная правка modules.lock, cargo add, правка корневого Cargo.toml, добавление
  polyforge-зависимости в crates/*, ослабление существующих тестов, #[allow], #[ignore]]

**Помечен contract-change.** ADR: `docs/adr/0005-event-risk-three-states.md` не применим; здесь
основание — аудит в `.night/STATE.md` (раздел «F-08 — аудит моста `cargo xtask`»).

## Поправка к исходному заданию: pin — это git rev, а не версия

Задание предлагало «bump версии». Измерено: в `modules.toml` поля версии **нет вообще**.

```
modules.toml:14  [module.polyforge]
modules.toml:18  source   = { git = "...", rev = "71c295c6f29546743a2c1fe43a64a04a..." }
modules.toml:19  command  = "polyforge-mcp"
modules.toml:23  update   = { channel = "tag", policy = "pr" }
```

Значит «0.4.0 → 0.4.1» — это **замена `rev` на коммит слияния**, а не правка строки версии.
Пакет записан под фактический механизм, потому что bump версии в этом файле нечего делать.

## Поправка к исходному заданию: `modules.lock` генерируется

```
modules.lock:1  # Генерируется `stellwark module measure` / `stellwerk module update`. Руками не править.
```

`modules.lock` стоит в `files_allowed` только потому, что он **меняется** вместе с `modules.toml`,
но править его руками запрещено отдельно. Обновление идёт через `stellwerk module update`.

## Шаги

1. Убедиться, что новый `rev` действительно содержит запись allowlist, **до** правки:
   ```bash
   git clone https://github.com/tensorov/polyforge.git /tmp/pf-check
   git -C /tmp/pf-check checkout <NEW_REV>
   grep -c 'cargo xtask' /tmp/pf-check/crates/polyforge-toolrunner/src/runner.rs
   ```
   Ожидается ≥ 1. Ноль означает, что тег указывает не на тот коммит, и пакет останавливается.
2. Подставить `<NEW_REV>` в `modules.toml:18`.
3. `stellwerk module update` — перегенерировать `modules.lock`.
4. `stellwerk module measure polyforge` — пересчитать `context_cost_tokens`. Схема MCP не
   меняется, поэтому ожидается прежнее `813`; расхождение означает, что изменение затронуло
   больше, чем allowlist, и требует отдельной приёмки.

## Приёмка

- `cargo xtask worker-tests` через `evidence_verify` → `Verified`, `exit 0`.
- `cargo xtask plan-lint-selftest` через `evidence_verify` → `Verified`, `exit 0`.
- `cargo xtask rm-rf-slash` → exit 2; `--exec=/bin/false` не исполняется
  (регрессия `extra_argument_is_not_forwarded_to_the_child`).
- `gate_evaluate` для `worker-pipeline` → `passed: true`.
- `cargo test --workspace --no-fail-fast` → 0 failed, число тестов не уменьшилось.
- F-08 помечен `RESOLVED`; evidence для `worker-pipeline-hardening` обновлён.

## Связанные находки

F-08 (мост аттестации), F-69 (allowlist = перечисление, не семейство), F-77 (allowlist прочитан
из исходника 0.4.0; записи `cargo xtask` в опубликованной 0.4.0 нет — ноль вхождений), F-76
(устаревшая очередь: пакеты числятся открытыми, а закрыты).

Contract-Change: polyforge pin 0.4.1

## Исполнено 2026-10-01 — маркер `36be0be`, F-08 закрыт

**Шаги.** Шаг 1 выполнен до правки `modules.toml`: тег `v0.4.1` указывает на deref-коммит
`b81f5848d5131d730f72d67e81db50f7377ab0c7`, и запись `cargo xtask` присутствует в нём самом
(`crates/polyforge-toolrunner/src/runner.rs:151-154`, allowlist вырос с 16 до 17 записей), а не
только в ветке PR. Шаг 2 — коммит `965b393`. Шаг 4 — `stellwark module measure polyforge` вернул
`exit 0`, `schema_bytes=4034`, `context_cost_tokens=813` без изменений, то есть схема MCP не
изменилась и `modules.lock` содержательно правильный.

**Шаг 3 завершился `exit 1`, и это не про pin.** Инструмент дошёл до `gh pr create` и упал: в
репозитории нет git remote. Сборка крейта и MCP handshake прошли, измерение состоялось.

**`modules.lock` не менялся намеренно.** Инструмент положил в `.stellwerk/pending/polyforge.patch`
diff, в котором строки `15-20` и `21-26` **идентичны** построчно: изменение касается только
невидимых символов. Применение такого diff дало бы ложную правку, поэтому пакет его не применял.

**Предусловие 2 оказалось невыполнимым, и это дефект пакета.** Оно требовало
`polyforge-cli --version == 0.4.1`, но такого флага нет: CLI отвечает `unknown command`, а
`polyforge-mcp --version` требует `initialize`. Проверка «отдаёт ли бинарь версию» была
неисполнима в принципе, и исполнитель не смог бы закрыть это предусловие. Фактическая версия
устанавливается **самим фактом прохождения `evidence_verify`**, то есть приёмкой, а не
предусловием. Записано здесь, чтобы пакет не вводил в заблуждение при повторном чтении.

**Граница применимости pin требует перезапуска процесса.** Первая попытка `evidence_verify` после
правки pin вернула `tool "cargo xtask" is not on the allowlist`, хотя запись в теге была и pin
стоял верно: процесс `polyforge-mcp` загрузил код до установки 0.4.1 и держит снимок в памяти.
Правка в репозитории не влияет на уже стартовавший процесс. После перезапуска обе аттестации
прошли: claim 137 → 138 (`cargo xtask worker-tests`) и claim 139 → 140
(`cargo xtask plan-lint-selftest`), обе `Verified` с exit 0, гейт `passed: true`.

**Побочная находка, к 0.4.1 не относящаяся.** Прежний rev `71c295c6f29546743a2c1fe43a64a04a07c9516c`
отсутствует в upstream: `upload-pack: not our ref` при полном сорокасимвольном SHA. То есть
`modules.toml` указывал на несуществующий коммит. Записано как наблюдение, правки не требует.
