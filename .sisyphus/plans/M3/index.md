# M3-index — карта packets

> Контент пакетов — в `.sisyphus/plans/M3/`. Все пакеты `data_class: public`.
> Зависимость: M1 закрыт (предикаты `required_level`, `permits`, `permits_model`).

## Зачем этот milestone

Виртуальный MCP-сервер на сессию: решает `allow|ask|deny` по риску, спрашивает
человека на границе, шейпит вывод в 2k токенов, пишет чек на **каждый** вызов.
Это слой, через который агент вообще получает доступ к инструментам.

## Карта packet → lane → depends_on

| packet | lane | depends_on | test | состояние |
| --- | --- | --- | --- | --- |
| packet-01 | m3/skeleton | — | `--test permission` | RED + contract-change (workspace) |
| packet-02 | m3/permission | 01 | `--test permission` | GREEN: RiskClass → Allow/Ask/Deny |
| packet-03a | m3/gate-red | 02 | `--test gate` | RED: ось риска R0-R5 и данные |
| packet-03b | m3/gate-ask-red | 03a | `--test gate_ask` | RED: ось В2/В4: Unclassified, Suspended |
| packet-04 | m3/gate | 03a, 03b | `--test gate` + `--test gate_ask` | GREEN: decide |
| packet-05 | m3/shaping-red | 04 | `--test shaping` | RED: усечение вывода |
| packet-06 | m3/shaping | 05 | `--test shaping` | GREEN: артефакт + хеш |
| packet-07 | m3/mount-red | 06 | `--test mount` | RED: бюджет монтажа |
| packet-08 | m3/mount | 07 | `--test mount` | GREEN: жадный проход |
| packet-09 | m3/receipt-red | 08 | `--test receipt` | RED: чек вызова |
| packet-10 | m3/receipt | 09 | `--test receipt` | GREEN: чек → событие |
| packet-11a | m3/session-surface-red | 10 | `--test session_contract` | RED: контракт состояний |
| packet-11b | m3/session-routing-red | 11a | `--test session` | RED: маршрутизация |
| packet-11c | m3/session-failure-red | 11b | `--test session_failure` | RED: вопрос, истечение, сбой |
| packet-12a | m3/session-routing-green | 11b | `--test session` | GREEN: монтаж, decide, вызов, чек |
| packet-12b | m3/session-question-green | 12a, 11c | `--test session_failure` + `--test session_contract` | GREEN: вопрос, истечение, сбой, субагенты |
| packet-12c | m3/session-budget-red | 12b, 17 | `--test session_failure` | RED: усечение и калибровка (ждёт 17) |
| packet-12d | m3/session-budget-green | 12c, 17, 18, 15, 16 | `--test session_failure` | GREEN: проверка калибровки, усечение, именованный отказ |
| packet-17 | m3/session-calibration-surface | 12b, 15, 16, 06 | `cargo test -p stellwerk-gateway` | F-24: `TokenCalibration` в `Session` (4 файла, ждёт решения) |
| packet-18 | m3/receipt-honesty | 17, 12c, 15, 16 | `cargo test -p stellwerk-gateway` | F-25 (6-7) + F-30 вариант A: поля чеков называются тем, что лежит, `ReceiptError` удалён (5 файлов) |
| packet-13 | m3/public-api-test | 06 | `--test shaping` | F-14: модуль shaping публичен |
| packet-14 | m3/receipt-workspace | 10 | `--test receipt` | F-22: рабочее место чека |
| packet-15 | m3/receipt-locks | 14 | `--test receipt` | F-25: замки на регрессию чека |
| packet-16 | m3/shaping-doc-source | 15, 06 | `--test shaping` | F-28: докстринг `shape` указывает на файл без калибровки |

## Волны

```
01 → 02        — permission (contract-change в 01)
03a → 03b → 04 — gate: риск × полномочия × данные
05 → 06        — shaping: 2k токенов, артефакт, хеш
07 → 08        — mount: что вообще подключать
09 → 10        — чек
11a → 11b → 11c — RED: контракт, маршрутизация, вопрос/сбой
12a → 12b      — GREEN: маршрутизация, затем вопрос/сбой/усечение
13 → 14        — F-14 (публичный shaping), F-22 (рабочее место чека)
```

Строго последовательно. Крейт новый, каждый пакет опирается на предыдущий.

## Порядок обработки вызова (нарушать нельзя)

```
1. инструмент смонтирован?   нет → Deny{not_mounted}, backend НЕ вызван
2. decide(req)                Deny → чек пишется, backend НЕ вызван
3. Ask в неинтерактивном      → Suspended + ApprovalRequest в .stellwerk/inbox/<run_id>.json
4. backend.invoke()           ошибка → чек всё равно записан
5. shape(вывод)               >2k токенов → хеш + путь к артефакту
6. чек в EventStore           для КАЖДОГО вызова, включая запрещённые
```

Проверка «not_mounted» идёт **до** `decide`: незамонтированный инструмент не должен
даже думать о правах. Запрещённый вызов **пишет чек**: иначе «тихие» попытки
не видны в расследовании.

## Инварианты

- правила берутся ТОЛЬКО из предикатов ядра, копий нет;
- `Ask` в неинтерактивном прогоне = `Suspended` + вопрос в inbox, а НЕ `Deny` (В4);
  истёкшее окно ответа = `ApprovalTimeout`, а не `Deny` — «никто не ответил» и
  «запрещено политикой» обязаны различаться и в статусе, и в чеке;
- в чеках только хеши, никогда тела вывода и никогда секреты;
- `R4` — `L5Owner` + `Ask` на ОДНО действие; при нехватке уровня `Deny` (В1);
- `R5` — безусловный `Deny` при любом уровне, включая `L5Owner` (ADR-0002);
- `Unclassified` — отдельное состояние, а не `R0`: в неинтерактивном прогоне `Deny`,
  в интерактивной `L5Owner`-сессии `Ask` (В2); значением по умолчанию не заменяется;
- суммарная цена смонтированных ≤ бюджета;
- порядок монтажа детерминирован: `(cost_tokens, name)`.

## Предикат приёмки milestone

```bash
cargo test -p stellwerk-gateway
```

0 failed. После M1+M2+M3: `cargo test --workspace` зелёный целиком.
