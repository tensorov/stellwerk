# M4 — ПЕРВЫЙ ВЕРТИКАЛЬНЫЙ СРЕЗ

> Контент пакетов — в `.sisyphus/plans/M4/`. Все пакеты `data_class: public`.
> Зависимость: M1 закрыт, M3-04 смержён (`bd3a20d`), **M2-05…12 не сделаны**.

## Зачем этот milestone

До M4 federation умела описывать и проверять, но не выполнять. M3-04 дал
`decide`, и он зелёный — и при этом **не вызывается ниоткуда**: `stellwerk-task`
не зависит от `stellwerk-gateway` (F-10, CRITICAL). M4 — milestone, где это
закрывается и где один игрушечный Task реально проходит
`apply → sandbox → worker exec → tool call → evidence → gate → export`
до статуса `Done` с верифицированным `ToolAttestation`.

**M4 — не «ещё фичи». M4 — проверка достижимости.** Метрика успеха milestone не
«все тесты зелёные», а «запрещённый вызов не дошёл до worker, а разрешённый дошёл
и оставил чек».

## Жёсткая внешняя зависимость

`runner.rs`, `events.rs`, `evidence.rs`, `lifecycle.rs` **не существуют** — их
создают M2-05…12. Поэтому:

| пакет | можно начать | почему |
|---|---|---|
| packet-01 | **сейчас** | `contract-change` на `Cargo.toml`; ни одного файла M2 не касается |
| packet-04 | после M2-12 | `watch` читает `EventStore` |
| packet-02…08 | после M2-05…12 | лежат на `runner`/`events`/`evidence` |

Раздавать пакеты 02+ до закрытия M2 бессмысленно: они указывают на файлы, которых
нет. Это не «блокер по времени», а неисполнимость по существу.

## Карта packet → lane → depends_on → test

| packet | lane | depends_on | test | состояние |
|---|---|---|---|---|
| packet-01 | m4/wire | — | `cargo build -p stellwerk-task` | contract-change, закрывает F-10 |
| packet-02 | m4/authorize | 01 | `--test permission_wire` | RED: раннер обязан спрашивать |
| packet-03 | m4/authorize | 02 | `--test permission_wire` | GREEN: `authorize` = `decide` |
| packet-04 | m4/cli | M2-12 | `--test cli_watch` | GREEN: `watch` |
| packet-05 | m4/vertical | 03, M2-12 | `--test vertical` | RED: чек на **каждый** вызов |
| packet-06 | m4/vertical | 05 | `--test vertical` | GREEN: цепочка evidence |
| packet-07 | m4/approval | 03 | `--test inbox` | GREEN: Ask → Suspended + inbox |
| packet-08 | m4/vertical | 04, 06, 07 | `--test vertical` | GREEN: **happy path** до Done |

## Где параллелизм безопасен

Один worktree — один агент. Внутри milestone пакеты строго последовательны, кроме
этих пар, которые не делят ни одного файла:

```
Lane m4/wire + m4/cli:
  packet-01  [task/Cargo.toml, Cargo.lock]        ─┐ разные файлы,
  packet-04  [task/src/cli.rs, tests/cli_watch.rs] ─┘ без общих → можно вместе

Lane m4/authorize (01 → 02 → 03), Lane m4/vertical (05 → 06 → 08)
и Lane m4/approval (07):
  05 и 07 разведены по файлам — runner.rs против approval.rs — и потому
  параллельны. Это выяснилось по линтеру, а не по замыслу: первая версия
  держала approval в runner.rs, C1 справедливо указал, что 05 и 07 делят файл,
  и параллелизм был бы фиктивным. Разведение по файлам — не косметика, оно
  делает утверждение о параллелизме проверяемым.
  08 ждёт и 04, и 06, и 07, поэтому 08 всегда последний.
```

**Нельзя** параллелить 02 и 03: оба трогают `runner.rs`, и они в одной цепочке
через `depends_on`.

## Непересекающиеся с активными lane

M4 не трогает `stellwerk-gateway`. Все пакеты M4 — в `stellwerk-task` и его
тестах. Значит M4 и M3-05…12 параллельны безопасно, если брать разные worktree.

## Что M4 обязан доказать тестами

1. **F-10 закрыт:** запрещённый вызов не доходит до worker. Не «decide вернул
   Deny», а «worker не запущен, чек записан, в чеке — причина».
2. **`Unclassified` → `Deny`**, а не `Allow` и не молчаливый `Skip`.
3. **`Ask` → `Suspended`**, и в БД задача имеет статус `Suspended`, а не `Failed`
   (остаток F-11: различение обязано дойти до `TaskStatus`).
4. **timeout → `Failed(approval_timeout)`**, и это **отличимо** от отказа по
   политике в статусе и в evidence.
5. **`R5` никогда не suspend'ится** — безусловный `Deny` даже при `L5Owner`.
6. **happy path:** один Task доходит до `Done` с верифицированным
   `ToolAttestation` и закрытым gate.
7. **чек пишется на каждый вызов, включая запрещённый** — иначе «тихие» попытки
   не видны в расследовании.

## Предикат приёмки milestone

```bash
cargo test -p stellwerk-task
```

`0 failed`. Плюс ручной прогон вертикального среза одним Task, зафиксированный
как `ToolAttestation` (PolyForge), и `gate_evaluate passed`.
