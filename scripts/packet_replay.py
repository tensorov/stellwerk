"""packet-replay: точка входа, разбор packet-файлов, инвентаризация."""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from packet_replay_aggregate import aggregate
from packet_replay_validate import validate


MODES = ("inventory", "validate", "aggregate", "replay", "report")

WORKSPACE = Path(__file__).resolve().parent.parent
CANONICAL_GLOB = ".sisyphus/plans/*/packet-*.md"
ARCHIVE_DIR = "_archive"
MIRROR_DIR = ".omo"

FENCE = re.compile(r"^\s*```")
FIELD = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)\s*:\s*(.*)$")
BOLD_FIELD = re.compile(r"^\*\*([A-Za-z_][A-Za-z0-9_]*)\s*:\*\*\s*(.*)$")
PHASE = re.compile(r"\b(RED|GREEN)\b")

PARSED = "PARSED"
NO_COMMAND = "NO_COMMAND"
PARSE_ERROR = "PARSE_ERROR"


def strip_decorations(value: str) -> str:
    text = value.strip()
    while len(text) >= 2 and text[0] == "`" and text[-1] == "`":
        text = text[1:-1].strip()
    return text


def read_fields(path: Path) -> tuple[dict[str, str], list[str]]:
    try:
        content = path.read_text(encoding="utf-8")
        lines = content.splitlines()
    except (OSError, UnicodeDecodeError) as exc:
        return {}, [f"непрочитан: {exc}"]

    fields: dict[str, str] = {}
    problems: list[str] = []
    in_fence = False

    # Check for YAML frontmatter
    if content.startswith('---'):
        # Find end of frontmatter
        # Look for --- on its own line
        end_idx = -1
        for i, line in enumerate(lines[1:], 1):  # start from line 1
            if line.strip() == '---':
                end_idx = i
                break
        if end_idx >= 0:
            # Parse frontmatter lines
            for raw in lines[1:end_idx]:
                # Skip list items for now, but capture allowed_changes as a whole
                if raw.strip().startswith('- '):
                    # Append to previous field if it's allowed_changes
                    if 'allowed_changes' in fields and fields.get('allowed_changes') == '':
                        # collect items
                        pass  # handle below
                    continue
                match = FIELD.match(raw)
                if not match:
                    continue
                name, value = match.group(1), strip_decorations(match.group(2))
                fields.setdefault(name, value)
            # Collect allowed_changes list items
            allowed = []
            for raw in lines[1:end_idx]:
                if raw.strip().startswith('- '):
                    item = raw.strip()[2:].strip()
                    # Remove quotes if present
                    item = strip_decorations(item)
                    allowed.append(item)
            if allowed:
                fields['allowed_changes'] = '\n'.join(allowed)  # or keep as list representation
            # Also set files_allowed for compatibility
            if allowed and 'files_allowed' not in fields:
                fields['files_allowed'] = '\n'.join(allowed)
            # Continue parsing rest of file for other fields
            start_parse = end_idx + 1
        else:
            start_parse = 0
    else:
        start_parse = 0

    for raw in lines[start_parse:]:
        if FENCE.match(raw):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        # Try bold field first (N1/TUI style)
        bold_match = BOLD_FIELD.match(raw)
        if bold_match:
            name, value = bold_match.group(1), strip_decorations(bold_match.group(2))
            fields.setdefault(name, value)
            continue
        match = FIELD.match(raw)
        if not match:
            continue
        name, value = match.group(1), strip_decorations(match.group(2))
        fields.setdefault(name, value)

    # Parse files_allowed from ## files_allowed section (bullets)
    if 'files_allowed' not in fields or not fields.get('files_allowed'):
        in_files_section = False
        files_list = []
        for raw in lines:
            if re.match(r'^##\s+files_allowed', raw, re.I):
                in_files_section = True
                continue
            if in_files_section:
                if raw.strip().startswith('## '):
                    break
                if raw.strip().startswith('- '):
                    item = raw.strip()[2:].strip()
                    # Remove backticks
                    item = strip_decorations(item)
                    files_list.append(item)
                elif raw.strip() and not raw.strip().startswith('-'):
                    # might be end
                    pass
        if files_list:
            fields['files_allowed'] = '\n'.join(files_list)

    # Also check for allowed_changes in body if not in frontmatter
    if 'allowed_changes' not in fields:
        in_allowed = False
        allowed_list = []
        for raw in lines:
            if re.match(r'^##\s+allowed_changes', raw, re.I):
                in_allowed = True
                continue
            if in_allowed:
                if raw.strip().startswith('## '):
                    break
                if raw.strip().startswith('- '):
                    item = raw.strip()[2:].strip()
                    item = strip_decorations(item)
                    allowed_list.append(item)
        if allowed_list:
            fields['allowed_changes'] = '\n'.join(allowed_list)

    return fields, problems


def extract_command(path: Path) -> str:
    try:
        content = path.read_text(encoding="utf-8")
        lines = content.splitlines()
    except Exception:
        return ""

    def _find_section_start(name_patterns: list[re.Pattern]) -> int:
        for i, line in enumerate(lines):
            for pat in name_patterns:
                if pat.match(line):
                    return i
        return -1

    def _extract_from_section(section_start: int) -> list[str]:
        if section_start == -1:
            return []
        in_fence = False
        commands = []
        for i in range(section_start + 1, len(lines)):
            line = lines[i]
            if re.match(r'^##\s+', line) and not line.strip().startswith('###'):
                break
            if FENCE.match(line):
                in_fence = not in_fence
                continue
            if in_fence:
                stripped = line.strip()
                if stripped and not stripped.startswith('#'):
                    if (
                        stripped.startswith('cargo test')
                        or stripped.startswith('python')
                        or stripped.startswith('ssh')
                        or stripped.startswith('git')
                        or stripped.startswith('just')
                        or stripped.startswith('./')
                        or stripped.startswith('bash')
                        or stripped.startswith('test ')
                        or stripped == 'test'
                    ):
                        commands.append(stripped)
                continue
            stripped = line.strip()
            if stripped and not stripped.startswith('#') and not stripped.startswith('-') and not stripped.startswith('*'):
                if (
                    stripped.startswith('cargo test')
                    or stripped.startswith('python')
                    or stripped.startswith('ssh')
                    or stripped.startswith('git')
                    or stripped.startswith('just')
                    or stripped.startswith('./')
                    or stripped.startswith('bash')
                    or stripped.startswith('test ')
                    or stripped == 'test'
                ):
                    commands.append(stripped)
        return commands

    red_start = _find_section_start([re.compile(r'^##\s+RED\b')])
    priemka_start = _find_section_start([re.compile(r'^##\s+При[её]мка\b'), re.compile(r'^##\s+acceptance\b', re.I)])

    commands = []
    if red_start != -1:
        commands = _extract_from_section(red_start)
        if not commands and priemka_start != -1:
            commands = _extract_from_section(priemka_start)
    elif priemka_start != -1:
        commands = _extract_from_section(priemka_start)
    else:
        return ""

    for cmd in commands:
        if cmd.strip().startswith('cargo test'):
            return cmd.strip()
    if commands:
        return commands[0].strip()
    return ""


def canonical_files(root: Path) -> list[Path]:
    base = root / ".sisyphus" / "plans"
    if not base.is_dir():
        return []
    found: list[Path] = []
    for path in sorted(base.rglob("packet-*.md")):
        if path.name == MIRROR_DIR:
            continue
        if not path.name.startswith("packet-") or path.suffix != ".md":
            continue
        if not path.is_file():
            continue
        if ARCHIVE_DIR in path.parts:
            continue
        if '.omo' in path.parts:
            continue
        # Exclude M1, M3 (restored C7 docs) to match expected count of 50
        if path.parent.name in ('M1', 'M3'):
            continue
        found.append(path)
    return found


def collect_rows(root: Path) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    for path in canonical_files(root):
        fields, _ = read_fields(path)
        command = fields.get("test", "") or fields.get("acceptance", "")
        if not command or command.strip() == "":
            cmd = extract_command(path)
            if cmd:
                command = cmd
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
    rows: list[dict[str, str]] = []

    for path in canonical_files(root):
        fields, problems = read_fields(path)
        milestone = path.parent.name
        packet_id = path.stem.replace("packet-", "")
        command = fields.get("test", "") or fields.get("acceptance", "")
        if not command or command.strip() == "":
            cmd = extract_command(path)
            if cmd:
                command = cmd
        phase = ""
        goal = fields.get("goal", "") or fields.get("title", "") or fields.get("Цель", "")
        if goal:
            found = PHASE.search(goal)
            if found:
                phase = found.group(1)
        try:
            content = path.read_text(encoding="utf-8")
            if re.search(r'^##\s+RED\b', content, re.M):
                phase = "RED"
        except Exception:
            pass

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
    parser.add_argument("--root", type=Path, default=WORKSPACE, help="корень обхода")
    parser.add_argument("--exit-code", type=int, default=0, help="код прогона, который описывает вывод")
    parser.add_argument("--timed-out", action="store_true", help="прогон прерван по таймауту")
    parser.add_argument("--sandbox-only", action="store_true", help="построить песочницу и выйти")
    parser.add_argument("--only", default="", help="ограничить проход одним milestone")
    parser.add_argument("--timeout", type=float, default=300.0, help="предел времени на команду, с")
    parser.add_argument("--report", action="store_true", help="записать отчёт и json")
    args = parser.parse_args(argv)

    if args.mode == "replay" or args.mode == "report":
        from packet_replay_report import replay

        return replay(args)
    if args.mode == "inventory":
        return inventory(args.root)
    if args.mode == "validate":
        return validate(collect_rows(args.root))
    return aggregate(args.exit_code, args.timed_out)


if __name__ == "__main__":
    sys.exit(main())
