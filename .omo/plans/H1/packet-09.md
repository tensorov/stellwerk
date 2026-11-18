---
id: H1/09
milestone: H1
title: packet_replay scans M1/M3 instead of hiding them
status: proposed
depends_on: [H1/04]
allowed_changes:
  - scripts/packet_replay.py
  - xtask/tests/packet_replay_selftest.rs
max_files: 2
max_diff_lines: 250
acceptance: cargo test -p xtask --test packet_replay_selftest
---

# H1/09: packet_replay scans M1/M3 instead of hiding them

## Зачем

`scripts/packet_replay.py:250-252` вручную исключает два milestone:

```python
# Exclude M1, M3 (restored C7 docs) to match expected count of 50
if path.parent.name in ('M1', 'M3'):
    continue
```

Эта ветка скрывает канонические пакеты `M1/packet-05.md` и `M1/packet-06.md`
из `inventory` и `validate`. Комментарий про «expected count of 50» устарел:
текущий `inventory` печатает 81 пакет, а без skip печатает 83. Ориентир 50 не
является целью и не должен подгоняться. Цель: полный проход по каноническим
`.sisyphus/plans/**`, без ручного списка исключений по milestone.

Петля `rglob('packet-*.md')` уже отсекает зеркало `.omo`, архив и не-файлы.
Ветка M1/M3 остается единственным местом, где канонический пакет теряется.

## RED

На базе `H1/04` падают девять тестов.

`cargo test -p xtask --test packet_replay_selftest`:

- `backticked_command_is_parsed`
- `counts_match_fixture_not_brief_heuristics`
- `long_target_name_is_not_truncated`
- `single_line_test_field_is_parsed`

`cargo test -p xtask --test packet_replay_validate`:

- `positional_filter_is_not_a_target`
- `nonexistent_package_is_invalid_package`
- `test_target_is_validated_against_metadata`
- `shell_and_escape_attempts_are_unsupported`
- `nonexistent_target_is_invalid_target`

Это 4 selftest и 5 validate, всего 9. Это и есть RED-доказательство: не
объявлять пакет зеленым, пока оба набора не пройдут.

## Шаг 2: минимальная правка

1. Удалить ветку `if path.parent.name in ('M1', 'M3'): continue` и устаревший
   комментарий про 50. `canonical_files` сканирует `M1` и `M3` наравне с
   остальными milestone.
2. В `xtask/tests/packet_replay_selftest.rs` добавить регрессионный тест,
   например `m1_and_m3_packets_are_not_filtered`: фикстура с
   `M1/packet-05.md` и `M3/packet-01.md`, запуск `inventory`, требование, чтобы
   оба файла попали в вывод. Тест обязан падать на старом skip и зеленеть после
   его удаления.
3. Не менять формат вывода и не подгонять числа под комментарий 50. Не
   добавлять новые исключения по milestone.

## Приёмка

Основная:

```
cargo test -p xtask --test packet_replay_selftest
```

Дополнительная:

```
cargo test -p xtask --test packet_replay_validate
```

`/tmp` переполнен. Запускать с `TMPDIR` под `/home` и отдельным
`CARGO_TARGET_DIR`: иначе `cc`/sqlite падают с ENOSPC, и это не ошибка кода.

## Не делается

No product code, no owner-WIP, no new dependencies, no `#[allow]`, no `#[ignore]`.
Не гнаться за `50` из комментария: текущий инвентарь 81, после снятия skip 83.
Не менять другие пакеты H1 и не трогать `scripts/packet_replay.py` за пределами
удаления skip.

## Бюджет

C2: src ≤ 200 logical lines, tests ≤ 250 added lines. `max_diff_lines` is raw and non-authoritative.
