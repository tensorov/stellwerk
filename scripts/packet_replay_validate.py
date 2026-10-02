"""Семантика команды `cargo test`: позиционный фильтр против имени target'а.

Ключевое различение, ради которого модуль выделен: в
`cargo test -p stellwerk-registry measure` слово `measure` — позиционный TESTNAME-фильтр,
а НЕ имя бинарника и не target. Cargo отбирает тесты по имени и, если таких нет,
не выполняет ничего, но завершается с кодом 0. Поэтому такая команда зелёная и при этом
ничего не проверила.

Существование package и target проверяется через `cargo metadata`: имя файла из строки
команды выводить нельзя, именно этот приём дал ложную находку про несуществующие тесты.

Ничего не исполняется: только решение о том, можно ли запускать и что именно она выберет.
"""

from __future__ import annotations

import fnmatch
import json
import re
import shlex
import subprocess
from pathlib import Path

WORKSPACE = Path(__file__).resolve().parent.parent


# Параметры, позволяющие выйти из песочницы или подменить исполняемую конфигурацию
# Cargo. Отвергаются целиком: их присутствие делает результат команды непригодным
# для проверки именно этого workspace.


FORBIDDEN_FLAGS = (
    "--manifest-path",
    "--config",
    "--target-dir",
    "--lockfile-path",
    "-Z",
)

# Первые два элемента команды: программа и подкоманда. Дальше — только известное.
CARGO_TEST_HEAD = ("cargo", "test")

# Shell-операторы. Их присутствие означает, что перед нами не одна команда, а
# конвейер, и запуск через оболочку такой текст исполнил бы, что и запрещено.
SHELL_OPERATORS = ("|", "||", "&&", ";", ">", "<", "$(", "`", "&", "\n", "\r")

VALID_COMMAND = "VALID_COMMAND"
INVALID_PACKAGE = "INVALID_PACKAGE"
INVALID_TARGET = "INVALID_TARGET"
UNSUPPORTED_COMMAND = "UNSUPPORTED_COMMAND"
PARSE_ERROR = "PARSE_ERROR"

# Значения, принимаемые за target. Позиционное слово сюда НЕ входит: это фильтр.
TARGET_FLAGS = ("--test", "--bin")


def load_metadata() -> dict:
    """Прочитать `cargo metadata` один раз на запуск.

    Единственный источник правды о том, какие package и target существуют: имя
    файла из строки команды или из имени пакета выводить нельзя, именно этот
    приём дал ложную находку про отсутствующие тесты M1.

    Корень берётся из расположения самого скрипта, а не из аргумента вызывающего:
    путь к workspace не должен задаваться извне, иначе проверку можно перенаправить
    на чужой проект и получить правдивый ответ не о том, что спрашивали.
    """
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=WORKSPACE,
        capture_output=True,
        text=True,
        check=False,
    )
    if out.returncode != 0:
        return {}
    try:
        return json.loads(out.stdout)
    except json.JSONDecodeError:
        return {}


def metadata_index(meta: dict) -> tuple[dict[str, set[str]], dict[str, set[str]]]:
    """Собрать индексы: package → targets, package → targets по всей метаданной."""
    packages: dict[str, set[str]] = {}
    targets: dict[str, set[str]] = {}
    for pkg in meta.get("packages", []):
        name = pkg.get("name")
        if not name:
            continue
        names: set[str] = set()
        for target in pkg.get("targets", []):
            kinds = target.get("kind") or []
            for kind in kinds:
                names.add(f"{kind}:{target.get('name')}")
            names.add(str(target.get("name")))
        packages[name] = names
        targets[name] = names
    return packages, targets


def classify_command(command: str, packages: dict[str, set[str]]) -> dict:
    """Разобрать команду семейства `cargo test` и сказать, что она означает.

    Возвращает словарь со статусом, пакетом, целью и позиционным фильтром. Ничего не
    исполняет: это решение о том, можно ли вообще запускать команду и что именно
    она выберет.
    """
    verdict: dict[str, object] = {
        "status": UNSUPPORTED_COMMAND,
        "package": "",
        "target": "",
        "filter": "",
        "reason": "",
    }

    if any(op in command for op in SHELL_OPERATORS):
        verdict["reason"] = "shell-оператор или перевод строки в команде"
        return verdict
    if not packages:
        verdict["reason"] = "не удалось прочитать cargo metadata"
        return verdict

    try:
        argv = shlex.split(command)
    except ValueError as exc:
        verdict["status"] = PARSE_ERROR
        verdict["reason"] = f"аргументы не разобраны: {exc}"
        return verdict

    if len(argv) < 2 or tuple(argv[:2]) != CARGO_TEST_HEAD:
        verdict["reason"] = "не семейство cargo test"
        return verdict

    package = ""
    target = ""
    positional: list[str] = []
    index = 2
    after_sep = False
    saw_sep = False

    while index < len(argv):
        token = argv[index]
        if token == "--":
            saw_sep = True
            after_sep = True
            index += 1
            continue
        if not after_sep:
            if token in FORBIDDEN_FLAGS:
                verdict["reason"] = f"параметр {token} меняет конфигурацию или песочницу"
                return verdict
            if token in ("-p", "--package"):
                index += 1
                package = argv[index] if index < len(argv) else ""
                index += 1
                continue
            if token.startswith("--package="):
                package = token.split("=", 1)[1]
                index += 1
                continue
            if token in TARGET_FLAGS:
                index += 1
                target = argv[index] if index < len(argv) else ""
                index += 1
                continue
            if token.startswith("--test=") or token.startswith("--bin="):
                target = token.split("=", 1)[1]
                index += 1
                continue
            if token.startswith("-"):
                index += 1
                continue
            positional.append(token)
            index += 1
            continue
        index += 1

    if package and package not in packages:
        verdict["status"] = INVALID_PACKAGE
        verdict["reason"] = f"пакета {package} нет в cargo metadata"
        return verdict

    if target:
        known = packages.get(package, set())
        if target in known:
            verdict["status"] = VALID_COMMAND
            verdict["package"] = package
            verdict["target"] = target
            return verdict
        if any(ch in target for ch in "*?["):
            # Цель задана образцом: точное имя не обязано существовать, но неоднозначные
            # образцы гадать нельзя — это отказ, а не «наверное подойдёт».
            verdict["reason"] = "образец target'а неоднозначен для проверки"
            return verdict
        verdict["status"] = INVALID_TARGET
        verdict["reason"] = f"цели {target} нет в пакете {package}"
        return verdict

    verdict["status"] = VALID_COMMAND
    verdict["package"] = package
    verdict["target"] = ""
    verdict["filter"] = " ".join(positional)
    return verdict


def validate(rows: list[dict[str, str]]) -> int:
    """Напечатать семантику каждой команды. Ничего не исполняется.

    Строки передаёт точка входа: обход файлов плана — не забота валидатора, и
    пытаться сделать его здесь означало бы либо циклический импорт, либо копипаст
    разбора в двух местах.
    """
    packages, _ = metadata_index(load_metadata())
    # Число прочитанных пакетов печатается всегда: пустой индекс при успешном запуске
    # и пустой индекс при непрочитанной metadata выглядели бы одинаково, а означают
    # разное — первое про формат команды, второе про сломанное окружение.
    print(f"METADATA_PACKAGES={len(packages)}")
    for row in rows:
        verdict = classify_command(row["command"], packages)
        print(
            f"{verdict['status']}\t{row['milestone']}/{row['packet_id']}"
            f"\tPACKAGE={verdict['package'] or '-'}"
            f"\tTARGET={verdict['target'] or '-'}"
            f"\tFILTER={verdict['filter'] or '-'}"
            f"\t{verdict['reason'] or row['command']}"
        )
    return 0
