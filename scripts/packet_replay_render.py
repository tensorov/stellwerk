"""Печать отчёта для человека. Знания о прогоне не содержит — только формат.

Модуль выделен, потому что драйвер и рендеринг — разные предметы. Драйвер накапливает
записи и ничего не знает о том, как они будут выглядеть; рендеринг ничего не знает о
прогонах и только раскладывает готовое по таблицам. Вместе они не помещались в
предел ADR-0003, и сокращать ради предела пришлось бы выбрасывать данные, а не слова.

Правило, которое рендеренг обязан соблюдать: ни одной константы от себя. Каждое число
приходит из payload. Отчёт, где «ожидаемое» число зашито, однажды становится ложью и
ломает ту самую проверку, ради которой написан.
"""

from __future__ import annotations

from pathlib import Path

from packet_replay_aggregate import PASS

def render_markdown(payload: dict) -> str:
    """Отчёт для человека. Числа — только из payload, ни одной константы от себя."""
    lines: list[str] = []
    add = lines.append

    summary = payload["summary"]
    inventory = payload["inventory"]

    add("# packet-replay — отчёт о проходе")
    add("")
    add(f"- Проверенный commit: `{payload['tested_commit']}`")
    add(f"- Инструмент: `{payload['tool']}`")
    add(f"- Длительность прохода: {payload['duration_s']} с")
    add("")
    add("## Toolchain")
    add("")
    for name, version in payload["toolchain"].items():
        add(f"- {name}: `{version}`")
    add("")

    add("## Что найдено в плане")
    add("")
    add(
        f"Пакетов в каноне: **{inventory.get('total', 0)}**; с командой: "
        f"**{inventory.get('parsed', 0)}**; без команды: **{inventory.get('no_command', 0)}**; "
        f"с ошибкой разбора: **{inventory.get('parse_error', 0)}**."
    )
    add(
        "Числа фактические и меняются вместе с планом; ожидаемых значений у отчёта нет."
    )
    add("")

    add("## Итог прохода")
    add("")
    add(f"- Записей: **{summary['total']}**")
    add(f"- Реально выполнено: **{summary['executed']}**")
    add(f"- Не выполнено: **{summary['not_executed']}**")
    add("")
    add("| Вердикт | Пакетов |")
    add("| --- | --- |")
    for verdict, count in summary["by_verdict"].items():
        add(f"| `{verdict}` | {count} |")
    add("")

    notable_rows = payload["notable"]
    titles = {
        "unsupported": "Неподдержанные формы команд",
        "empty_selection": "Пустые выборки и неоднозначные результаты",
        "errors": "Ошибки команд",
    }
    for key in ("unsupported", "empty_selection", "errors"):
        rows = notable_rows[key]
        add(f"## {titles[key]} — {len(rows)}")
        add("")
        if not rows:
            add("Пусто.")
            add("")
            continue
        add("| Пакет | Команда | Вердикт | Причина |")
        add("| --- | --- | --- | --- |")
        for row in rows:
            command = str(row["command"]).replace("|", "\\|")
            reason = str(row["reason"]).replace("|", "\\|")
            add(f"| {row['milestone']}/{row['packet_id']} | `{command}` | `{row['verdict']}` | {reason} |")
        add("")

    add("## Про все пакеты")
    add("")
    add("| Пакет | Команда | argv | Target | Фильтр | Selected | Passed | Failed | Exit | Timeout | Вердикт |")
    add("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    for record in payload["records"]:
        totals = record.get("totals") or {}
        selected = record.get("selected_by_list")
        argv = " ".join(str(a) for a in (record.get("argv") or []))
        command = str(record.get("command", "")).replace("|", "\\|")
        argv = argv.replace("|", "\\|")
        add(
            "| {milestone}/{packet_id} | `{command}` | `{argv}` | {target} | {filter} | {selected} "
            "| {passed} | {failed} | {exit_code} | {timed_out} | `{verdict}` |".format(
                milestone=record.get("milestone", ""),
                packet_id=record.get("packet_id", ""),
                command=command,
                argv=argv or "-",
                target=record.get("target") or "-",
                filter=record.get("filter") or "-",
                selected="null" if selected is None else selected,
                passed=totals.get("passed", "-"),
                failed=totals.get("failed", "-"),
                exit_code=record.get("exit_code"),
                timed_out="да" if record.get("timed_out") else "нет",
                verdict=record.get("verdict") or "-",
            )
        )
    add("")

    add("## Ограничения отчёта")
    add("")
    add(
        "- Отвечает только на вопрос «воспроизводима ли команда на этом commit». "
        "Ни «был ли исторический RED», ни «покрыт ли крейт полностью»."
    )
    add(
        "- `PASS` команды не доказывает, что acceptance пакета выполнен полностью: "
        "команда может проверять подмножество."
    )
    add(
        "- Проход не воспроизводит историческую стадию RED автоматически. Если команда "
        "проверяла ожидаемое падение, а сегодня зелено, фактический результат и "
        "историческое ожидание показаны раздельно."
    )
    add(
        "- Сумма `passed` по пакетам не есть число уникальных тестов проекта: разные "
        "команды многократно запускают одни и те же тесты."
    )
    add(
        "- Числа фактические. Ожидаемых констант у отчёта нет: новый пакет обязан менять "
        "эти числа, а не опровергать зашитое значение."
    )
    add("")
    return "\n".join(lines)


def write_markdown(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(render_markdown(payload), encoding="utf-8")