#!/usr/bin/env bash
# accept.sh — приёмка пакета после прогона. Использование:
#   scripts/integrator/accept.sh <milestone> <packet> <sandbox>
#
# Пять проверок, каждая из которых закрывает реальный промах сессии #2:
#
# (a) тест пакета запускается САМОСТОТЕЛЬНО — не по отчёту воркера, а здесь.
#     Сессия #2: milestone был зашит на M1, и M2-пакет «прошёл бы», проверив чужой
#     крейт. Milestone теперь обязателен параметр, а не константа.
#
# (b) RED и GREEN имеют РАЗНЫЕ критерии. Сессия #2: один критерий «тест зелёный»
#     на все пакеты, из-за чего RED-пакет нельзя было принять, а GREEN-пакет
#     мог пройти, ничего не проверив.
#
# (c) число assert! в тестах НЕ уменьшилось — тест не ослаблен.
#
# (d) лимит файлов ПРОВЕРЯЕТСЯ, с учётом limit_waiver. Сессия #2: лимит
#     вычислялся, но не использовался, и пакет с 4 файлами был объявлен успешным.
#
# (e) новый тестовый файл не даёт ложной тревоги: файла нет в базе → сравнивать
#     не с чем, и отсутствие считается нормой, а не ослаблением.
#
# Код возврата: 0 — принято; 1 — отклонено (каждая проверка печатает причину).
set -uo pipefail

MILESTONE="${1:?milestone обязателен (M1|M2|M3)}"
PACKET="${2:?packet обязателен (packet-NN)}"
SANDBOX="${3:?sandbox обязателен}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BASE_REF="${STELLWERK_BASE:-main}"
PLANS_DIR="${STELLWERK_PLANS_DIR:-$ROOT/.sisyphus/plans}"
PKT_FILE="$PLANS_DIR/$MILESTONE/$PACKET.md"
MAX_FILES=3

fails=0
ok()   { printf '  \033[32mok\033[0m   %s\n' "$1"; }
bad()  { fails=$((fails+1)); printf '  \033[31mFAIL\033[0m %s\n' "$1"; }

[ -f "$PKT_FILE" ]     || { echo "accept: нет пакета $PKT_FILE" >&2; exit 1; }
[ -d "$SANDBOX" ]      || { echo "accept: нет песочницы $SANDBOX" >&2; exit 1; }

field() { grep -m1 "^$1:" "$PKT_FILE" | sed "s/^$1: *//"; }

# Тело markdown-секции: от `## <heading>` до следующего `## `. Заголовок может
# иметь уточнение в скобках (`## Приёмка (владелец запускает сам)`).
has_section() { grep -qE "^## $1" "$PKT_FILE"; }
section_body() {
  awk -v pat="$1" '
    $0 ~ ("^## " pat) { inside = 1; next }
    inside && /^## / { exit }
    inside { print }
  ' "$PKT_FILE"
}
first_cargo_test() {
  awk '{
    line = $0
    sub(/^[[:space:]]+/, "", line)
    sub(/^\$[[:space:]]+/, "", line)
    if (line ~ /^cargo test([[:space:]]|$)/) {
      sub(/[[:space:]]*;.*$/, "", line)
      print line
      exit
    }
  }'
}
join_lines() {
  awk 'NF { out = (out == "" ? $0 : out ", " $0) } END { print out }'
}
# N2: frontmatter `allowed_changes:` — multiline `- path` или inline `[a, b]`.
files_from_frontmatter() {
  awk '
    /^---[[:space:]]*$/ { fm++; next }
    fm == 1 && /^allowed_changes:/ {
      line = $0
      sub(/^allowed_changes:[[:space:]]*/, "", line)
      if (line ~ /^\[/) {
        gsub(/[\[\]]/, "", line)
        n = split(line, a, ",")
        for (i = 1; i <= n; i++) {
          gsub(/^[[:space:]]+|[[:space:]]+$/, "", a[i])
          if (a[i] != "") print a[i]
        }
        next
      }
      inlist = 1
      next
    }
    fm == 1 && inlist && /^[[:space:]]+-[[:space:]]/ {
      line = $0
      sub(/^[[:space:]]+-[[:space:]]*/, "", line)
      print line
      next
    }
    fm == 1 && inlist && /^[^[:space:]]/ { inlist = 0 }
  ' "$PKT_FILE"
}
# N1/TUI: `## files_allowed` — bullets, путь в backticks или до «—».
files_from_section() {
  section_body "files_allowed" | awk '
    /^[[:space:]]*-[[:space:]]/ {
      line = $0
      sub(/^[[:space:]]*-[[:space:]]*/, "", line)
      if (match(line, /`[^`]+`/)) {
        line = substr(line, RSTART + 1, RLENGTH - 2)
      } else {
        sub(/[[:space:]]+—.*$/, "", line)
      }
      print line
    }'
}

TEST_CMD=""
FILES_ALLOWED=""
WAIVER="$(field limit_waiver)"
GOAL="$(field goal)"
ACCEPT="$(field acceptance)"

if has_section "files_allowed"; then
  FILES_ALLOWED="$(files_from_section | join_lines)"
elif grep -q '^allowed_changes:' "$PKT_FILE"; then
  FILES_ALLOWED="$(files_from_frontmatter | join_lines)"
else
  FILES_ALLOWED="$(field files_allowed)"
fi
# N1/TUI пишут waiver жирным: `**limit_waiver:** ...`.
if [ -z "$WAIVER" ]; then
  WAIVER="$(grep -m1 '^\*\*limit_waiver:\*\*' "$PKT_FILE" \
            | sed 's/^\*\*limit_waiver:\*\*[[:space:]]*//')"
fi

# ── (a)+(b) критерий RED/GREEN ──────────────────────────────────────────────
# RED: есть секция `## RED` либо legacy goal/acceptance начинается с RED.
is_red=0
if has_section "RED"; then
  is_red=1
else
  case "$(printf '%s' "$GOAL" | tr '[:lower:]' '[:upper:]')" in
    RED*) is_red=1 ;;
  esac
  case "$(printf '%s' "$ACCEPT" | tr '[:lower:]' '[:upper:]')" in
    RED*) is_red=1 ;;
  esac
fi

# Команда: RED — первая `cargo test` из `## RED`; GREEN — из `## Приёмка`.
# Legacy-пакеты без секций читают поле `test:`.
if [ "$is_red" = 1 ]; then
  if has_section "RED"; then
    TEST_CMD="$(section_body "RED" | first_cargo_test)"
  else
    TEST_CMD="$(field test)"
  fi
else
  if has_section "Приёмка"; then
    TEST_CMD="$(section_body "Приёмка" | first_cargo_test)"
  else
    TEST_CMD="$(field test)"
  fi
fi

echo "=== accept: $MILESTONE/$PACKET ==="
echo "  test:        ${TEST_CMD:-NO_TEST_COMMAND}"
echo "  files:       $FILES_ALLOWED"
[ -n "$WAIVER" ] && echo "  limit_waiver: $WAIVER"

# Нет `cargo test` — это отдельное состояние, а не «принято по умолчанию».
if [ -z "$TEST_CMD" ]; then
  bad "(a/b) NO_TEST_COMMAND: в пакете нет cargo test — приёмка невозможна"
  printf '\naccept: ОТКЛОНЕНО (NO_TEST_COMMAND)\n'
  exit 1
fi

RUNDIR=$(mktemp -d /tmp/accept-XXXXXX)
read -r -a TEST_ARGV <<< "$TEST_CMD"
if (cd "$SANDBOX" && "${TEST_ARGV[@]}") > "$RUNDIR/test.log" 2>&1; then
  TEST_RC=0
else
  TEST_RC=$?
fi
RESULT_LINE="$(grep -E '^test result:' "$RUNDIR/test.log" | head -1)"

if [ "$is_red" = 1 ]; then
  echo "  (a/b) RED-пакет: ожидается ПАДЕНИЕ"
  if [ "$TEST_RC" -eq 0 ]; then
    bad "(b) RED-пакет прошёл — приёмка инвертирована? Проверь пакет."
  elif grep -qE 'error\[E[0-9]+\]|error: could not compile' "$RUNDIR/test.log"; then
    bad "(b) падение на компиляции, а не на заглушке/assert"
    head -12 "$RUNDIR/test.log" | sed 's/^/        /'
  else
    ok "(b) упал как задумано: $(grep -oE 'not (yet )?implemented[^"]*' "$RUNDIR/test.log" | head -1)"
  fi
else
  echo "  (a/b) GREEN-пакет: ожидается ПРОХОЖДЕНИЕ"
  if [ "$TEST_RC" -ne 0 ]; then
    bad "(a) тест пакета не прошёл (rc=$TEST_RC): $RESULT_LINE"
    grep -E '^(test|error)' "$RUNDIR/test.log" | head -8 | sed 's/^/        /'
  elif ! printf '%s' "$RESULT_LINE" | grep -qE '^test result: ok\.'; then
    bad "(a) тест отработал, но строки 'test result: ok.' нет — вывод неинформативен"
  elif ! printf '%s' "$RESULT_LINE" | grep -qE '[1-9][0-9]* passed'; then
    bad "(a) тест зелёный, но ни одного выполненного теста ($RESULT_LINE) — приёмка пустая"
  else
    ok "(a) $RESULT_LINE"
  fi
fi

# ── (c) целостность: число assert! не уменьшилось ───────────────────────────
if [ -d "$SANDBOX/crates" ]; then
  weakened=""
  while IFS= read -r f; do
    rel="${f#$SANDBOX/}"
    if git -C "$ROOT" cat-file -e "$BASE_REF:$rel" 2>/dev/null; then
      before=$(git -C "$ROOT" show "$BASE_REF:$rel" | grep -cE 'assert!|assert_eq!|panic!')
      after=$(grep -cE 'assert!|assert_eq!|panic!' "$f")
      [ "$before" != "$after" ] && weakened="$weakened $rel($before→$after)"
    fi
    # файла нет в базе → новый тест, сравнивать не с чем: это норма (e)
  done < <(find "$SANDBOX/crates" -path '*/tests/*.rs' -type f 2>/dev/null | sort)
  if [ -n "$weakened" ]; then
    bad "(c) число проверок изменилось:$weakened"
  else
    ok "(c) тесты не ослаблены (новые файлы пропущены корректно)"
  fi
fi

# ── (d) лимит файлов ПРОВЕРЯЕТСЯ ────────────────────────────────────────────
# Точный список изменённых файлов берём из артефактов диспатча. Fallback —
# только исходники: target/ содержит десятки артефактов сборки, и их подсчёт
# давал бы бессмысленное «36 файлов».
if [ -f "$SANDBOX.run/sandbox.diff" ]; then
  # Путь — ВТОРОЕ поле строки '+++ b/<path>'; брать $1 нельзя: это литерал '+++',
  # и все файлы схлопывались бы в один уникальный элемент.
  NF=$(grep -E '^\+\+\+ ' "$SANDBOX.run/sandbox.diff" 2>/dev/null \
        | awk '{print $2}' | sed "s|^$SANDBOX/||" | sort -u | wc -l | tr -d ' ')
elif [ -d "$SANDBOX/.git" ] || [ "$SANDBOX" = "$ROOT" ]; then
  # Пакет реализован интегратором прямо в репозитории: точная мера — git.
  # Файловый fallback здесь давал бы мусор (при «песочнице = корень репо» всё
  # созданное позже Cargo.toml считается изменением).
  NF=$(git -C "$SANDBOX" status --porcelain 2>/dev/null | wc -l | tr -d ' ')
else
  NF=0
fi
NF="${NF:-0}"
if [ "$NF" -gt "$MAX_FILES" ] && [ -z "$WAIVER" ]; then
  bad "(d) $NF файлов (лимит $MAX_FILES) и нет поля limit_waiver в пакете"
elif [ "$NF" -gt "$MAX_FILES" ] && [ -n "$WAIVER" ]; then
  ok "(d) $NF файлов > $MAX_FILES, но limit_waiver присутствует: $WAIVER"
else
  ok "(d) $NF файлов (лимит $MAX_FILES)"
fi

printf '\n'
if [ "$fails" -gt 0 ]; then
  printf 'accept: ОТКЛОНЕНО (%d проверок не пройдено)\n' "$fails"
  exit 1
fi
printf 'accept: ПРИНЯТО\n'
exit 0
