#!/usr/bin/env bash
# Полный цикл одного пакета: dispatch -> приёмка (5 проверок) -> merge --no-ff.
# Возвращает 0 при полном успехе, иначе ненулевой код.
set -u
LANE="$1"; PKT="$2"; BR="m1/$(echo "$LANE" | tr '/' '-')-$PKT"
ROOT=/home/izyabretatel/gits/stellwerk
R="$ROOT/.night/runs"; mkdir -p "$R"; cd "$ROOT" || exit 9

if [ "${3:-}" = "--accept-only" ]; then
  SB=$(grep -oE "/tmp/stellwerk-sandbox/[^ ]+" "$R/$PKT.log" | head -1)
  echo "### $PKT — приёмка существующей песочницы (dispatch не повторяется)"
else
echo "### $PKT (lane $LANE)"
STELLWERK_PACKET_DIR="$ROOT/.sisyphus/plans/M1" STELLWERK_PACKET_TIMEOUT=1500 \
  timeout 1700 "$HOME/.local/bin/stellwerk-dispatch" "$LANE" "$PKT" > "$R/$PKT.log" 2>&1
RC=$?
grep -E "VALIDITY GATE" "$R/$PKT.log" | sed 's/^/  gate: /'
SB=$(grep -oE "/tmp/stellwerk-sandbox/[^ ]+" "$R/$PKT.log" | head -1)
[ -n "$SB" ] || { echo "  FAIL: нет песочницы (dispatch rc=$RC)"; tail -5 "$R/$PKT.log" | sed 's/^/    /'; exit 1; }
fi
V=$(cat "$SB.run/validity.json" 2>/dev/null)
echo "$V" | grep -q '"valid": true' || { echo "  FAIL: gate невалиден: $V"; exit 1; }
# (a) собственный тест пакета из его frontmatter
TEST=$(grep -m1 '^test: ' ".sisyphus/plans/M1/$PKT.md" | sed 's/^test: //')
echo "  (b) тест пакета: $TEST"
( cd "$SB" && bash -c "$TEST" 2>&1 | grep -E "^test result" | sed 's/^/      /' ) || true
RES=$( cd "$SB" && bash -c "$TEST" 2>&1 | grep -E "^test result" | head -1 )
echo "$RES" | grep -qE "^test result: ok\. [1-9][0-9]* passed; 0 failed" || { echo "  FAIL(b): тест не зелёный: $RES"; exit 2; }
# (c) целостность: тестовые файлы не менялись
mapfile -t TF < <(cd "$SB" && find crates -path '*/tests/*.rs' -type f | sort)
WEAK=0
for f in "${TF[@]}"; do
  A=$(grep -cE "assert!|assert_eq!|panic!" "$SB/$f" 2>/dev/null || echo 0)
  git cat-file -e "main:$f" 2>/dev/null || continue   # новый файл: не с чем сравнивать
  B=$(git show "main:$f" | grep -cE "assert!|assert_eq!|panic!" || echo 0)
  [ "$A" != "$B" ] && { echo "  FAIL(c): $f assert $B -> $A (ослабление)"; WEAK=1; }
done
[ "$WEAK" = 0 ] && echo "  (c) целостность: тесты не тронуты"
# (d) лимиты
NS=$(cat "$SB.run/sandbox.numstat" 2>/dev/null)
INS=$(echo "$NS" | grep -oE '[0-9]+ insertions' | grep -oE '[0-9]+' || echo 0)
DEL=$(echo "$NS" | grep -oE '[0-9]+ deletions' | grep -oE '[0-9]+' || echo 0)
NF=$(grep -cE '^\+\+\+ ' "$SB.run/sandbox.diff" 2>/dev/null || echo 0)
NF=$(grep -E '^\+\+\+ ' "$SB.run/sandbox.diff" 2>/dev/null | sort -u | wc -l)
TOT=$((INS + DEL))
echo "  (d) лимиты: файлов=$NF/3, строк=$TOT/200"
LIMIT_OK=1
[ "$NF" -gt 3 ] && LIMIT_OK=0
[ "$TOT" -gt 200 ] && LIMIT_OK=0
# (e) merge
bash .night/merge-packet.sh "$SB" "$PKT" "$BR" "feat($(echo $PKT | sed 's/packet-//')): $PKT

Приёмка: $TEST → $RES
Целостность: тестовые файлы не изменены.
Лимиты: $NF файл(ов), $TOT строк.
Packet-Result: DONE
Dispatch: VALID
Packet: M1/$PKT, lane $LANE" 2>&1 | tail -2 | sed 's/^/  /'
echo "  ИТОГ: $(just check-status | grep core | sed 's/^ *//')"
just check >/dev/null 2>&1 && echo "  gate: PASS" || { echo "  gate: FAIL"; exit 3; }
echo "  $PKT: OK"
