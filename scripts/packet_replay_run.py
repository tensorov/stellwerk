"""Исполнитель: единственное место, где что-то запускается.

Отдельный модуль, потому что это другой предмет, чем разбор и семантика. Здесь нет
ни печатных отчётов, ни проверки существования package: только превращение строки
команды в argv, запуск с таймаутом и подсчёт того, что команда собирается выполнить.

Разделение с `packet_replay_validate` намеренное и не дублирование: валидатор отвечает
«что команда означает» и argv не возвращает, исполнитель — «можно ли выполнить ровно
этот список». Поэтому неизвестный флаг отвергается здесь, а не пропускается молча.
"""

from __future__ import annotations

import os
import re
import shlex
import signal
import subprocess
import time
from pathlib import Path

RECURSION = "RECURSION"
REPLAY_SCRIPT = "packet_replay.py"

UNSUPPORTED = "UNSUPPORTED_COMMAND"

# Флаги, не требующие значения. Всё, чего здесь нет, отвергается: неизвестный флаг
# означает, что команда может означать не то, о чём мы думаем.
BARE_FLAGS = frozenset({
    "--lib", "--bins", "--tests", "--all-targets", "--doc", "--no-run",
    "--workspace", "--all", "--quiet", "--verbose", "--nocapture", "--color",
    "--locked", "--offline", "--no-fail-fast", "--include-ignored", "--release",
})

# Флаги, за которыми обязательно следует значение.
VALUE_FLAGS = frozenset({"-p", "--package", "--test", "--bin", "--features", "--exclude"})

# Параметры, меняющие конфигурацию или место сборки: их присутствие делает результат
# непригодным для проверки именно этого workspace.
FORBIDDEN_FLAGS = frozenset(
    {"--manifest-path", "--config", "--target-dir", "--lockfile-path", "-Z"}
)

# Строка, которой libtest помечает имя теста в выводе `--list`.
LISTED_TEST = re.compile(r":\s*test$")


def refuse_recursion(argv: list[str]) -> str:
    """Причина отказа, если команда запускает сам replay. Пустая строка — можно."""
    for token in argv:
        if REPLAY_SCRIPT in token:
            return f"{RECURSION}: команда содержит сам packet_replay.py"
    return ""


def build_run_argv(command: str) -> tuple[list[str], str]:
    """Построить argv для исполнения либо назвать причину отказа.

    Возвращается пара: сам список аргументов и пустая строка, либо пустой список и
    причина. Отказ всегда назван: молча пропущенный неизвестный флаг — это запуск
    не того, что написано в пакете.
    """
    try:
        argv = shlex.split(command)
    except ValueError as exc:
        return [], f"аргументы не разобраны: {exc}"
    if not argv:
        return [], "пустая команда"

    reason = refuse_recursion(argv)
    if reason:
        return [], reason

    index = 2 if argv[0] == "cargo" and len(argv) > 1 and argv[1] == "test" else 0
    if index == 0:
        return [], "не семейство cargo test"

    while index < len(argv):
        token = argv[index]
        if token == "--":
            break
        if token in FORBIDDEN_FLAGS:
            return [], f"параметр {token} меняет конфигурацию или песочницу"
        if token in VALUE_FLAGS:
            if index + 1 >= len(argv):
                return [], f"параметр {token} требует значения"
            index += 2
            continue
        if token.startswith("--") and "=" in token:
            name = token.split("=", 1)[0]
            if name in VALUE_FLAGS or name in BARE_FLAGS:
                index += 1
                continue
            return [], f"неизвестный параметр {name}"
        if token.startswith("-"):
            if token not in BARE_FLAGS:
                return [], f"неизвестный параметр {token}"
            index += 1
            continue
        index += 1

    return argv, ""


def discovery_argv(argv: list[str]) -> list[str]:
    """Тот же отбор, но с `--list`: условия выбора обязаны сохраниться.

    `--` идёт в конце, чтобы libtest, а не cargo, получил флаг: иначе отбор тестов
    меняется на отбор пакетов и discovery измеряет не ту выборку.
    """
    return list(argv) + ["--", "--list"]


def count_listed(text: str) -> int | None:
    """Число тестов из вывода `--list`, либо None, если вывод неразбираем.

    `None` и `0` — разные вещи: ноль означает «libtest сказал, что тестов нет», а
    `None` — «это не вывод libtest», и второй случай обязан быть виден в отчёте.
    """
    if not any(LISTED_TEST.search(line) for line in text.splitlines()):
        return None
    return sum(1 for line in text.splitlines() if LISTED_TEST.search(line))


def execute(argv: list[str], cwd: Path, timeout: float) -> dict:
    """Запустить argv в каталоге и вернуть всё, что о прогоне известно.

    При таймауте убивается вся группа процесса, а накопленный вывод сохраняется:
    убийство только прямого ребёнка оставило бы cargo-потомков работать и писать в
    тот же каталог.
    """
    started = time.monotonic()
    proc = subprocess.Popen(
        argv,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    timed_out = False
    try:
        out, err = proc.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        timed_out = True
        try:
            os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
        except ProcessLookupError:
            pass
        out, err = proc.communicate()

    return {
        "argv": list(argv),
        "exit_code": None if timed_out else proc.returncode,
        "timed_out": timed_out,
        "duration_s": round(time.monotonic() - started, 3),
        "stdout": out.decode("utf-8", "replace"),
        "stderr": err.decode("utf-8", "replace"),
    }

# Флаги, при которых команда перечисляет тесты, но не выполняет их. Их прогон нельзя
# выдавать за выполненные тесты: `passed > 0` отсюда не бывает, а жадный парсер строк
# однажды выдал бы «71 тест выполнен» там, где 71 тест даже не запускался.
ENUMERATE_FLAGS = ("--list", "--no-run")

# Признак, что команда ничего не исполняла, а только перечислила.
DISCOVERY_ONLY = "DISCOVERY_ONLY"


def enumerates_only(argv: list[str]) -> bool:
    """Перечисляет ли команда тесты вместо их исполнения."""
    head = argv[: argv.index("--")] if "--" in argv else argv
    return any(token in ENUMERATE_FLAGS for token in head)


def run_command(
    command: str,
    packages: dict[str, set[str]],
    sandbox: Path,
    timeout: float,
    discover: bool = True,
) -> dict:
    """Прогнать одну команду и вернуть запись для отчёта.

    Ни одна из величин не выводится из другой: `selected_by_list` — это то, что
    команда собиралась выполнить, а `totals` — то, что она действительно выполнила.
    Смешивать их нельзя, иначе «найдено 71» превратится в «71 выполнен».
    """
    from packet_replay_aggregate import classify, parse_result_lines
    from packet_replay_validate import VALID_COMMAND, classify_command

    record: dict[str, object] = {
        "command": command,
        "argv": [],
        "status": "",
        "package": "",
        "target": "",
        "filter": "",
        "selected_by_list": None,
        "totals": None,
        "exit_code": None,
        "timed_out": False,
        "duration_s": 0.0,
        "verdict": "",
        "reason": "",
        "execution_kind": "",
    }

    meaning = classify_command(command, packages)
    record["status"] = meaning["status"]
    record["package"] = meaning["package"]
    record["target"] = meaning["target"]
    record["filter"] = meaning["filter"]

    if meaning["status"] != VALID_COMMAND:
        record["verdict"] = meaning["status"]
        record["reason"] = meaning["reason"]
        return record

    argv, refusal = build_run_argv(command)
    record["argv"] = argv
    if refusal:
        record["verdict"] = UNSUPPORTED
        record["reason"] = refusal
        return record

    # Перечисляющая команда и есть discovery: отдельный запуск ей не нужен, второй
    # запуск той же строки посчитал бы одно и то же дважды.
    enumerates = enumerates_only(argv)
    record["execution_kind"] = DISCOVERY_ONLY if enumerates else "RUN"
    if discover and not enumerates:
        listed = execute(discovery_argv(argv), sandbox, timeout)
        record["selected_by_list"] = count_listed(listed["stdout"])

    run = execute(argv, sandbox, timeout)
    totals = parse_result_lines(run["stdout"])
    record["totals"] = totals
    record["exit_code"] = run["exit_code"]
    record["timed_out"] = run["timed_out"]
    record["duration_s"] = run["duration_s"]

    if enumerates and record["selected_by_list"] is None:
        record["selected_by_list"] = count_listed(run["stdout"])

    selected = record["selected_by_list"]
    record["verdict"] = classify(totals, run["exit_code"] or 0, run["timed_out"], selected)
    if record["verdict"] == "PASS" and enumerates:
        record["verdict"] = INCONCLUSIVE_NO_RESULT
        record["reason"] = "команда перечислила тесты, но не выполнила их"
    return record
