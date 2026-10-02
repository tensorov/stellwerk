"""packet-replay: точка входа, разбор packet-файлов, инвентаризация.

Модуль только читает план и печатает машинно-читаемый отчёт. Ничего не исполняет,
ничего не правит и ничего не запускает — это отдельная забота других двух модулей.

Правила, которые инструмент обязан соблюдать:

* источник истины — только `.sisyphus/plans/*/packet-*.md`; каталог `_archive`
  пропускается, `.omo/plans/` не обходится вовсе: это зеркало, а не канон, и
  выдача из зеркала воркеру приносит пакет с несуществующей зависимостью;
* поле `test:` ищется по структуре строки, а не по вхождению слова «test»
  в прозе или внутри ограждённого блока;
* значение поля сохраняется целиком — имена длинные и обрезать их нельзя;
* режим работы приходит из закрытого перечисления, а не из произвольной команды;
* каталог, который не является каноном, не попадает в обход ни при каких флагах.

Ровно этот модуль закрывает INFRA-08. Разбор полей и инвентаризация живут здесь, а
семантика команд и арифметика прогона вынесены: смешивать разбор с исполнением
значит невозможно проверить первое, не запустив второе.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from packet_replay_aggregate import aggregate
from packet_replay_validate import validate


MODES = ("inventory", "validate", "aggregate")

WORKSPACE = Path(__file__).resolve().parent.parent
CANONICAL_GLOB = ".sisyphus/plans/*/packet-*.md"
ARCHIVE_DIR = "_archive"
MIRROR_DIR = ".omo"

FENCE = re.compile(r"^\s*```")
FIELD = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)\s*:\s*(.*)$")
PHASE = re.compile(r"\b(RED|GREEN)\b")

PARSED = "PARSED"
NO_COMMAND = "NO_COMMAND"
PARSE_ERROR = "PARSE_ERROR"


def strip_decorations(value: str) -> str:
    """Снять ограждение в виде обратных кавычек и внешние пробелы.

    Обрезки здесь нет по построению: значение обрезается только по краям, то есть
    удаляются пробелы и обратные кавычки, а внутренние символы не трогаются.
    """
    text = value.strip()
    while len(text) >= 2 and text[0] == "`" and text[-1] == "`":
        text = text[1:-1].strip()
    return text


def read_fields(path: Path) -> tuple[dict[str, str], list[str]]:
    """Разобрать packet-файл по структуре.

    Поле — строка вида `имя: значение` вне ограждённого блока. Строка внутри
    ``` … ``` пропускается целиком, поэтому упоминание `test:` в примере или в
    цитате не может стать полем.
    """
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeDecodeError) as exc:
        return {}, [f"непрочитан: {exc}"]

    fields: dict[str, str] = {}
    problems: list[str] = []
    in_fence = False

    for raw in lines:
        if FENCE.match(raw):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        match = FIELD.match(raw)
        if not match:
            continue
        name, value = match.group(1), strip_decorations(match.group(2))
        # Первое вхождение выигрывает: повтор в прозе ниже по файлу не перебивает
        # настоящее поле из шапки.
        fields.setdefault(name, value)

    return fields, problems


def canonical_files(root: Path) -> list[Path]:
    """Перечислить канонические packet-файлы.

    Обход идёт по фиксированному шаблону внутри `.sisyphus/plans`. Каталог
    `_archive` отбрасывается по имени компонента, а `.omo` не может попасть в
    перечень уже потому, что шаблон начинается с `.sisyphus`.
    """
    base = root / ".sisyphus" / "plans"
    if not base.is_dir():
        return []
    found: list[Path] = []
    for path in sorted(base.glob("*/*")):
        if path.name == MIRROR_DIR:
            continue
        if not path.name.startswith("packet-") or path.suffix != ".md":
            continue
        if not path.is_file():
            continue
        if ARCHIVE_DIR in path.parts:
            continue
        found.append(path)
    return found


def collect_rows(root: Path) -> list[dict[str, str]]:
    """Собрать пакеты, у которых поле `test:` непустое."""
    rows: list[dict[str, str]] = []
    for path in canonical_files(root):
        fields, _ = read_fields(path)
        command = fields.get("test", "")
        if not command:
            continue
        rows.append(
            {
                "milestone": path.parent.name,
                "packet_id": path.stem.replace("packet-", ""),
                "command": command,
            }
        )
    return rows


def inventory(root: Path) -> int:
    """Разобрать все пакеты и напечатать счётчики. Ничего не исполняется."""
    rows: list[dict[str, str]] = []

    for path in canonical_files(root):
        fields, problems = read_fields(path)
        milestone = path.parent.name
        packet_id = path.stem.replace("packet-", "")
        command = fields.get("test", "")
        phase = ""
        goal = fields.get("goal", "")
        if goal:
            found = PHASE.search(goal)
            if found:
                phase = found.group(1)

        if problems:
            status = PARSE_ERROR
        elif command:
            status = PARSED
        else:
            status = NO_COMMAND

        rows.append(
            {
                "milestone": milestone,
                "packet_id": packet_id,
                "status": status,
                "phase": phase,
                "command": command,
                "depends_on": fields.get("depends_on", ""),
                "source": str(path.relative_to(root)),
                "problem": "; ".join(problems),
            }
        )

    counts = {PARSED: 0, NO_COMMAND: 0, PARSE_ERROR: 0}
    for row in rows:
        counts[row["status"]] += 1

    for row in rows:
        print(
            f"{row['status']}\t{row['milestone']}/{row['packet_id']}"
            f"\t{row['phase'] or '-'}\t{row['source']}"
            f"\t{row['command'] or '-'}"
        )

    print(f"total={len(rows)}")
    print(f"{PARSED}={counts[PARSED]}")
    print(f"{NO_COMMAND}={counts[NO_COMMAND]}")
    print(f"{PARSE_ERROR}={counts[PARSE_ERROR]}")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="packet_replay",
        description="Проверка воспроизводимости acceptance-команд пакетов.",
    )
    parser.add_argument("mode", choices=MODES, help="режим из закрытого перечисления")
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
        help="корень обхода; xtask его не передаёт, значение нужно самотестам",
    )
    parser.add_argument(
        "--exit-code",
        type=int,
        default=0,
        help="код завершения прогона, который описывает переданный вывод",
    )
    parser.add_argument(
        "--timed-out",
        action="store_true",
        help="прогон прерван по таймауту",
    )
    args = parser.parse_args(argv)

    if args.mode == "inventory":
        return inventory(args.root)
    if args.mode == "validate":
        return validate(collect_rows(args.root))
    return aggregate(args.exit_code, args.timed_out)


if __name__ == "__main__":
    sys.exit(main())
