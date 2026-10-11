#!/bin/sh
# Отмена прежней очереди Stellwerk — выполняет ВЛАДЕЛЕЦ.
#
# run-id: diag-20261011-174538
# Подготовлено аудитом. Аудит выполнить не мог: `rm *` запрещён политикой,
# обходить запрет через другой инструмент запрещено инструкцией.
#
# ВНИМАНИЕ: скрипт удаляет спецификации прежней очереди.
# Сначала пройдите ШАГ 0. Если проверка архива не пройдена — останавливайтесь.

set -e
cd "$(dirname "$0")/../../.." || exit 1

# ── ШАГ 0. ПРОВЕРКА АРХИВА (обязательно до удаления) ─────────────────────
BK="$HOME/.local/state/stellwerk/diagnostics/diag-20261011-174538"
echo "== ШАГ 0: проверка архива =="
( cd "$BK/plans-backup" && sha256sum -c "$BK/MANIFEST.sha256" )
echo "Архив цел. 396 файлов. Можно продолжать."
echo

# ── ШАГ 1. Прекратить автоматическое возобновление ───────────────────────
# boulder.json указывает active_plan на .omo/plans/M6-21-missed-and-unviable-gate.md
# Если не обнулить — указатель останется на файле, который ниже удаляется.
echo "== ШАГ 1: boulder — снять active_plan =="
jq '.status="cancelled" | .active_plan=null' .omo/boulder.json > .omo/boulder.json.new
mv .omo/boulder.json.new .omo/boulder.json
echo "  status=cancelled, active_plan=null"
echo

# ── ШАГ 2. Пакеты из канона и зеркала ────────────────────────────────────
# glob packet-*.md НЕ задевает *.result.jsonl / *.result.stderr — это evidence.
echo "== ШАГ 2: пакеты (packet-*.md) =="
find .sisyphus/plans -name 'packet-*.md' -delete
find .omo/plans      -name 'packet-*.md' -delete
echo "  канон и зеркало очищены от пакетов"
echo

# ── ШАГ 3. Спецификации и индексы ────────────────────────────────────────
# ВАЖНО: в прошлой версии этого скрипта шаг отсутствовал, из-за чего
# active_plan из boulder и milestone-планы оставались пригодными для раздачи.
# Удаляются ТОЛЬКО перечисленные пути. Evidence (*.result.*) не трогается.
echo "== ШАГ 3: спецификации и индексы прежней очереди =="

# 3a. Спецификации milestone (в т.ч. тот, на который ссылался boulder)
rm -f .omo/plans/M6-21-missed-and-unviable-gate.md \
      .omo/plans/M5-mutation-gate-and-runtime.md \
      .omo/plans/M8-refactoring-debt-first.md \
      .omo/plans/m0-registry.md \
      .omo/plans/M0-index.md \
      .omo/plans/M3/REVIEW.md

# 3b. Индексы зеркала
rm -f .omo/plans/M1/index.md .omo/plans/M2/index.md .omo/plans/M3/index.md \
      .omo/plans/M4/index.md .omo/plans/M5/index.md .omo/plans/M6/index.md \
      .omo/plans/INFRA/index.md .omo/plans/INT1/index.md

# 3c. Индексы и обзор канона
rm -f .sisyphus/plans/M0-index.md \
      .sisyphus/plans/INFRA/index.md \
      .sisyphus/plans/M1/index.md .sisyphus/plans/M2/index.md \
      .sisyphus/plans/M3/index.md .sisyphus/plans/M4/index.md \
      .sisyphus/plans/M5/index.md .sisyphus/plans/M6/index.md \
      .sisyphus/plans/M7/index.md \
      .sisyphus/plans/M3/REVIEW.md

# 3d. Черновики
rm -f .omo/pending/*.md

# 3e. Помеченные SUPERSEDED (уже вне раздачи, но оставались в зеркале)
rm -f .omo/plans/_archive/*.SUPERSEDED.md

echo "  спецификации и индексы удалены"
echo

# ── ШАГ 4. Проверка результата ───────────────────────────────────────────
echo "== ШАГ 4: что осталось в зеркале (должны быть только evidence) =="
find .omo/plans -type f | sed 's/^/    /'
echo
echo "== Проверка: не осталось ли раздаваемых .md =="
LEFT=$(find .omo/plans .sisyphus/plans -name '*.md' 2>/dev/null | wc -l)
echo "    .md файлов осталось: $LEFT  (ожидается 0)"
echo
echo "== Проверка: boulder обезврежен =="
jq -r '"    status=" + .status + "  active_plan=" + (.active_plan|tostring)' .omo/boulder.json
echo
echo "== НЕ УДАЛЕНО (по условию: результаты, evidence, runtime) =="
echo "    .stellwerk/  .pf/  .night/  docs/adr/  .omo/notepads/"
echo "    *.result.jsonl  *.result.stderr  .sisyphus/plans/.gitkeep"
echo "    53 worktree — не удаляются"