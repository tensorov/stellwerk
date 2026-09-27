#!/usr/bin/env bash
# Цикл раздачи одного пакета M1: dispatch -> отчёт артефактов.
# Приёмку (тест, целостность, лимиты, merge) делает интегратор отдельно.
set -u
LANE="$1"; PKT="$2"
ROOT=/home/izyabretatel/gits/stellwerk
LOG="$ROOT/.night/runs/${PKT}.log"
mkdir -p "$ROOT/.night/runs"
cd "$ROOT" || exit 1
echo "=== dispatch: lane=$LANE packet=$PKT ==="
STELLWERK_PACKET_DIR="$ROOT/.sisyphus/plans/M1" \
STELLWERK_PACKET_TIMEOUT="${STELLWERK_PACKET_TIMEOUT:-1500}" \
  timeout 1700 "$HOME/.local/bin/stellwerk-dispatch" "$LANE" "$PKT" > "$LOG" 2>&1
echo "dispatch exit=$?"
grep -E "sandbox:|pi exit=|VALIDITY GATE|files_changed|insertions|STDOUT" "$LOG" | sed 's/^/  /'
SB=$(grep -oE "/tmp/stellwerk-sandbox/[^ ]+" "$LOG" | head -1)
echo "sandbox=$SB"
V="$SB.run/validity.json"
if [ -f "$V" ]; then
  echo "--- validity.json ---"
  cat "$V" | tr -d '\n' | sed 's/  */ /g'; echo
fi
echo "--- numstat (лимит: <=3 файла, <=200 строк) ---"
cat "$SB.run/sandbox.numstat" 2>/dev/null | sed 's/^/  /'
echo "--- какие файлы изменены ---"
grep -E "^\+\+\+ " "$SB.run/sandbox.diff" 2>/dev/null | sed "s|$SB/||;s/^/  /" | sort -u
echo "$SB" > "$ROOT/.night/runs/${PKT}.sandbox"
