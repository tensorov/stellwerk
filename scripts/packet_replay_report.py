"""Драйвер прохода: один запуск инструмента по всем пакетам и запись результата.

Отдельный модуль, потому что это другой предмет, чем исполнение. Здесь нет ни разбора
пакетов, ни семантики команд, ни таймаута: только обход, накопление записей и печать
двух артефактов. Проверка «можно ли запускать» уже произошла в исполнителе, и повторять
её здесь означало бы получить два места, где решается одно и то же.

Правила, которые драйвер обязан соблюдать:

* проход последователен: один пакет за раз, без параллели;
* каждый пакет даёт ровно одну запись, даже если команда не была выполнена вовсе, —
  иначе «не выполнено» нельзя отличить от «забыли»;
* числа не зашиваются: сколько пакетов и команд найдено, выводится из факта, потому
  что новый пакет обязан менять эти числа, а не опровергать константу;
* отчёт — наблюдение, а не приговор: обнаруженный дефект перечисляется, но не
  исправляется по ходу прохода.
"""

from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from packet_replay_run import run_command
from packet_replay_aggregate import PASS
from packet_replay_render import render_markdown, write_markdown
from packet_replay_validate import load_metadata, metadata_index

# Куда складываются артефакты. Отчёт — наблюдение о проходе, поэтому он не считается
# частью проверяемого дерева и не попадает в песочницу.
REPORT_MD = Path(".night") / "PACKET_REPLAY_REPORT.md"
REPORT_JSON = Path(".night") / "packet-replay.json"

DEFAULT_TIMEOUT = 300.0

TOOL_VERSION = "packet-replay/1.0"


def toolchain() -> dict[str, str]:
    """Версии инструментов, которыми измерялось. Без них отчёт невоспроизводим."""
    found = {}
    for name, argv in (
        ("cargo", ["cargo", "--version"]),
        ("rustc", ["rustc", "--version"]),
        ("python3", ["python3", "--version"]),
        ("git", ["git", "--version"]),
    ):
        try:
            out = subprocess.run(argv, capture_output=True, text=True, check=False, timeout=30)
            found[name] = (out.stdout or out.stderr).strip().splitlines()[0]
        except (OSError, subprocess.SubprocessError, IndexError):
            found[name] = "неизвестно"
    return found


def run_pass(
    rows: list[dict[str, str]],
    sandbox: Path,
    timeout: float = DEFAULT_TIMEOUT,
    only: str = "",
) -> list[dict]:
    """Прогнать команды по одному пакету и вернуть записи.

    `only` ограничивает проход одним milestone — это ограничение отчёта, а не ослабление
    правил исполнения: всё исполняемое по-прежнему исполняется в песочнице.
    """
    packages, _ = metadata_index(load_metadata())
    records = []
    for row in rows:
        if only and row["milestone"] != only:
            continue
        record = run_command(row["command"], packages, sandbox, timeout)
        record["milestone"] = row["milestone"]
        record["packet_id"] = row["packet_id"]
        records.append(record)
    return records


def summarize(records: list[dict]) -> dict[str, object]:
    """Счётчики по вердиктам. Все — из фактических записей, ни одна не задана руками."""
    by_verdict: dict[str, int] = {}
    executed = 0
    for record in records:
        verdict = str(record.get("verdict") or "НЕТ")
        by_verdict[verdict] = by_verdict.get(verdict, 0) + 1
        if record.get("exit_code") is not None or record.get("totals"):
            executed += 1
    return {
        "total": len(records),
        "executed": executed,
        "not_executed": len(records) - executed,
        "by_verdict": dict(sorted(by_verdict.items())),
    }


def notable(records: list[dict]) -> dict[str, list[dict]]:
    """Разложить не-PASS по причинам. Каждый дефект — с пакетом и машиным вердиктом."""
    buckets: dict[str, list[dict]] = {
        "errors": [],
        "empty_selection": [],
        "unsupported": [],
        "passed": [],
    }
    for record in records:
        verdict = str(record.get("verdict") or "")
        item = {
            "milestone": record.get("milestone", ""),
            "packet_id": record.get("packet_id", ""),
            "command": record.get("command", ""),
            "verdict": verdict,
            "reason": record.get("reason", ""),
        }
        if verdict == PASS:
            buckets["passed"].append(item)
        elif verdict.startswith("UNSUPPORTED") or verdict == "RECURSION":
            buckets["unsupported"].append(item)
        elif verdict.startswith("INCONCLUSIVE") or verdict == "DISCOVERY_FAILED":
            buckets["empty_selection"].append(item)
        else:
            buckets["errors"].append(item)
    return buckets


def to_json(
    records: list[dict],
    commit: str,
    started: float,
    sandbox: Path,
    inventory_counts: dict[str, int],
) -> dict:
    """Полная запись прохода. Всё, что в отчёте, берётся отсюда."""
    return {
        "tool": TOOL_VERSION,
        "tested_commit": commit,
        "started_at": started,
        "finished_at": time.time(),
        "duration_s": round(time.time() - started, 1),
        "sandbox": str(sandbox),
        "toolchain": toolchain(),
        "inventory": inventory_counts,
        "summary": summarize(records),
        "notable": notable(records),
        "records": records,
    }


def write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=False) + "\n",
        encoding="utf-8",
    )


def inventory_counts(root: Path) -> dict[str, int]:
    """Счётчики разбора плана для отчёта. Из фактического обхода, не из константы."""
    from packet_replay import canonical_files, read_fields

    counts = {"total": 0, "parsed": 0, "no_command": 0, "parse_error": 0}
    for path in canonical_files(root):
        counts["total"] += 1
        fields, problems = read_fields(path)
        if problems:
            counts["parse_error"] += 1
        elif fields.get("test", ""):
            counts["parsed"] += 1
        else:
            counts["no_command"] += 1
    return counts


def replay(args) -> int:
    """Полный проход: песочница, записи по пакетам, два артефакта.

    Порядок именно такой: песочница строится до обхода, потому что прогон обязан идти
    на изолированной копии того commit, который проверен. Артефакты пишутся после
    прохода, а не во время, — иначе прерванный проход оставил бы отчёт, который
    выглядит завершённым.
    """
    from packet_replay import WORKSPACE, collect_rows
    from packet_replay_execute import make_sandbox, sandbox_report

    sandbox = make_sandbox(WORKSPACE)
    print(sandbox_report(sandbox))

    started = time.time()
    commit = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=WORKSPACE,
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()

    records = run_pass(collect_rows(args.root), sandbox, args.timeout, args.only)
    payload = to_json(records, commit, started, sandbox, inventory_counts(args.root))

    print(f"RECORDS={len(records)}")
    print(f"COMMIT={commit}")
    if args.report:
        write_json(WORKSPACE / REPORT_JSON, payload)
        write_markdown(WORKSPACE / REPORT_MD, payload)
        print(f"REPORT_MD={REPORT_MD}")
        print(f"REPORT_JSON={REPORT_JSON}")
    return 0
