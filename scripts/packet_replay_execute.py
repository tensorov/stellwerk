"""Песочница прогона: копия дерева репозитория через `git archive HEAD`.

Отдельный модуль, потому что это другой предмет, чем разбор команд и арифметика
строк результата. Здесь нет ни печатных отчётов, ни классификации — только
извлечение состояния репозитория в каталог, безопасный для исполнения.

Зачем это нужно: прогон acceptance-команды не имеет права изменить рабочее дерево.
`git archive` выгружает только отслеживаемые файлы, поэтому в песочнице нет ни
`.git`, ни возможности дописать в историю. Копия `cp -r` для этого не годится:
она тащит за собой `.git` целиком.

Правила, которые модуль обязан соблюдать:

* ни одна команда не выполняется shell-строкой — только список аргументов;
* песочница живёт вне рабочего дерева, иначе прогон испортит исходники;
* каталог песочницы печатается машиночитаемо, а не разбирается из прозаического
  вывода: путь должен потребляться без вычислений на стороне вызывающего.
"""

from __future__ import annotations

import shutil
import subprocess
import tempfile
from pathlib import Path

WORKSPACE = Path(__file__).resolve().parent.parent

SANDBOX_PREFIX = "stellwerk-replay-"


def _run(argv: list[str], repo: Path, stdin: bytes | None = None) -> bytes:
    """Выполнить одну внешнюю команду и вернуть её stdout.

    `shell` не используется: список аргументов передаётся как есть. Это не
    параноия — ровно этот запрет закрывает пробел, где строка команды из packet-файла
    могла бы превратиться в произвольный shell.
    """
    done = subprocess.run(
        argv,
        cwd=repo,
        input=stdin,
        capture_output=True,
        check=False,
    )
    if done.returncode != 0:
        detail = done.stderr.decode("utf-8", "replace").strip()
        raise RuntimeError(f"{argv[0]} не отработал: {detail or 'без вывода'}")
    return done.stdout


def make_sandbox(repo: Path | None = None) -> Path:
    """Создать каталог и распаковать в него `git archive HEAD`.

    Возвращается путь, который ещё существует: уборка — дело вызывающего, потому
    что только вызывающий знает, когда проверка закончилась.
    """
    source = repo or WORKSPACE
    archive = _run(["git", "archive", "--format=tar", "HEAD"], source)
    target = Path(tempfile.mkdtemp(prefix=SANDBOX_PREFIX))
    try:
        _run(["tar", "--extract", "--file", "-", "--directory", str(target)], source, archive)
    except RuntimeError:
        # Каталог без распакованного дерева бесполезен и только занимает место:
        # оставить его значит оставить на диске то, что никто не удалит.
        shutil.rmtree(target, ignore_errors=True)
        raise
    return target


def sandbox_report(sandbox: Path) -> str:
    """Строка, по которой вызывающий узнаёт путь песочницы."""
    return f"SANDBOX={sandbox}"