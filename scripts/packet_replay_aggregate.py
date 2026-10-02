"""Арифметика прогона: суммирование строк `test result` и итоговая классификация.

Отдельный модуль, потому что это другой предмет, чем разбор команд: здесь нет ни
argv, ни пакетов, ни песочницы — только строки вывода и правило, по которому из них
получается вердикт.

Правило, ради которого модуль выделен: `filtered > 0` сам по себе не ослабляет
результат. Команда, отобравшая подмножество и выполнившая часть, проверяла именно это
подмножество. Ослабляет только `passed == 0` — тогда не выполнено ничего, и «зелёный»
код возврата ничего не доказывает.

Ни одна строка вывода не учитывается дважды и ни одна не теряется: берутся ВСЕ
`test result:`, потому что взятие первой или последней давало ложный итог.
"""

from __future__ import annotations

import re
import sys


RESULT_LINE = re.compile(
    r"test result:\s*(?P<verdict>ok|FAILED)\.\s+"
    r"(?P<passed>\d+)\s+passed;\s+"
    r"(?P<failed>\d+)\s+failed;\s+"
    r"(?P<ignored>\d+)\s+ignored;\s+"
    r"(?P<measured>\d+)\s+measured;\s+"
    r"(?P<filtered>\d+)\s+filtered out"
)

PASS = "PASS"
FAIL = "FAIL"
INCONCLUSIVE_ZERO_EXECUTED = "INCONCLUSIVE_ZERO_EXECUTED"
INCONCLUSIVE_NO_RESULT = "INCONCLUSIVE_NO_RESULT"
DISCOVERY_FAILED = "DISCOVERY_FAILED"


def parse_result_lines(text: str) -> dict[str, int]:
    """Просуммировать ВСЕ строки `test result:`.

    Каждая физическая строка учитывается ровно один раз, поэтому повтор той же строки
    дважды в выводе считается дважды — как и было на самом деле. Ни первую, ни
    последнюю строку отдельно брать нельзя: это теряло остальные бинарники.
    """
    totals = {
        "passed": 0,
        "failed": 0,
        "ignored": 0,
        "measured": 0,
        "filtered": 0,
        "result_lines": 0,
    }
    for line in text.splitlines():
        found = RESULT_LINE.search(line)
        if not found:
            continue
        totals["result_lines"] += 1
        totals["passed"] += int(found.group("passed"))
        totals["failed"] += int(found.group("failed"))
        totals["ignored"] += int(found.group("ignored"))
        totals["measured"] += int(found.group("measured"))
        totals["filtered"] += int(found.group("filtered"))
    return totals


def classify(
    totals: dict[str, int],
    exit_code: int,
    timed_out: bool,
    selected_by_list: int | None,
) -> str:
    """Итог по правилам брифа.

    `filtered > 0` сам по себе не ослабляет результат: команда, отобравшая подмножество
    и выполнившая часть, проверяла именно это подмножество. Ослабляет только
    `passed == 0` — тогда не выполнено ничего, и «зелёный» код ничего не доказывает.
    """
    if timed_out:
        return FAIL
    if exit_code != 0:
        return FAIL
    if totals["failed"] > 0:
        return FAIL
    if selected_by_list is not None and selected_by_list == 0 and totals["result_lines"] == 0:
        return DISCOVERY_FAILED
    if totals["result_lines"] == 0:
        return INCONCLUSIVE_NO_RESULT
    if totals["passed"] == 0 and (totals["filtered"] > 0 or selected_by_list == 0):
        return INCONCLUSIVE_ZERO_EXECUTED
    if totals["passed"] > 0:
        return PASS
    return INCONCLUSIVE_NO_RESULT


def aggregate(exit_code: int, timed_out: bool) -> int:
    """Принять вывод прогона на stdin и напечатать итог."""
    text = sys.stdin.read()
    totals = parse_result_lines(text)
    # Discovery здесь не выполнялся, поэтому честный ответ — «неизвестно», то есть
    # null. Подставлять выведенное из строк число значило бы выдать догадку за
    # измерение: при result_lines == 0 такой ноль дал бы ложный DISCOVERY_FAILED
    # вместо честного INCONCLUSIVE_NO_RESULT.
    selected: int | None = None
    verdict = classify(totals, exit_code, timed_out, selected)

    print(f"EXIT_CODE={exit_code}")
    print(f"TIMED_OUT={str(timed_out).lower()}")
    print(f"RESULT_LINES={totals['result_lines']}")
    print(f"PASSED={totals['passed']}")
    print(f"FAILED={totals['failed']}")
    print(f"IGNORED={totals['ignored']}")
    print(f"MEASURED={totals['measured']}")
    print(f"FILTERED={totals['filtered']}")
    print(f"SELECTED_BY_LIST={'null' if selected is None else selected}")
    print(f"CLASS={verdict}")
    return 0
