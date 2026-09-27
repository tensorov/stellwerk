#!/usr/bin/env bash
# plan-lint.sh — приёмка пакета до выдачи воркеру.
#
# Сессия #3: реализация перенесена в scripts/plan-lint.py (stdlib, без
# зависимостей). Причина — прежняя версия проверяла только .sisyphus/plans/
# верхнего уровня и не знала про вложенную раскладку .sisyphus/plans/<M>/,
# где живут 36 пакетов M1–M3. Фактически она молча пропускала всё, что важно.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [ "${1:-}" = "--self-test" ]; then
  exec python3 "$HERE/plan-lint.py" --self-test
fi
exec python3 "$HERE/plan-lint.py" "$@"
