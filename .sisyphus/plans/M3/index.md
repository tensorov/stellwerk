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
| packet-03 | m3/gate-red | 02 | `--test gate` | RED: решение по вызову |
| packet-04 | m3/gate | 03 | `--test gate` | GREEN: decide |
| packet-05 | m3/shaping-red | 04 | `--test shaping` | RED: усечение вывода |
| packet-06 | m3/shaping | 05 | `--test shaping` | GREEN: артефакт + хеш |
| packet-07 | m3/mount-red | 06 | `--test mount` | RED: бюджет монтажа |
| packet-08 | m3/mount | 07 | `--test mount` | GREEN: жадный проход |
| packet-09 | m3/receipt-red | 08 | `--test receipt` | RED: чек вызова |
| packet-10 | m3/receipt | 09 | `--test receipt` | GREEN: чек → событие |
| packet-11 | m3/session-red | 10 | `--test session` | RED: маршрутизация |
| packet-12 | m3/session | 11 | `cargo test -p stellwerk-gateway` | GREEN: сессия целиком |

## Волны

```
01 → 02        — permission (contract-change в 01)
03 → 04        — gate: риск × полномочия × данные
05 → 06        — shaping: 2k токенов, артефакт, хеш
07 → 08        — mount: что вообще подключать
09 → 10 → 11 → 12 — чек, затем сессия
```

Строго последовательно. Крейт новый, каждый пакет опирается на предыдущий.

## Порядок обработки вызова (нарушать нельзя)

```
1. инструмент смонтирован?   нет → Deny{not_mounted}, backend НЕ вызван
2. decide(req)                Deny → чек пишется, backend НЕ вызван
3. Ask без подтверждения     → Deny{ask_unresolved}
4. backend.invoke()           ошибка → чек всё равно записан
5. shape(вывод)               >2k токенов → хеш + путь к артефакту
6. чек в EventStore           для КАЖДОГО вызова, включая запрещённые
```

Проверка «not_mounted» идёт **до** `decide`: незамонтированный инструмент не должен
даже думать о правах. Запрещённый вызов **пишет чек**: иначе «тихие» попытки
не видны в расследовании.

## Инварианты

- правила берутся ТОЛЬКО из предикатов ядра, копий нет;
- `Ask` без подтверждения = `Deny` (провал безопасен);
- в чеках только хеши, никогда тела вывода и никогда секреты;
- `R4`/`R5` — всегда `Deny`, даже при `L5Owner`;
- суммарная цена смонтированных ≤ бюджета;
- порядок монтажа детерминирован: `(cost_tokens, name)`.

## Предикат приёмки milestone

```bash
cargo test -p stellwerk-gateway
```

0 failed. После M1+M2+M3: `cargo test --workspace` зелёный целиком.
