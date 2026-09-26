#!/usr/bin/env bash
# run-one.sh — Phase B/C. Один прогон: arm ∈ {baseline,solpi}, workload ∈ {W1,W2}.
#
# Инварианты:
#  * umask 077 — pi-home и всё, что с ключами, создаётся 600/700;
#  * ПЕСОЧНИЦА = `git archive BASE | tar -x` ВНЕ репозитория: .git нет вообще,
#    поэтому ничто не указывает на хост-репо;
#  * эталонный слепок BASE рядом; метрики diff = сравнение деревьев (dir_diff.py);
#  * CARGO_TARGET_DIR уникален на прогон, вне репозитория, удаляется после метрик;
#  * сохраняются run.jsonl, run.stderr, tests.txt, metrics.json, diff.txt, session/
#    (в session/ лежит ledger.jsonl ObservationPack — доказательство активации);
#  * session-dir на каждый rep, персистентная сессия (SoL-Pi требует persistent session);
#  * команда pi ИДЕНТИЧНА в обоих плечах — отличается только PI_CODING_AGENT_DIR;
#  * таймаут: W1=300s, W2=900s, по таймауту SIGTERM → 10s → SIGKILL;
#  * net_path снимается ДО прогона; смена пути внутри матрицы = прогоны не сравнимы;
#  * integrity.sh ДО и ПОСЛЕ; FAIL → немедленный выход.
set -uo pipefail
umask 077

ARM="${1:?arm}"; WL="${2:?workload}"; REP="${3:?rep}"; TS="${4:?ts}"
ATTEMPT="${5:-1}"; RETRY_OF="${6:-}"
REPO="${STELLWERK_REPO:-/home/izyabretatel/gits/stellwerk}"
BASE="2d572f1"
ROOT="$REPO/.ablation-solpi"
PI="$HOME/.pi/agent/bin/pi"
LEDGER="${LEDGER:-$ROOT/runs/$TS/ledger.tsv}"
RUN="$ROOT/runs/$TS/$ARM/$WL/r$REP"
[ "$ATTEMPT" -gt 1 ] && RUN="$RUN/try$ATTEMPT"
SESSION="$RUN/session"
PIHOME="$ROOT/pi-home-$ARM"
SBOX_ROOT="/tmp/opencode/ablation-sandbox/$TS"
TAG="$ARM-$WL-r$REP"
[ "$ATTEMPT" -gt 1 ] && TAG="$TAG-try$ATTEMPT"
SBOX="$SBOX_ROOT/$TAG/wt"
SBOX_BASE="$SBOX_ROOT/$TAG/base"
CARGO_TARGET="/tmp/opencode/ablation-target/$TS/$TAG"

MICRO='Ты micro-worker Stellwerk. Выполни ровно один packet ниже. Не читай другие packets и планы.
Хвост ответа строго: STATUS/FILES/TEST/EVIDENCE.'

case "$WL" in
  W1) TIMEOUT=300; TEST_CMD=(cargo test -p stellwerk-registry model) ;;
  W2) TIMEOUT=900; TEST_CMD=(cargo test -p stellwerk-registry measure) ;;
  *)  echo "unknown workload: $WL"; exit 2 ;;
esac

export CI=true DEBIAN_FRONTEND=noninteractive GIT_TERMINAL_PROMPT=0
export GIT_EDITOR=: EDITOR=: VISUAL='' GIT_SEQUENCE_EDITOR=: GIT_MERGE_AUTOEDIT=no
export GIT_PAGER=cat PAGER=cat

echo "== run: arm=$ARM workload=$WL rep=$REP ts=$TS attempt=$ATTEMPT timeout=${TIMEOUT}s"
mkdir -p "$RUN" "$SESSION" "$(dirname "$LEDGER")"

NET_PATH=$(python3 -c "
import socket, ipaddress
try:
    ip = socket.gethostbyname('api.neuraldeep.ru')
except Exception:
    print('unknown'); raise SystemExit
o = ipaddress.ip_address(ip)
kind = 'fake-ip' if o in ipaddress.ip_network('198.18.0.0/15') else 'real-ip'
print('%s:%s' % (kind, ip))
")
NET_TUN=$(ls /sys/class/net 2>/dev/null | grep -E '^(tun|neko|wg|utun|ngl|nap)' | tr '\n' ',' | sed 's/,$//')
NET_TUN="${NET_TUN:-none}"
echo "== net_path=$NET_PATH tun=$NET_TUN =="

bash "$ROOT/scripts/integrity.sh" check || { echo "STOP: integrity не прошёл ДО прогона"; exit 1; }

echo "== path-guard =="
for src in "$ROOT/workloads/$WL.md" "$MICRO"; do
  if grep -qE "$HOME|/home/[a-z]+/" "$src" 2>/dev/null; then
    echo "STOP: в промпте есть абсолютный путь ($(basename "$src"))"; exit 1
  fi
done
echo "path-guard OK: абсолютных путей в промпте нет"

rm -rf "$SBOX" "$SBOX_BASE" "$CARGO_TARGET" 2>/dev/null
mkdir -p "$SBOX" "$SBOX_BASE" "$CARGO_TARGET"
if ! git -C "$REPO" archive "$BASE" | tar -x -C "$SBOX" 2>"$RUN/archive.log"; then
  echo "STOP: git archive не удался"; cat "$RUN/archive.log"; exit 1
fi
git -C "$REPO" archive "$BASE" | tar -x -C "$SBOX_BASE"
[ -e "$SBOX/.git" ] && { echo "STOP: в песочнице есть .git"; exit 1; }
BADLINKS=$(find "$SBOX" -type l 2>/dev/null | while read -r l; do
  t=$(readlink -f "$l" 2>/dev/null); case "$t" in "$SBOX"/*) ;; *) printf '%s -> %s\n' "$l" "$t";; esac
done)
if [ -n "$BADLINKS" ]; then echo "STOP: симлинки наружу:"; echo "$BADLINKS"; exit 1; fi
echo "sandbox OK (git archive, без .git): $SBOX"

rm -rf "$SESSION"; mkdir -p "$SESSION"

START=$(date +%s)
(
  cd "$SBOX" || exit 1
  exec env PI_CODING_AGENT_DIR="$PIHOME" \
    PI_CODING_AGENT_SESSION_DIR="$SESSION" \
    CARGO_TARGET_DIR="$CARGO_TARGET" \
    timeout --signal=TERM --kill-after=10s "$TIMEOUT" \
    "$PI" --provider neuraldeep --model qwen3.6-unlim-noreason --thinking off \
         -p "$(cat "$ROOT/workloads/$WL.md")" \
         --mode json --no-prompt-templates \
         --append-system-prompt "$MICRO"
) > "$RUN/run.jsonl" 2> "$RUN/run.stderr" &
PI_PID=$!
python3 "$ROOT/scripts/loop_guard.py" --jsonl "$RUN/run.jsonl" \
  --trigger "$RUN/loop_guard.json" --pid "$PI_PID" --kill-after 10 \
  >> "$RUN/run.stderr" 2>&1 &
GUARD_PID=$!
wait "$PI_PID"; RC=$?
kill "$GUARD_PID" 2>/dev/null
wait "$GUARD_PID" 2>/dev/null
if [ -f "$RUN/loop_guard.json" ]; then
  echo "LOOP-GUARD сработал: $(tr -d '\n' < "$RUN/loop_guard.json" | cut -c1-120)"
fi
WALL=$(( $(date +%s) - START ))
TIMED_OUT=false
if [ "$RC" -eq 124 ] || [ "$RC" -eq 137 ]; then TIMED_OUT=true; fi
echo "$WALL" > "$RUN/run.jsonl.wall"

if ! bash "$ROOT/scripts/integrity.sh" check; then
  echo "STOP: агент писал вне песочницы (ранняя integrity), $ARM/$WL/r$REP try$ATTEMPT"
  exit 2
fi

( cd "$SBOX" && CARGO_TARGET_DIR="$CARGO_TARGET" "${TEST_CMD[@]}" ) > "$RUN/tests.txt" 2>&1
echo "$?" > "$RUN/tests.txt.rc"
if [ "$WL" = "W2" ]; then
  ( cd "$SBOX" && CARGO_TARGET_DIR="$CARGO_TARGET" cargo test -p stellwerk-registry -- --list ) \
    > "$RUN/tests_list.txt" 2>&1
  echo "$?" > "$RUN/tests_list.txt.rc"
fi
python3 "$ROOT/scripts/dir_diff.py" --base "$SBOX_BASE" --new "$SBOX" --out "$RUN/diff.txt" >/dev/null
if [ "$WL" = "W1" ]; then
  python3 "$ROOT/scripts/verify_w1.py" --repo "$REPO" --base "$BASE" \
    --relpath crates/stellwerk-registry/src/model.rs \
    --file "$SBOX/crates/stellwerk-registry/src/model.rs" \
    --out "$RUN/w1_integrity.json" || echo "WARN: base_intact=false"
fi

python3 "$ROOT/scripts/extract.py" \
  --run "$RUN/run.jsonl" --session-dir "$SESSION" \
  --tests "$RUN/tests.txt" --diff "$RUN/diff.txt" \
  --w1-integrity "$RUN/w1_integrity.json" --stderr "$RUN/run.stderr" \
  --wall "$WALL" --timeout "$TIMED_OUT" --exit-code "$RC" \
  --arm "$ARM" --workload "$WL" --rep "$REP" --ts "$TS" \
  --attempt "$ATTEMPT" --retry-of "$RETRY_OF" \
  --net-path "$NET_PATH" --net-tun "$NET_TUN" --loop-guard "$RUN/loop_guard.json" \
  --out "$RUN/metrics.json" || { echo "STOP: extract.py упал"; exit 1; }

rm -rf "$CARGO_TARGET" "$SBOX" "$SBOX_BASE"

bash "$ROOT/scripts/integrity.sh" check || { echo "STOP: integrity не прошёл ПОСЛЕ"; exit 1; }

python3 - "$LEDGER" "$RUN/metrics.json" <<'PY'
import json, os, sys
ledger, mpath = sys.argv[1], sys.argv[2]
m = json.load(open(mpath))
cols = ('ts','arm','workload','rep','attempt','retry_of','valid','invalid_reason','rep_status',
        'net_path','net_tun','native_compactions','solpi_compactions',
        'total_input_cum','total_output_cum','cache_read_cum','tool_steps','turns',
        'peak_context','final_context','af_fired','op_fired','op_removed_tokens','ledger_files',
        'timeout','wall_seconds','stop_reason_last','tests_exit','tests_passed','tests_failed',
        'diff_files_changed','diff_insertions','diff_deletions','files_touched','usage_match',
        'activation_valid','equivalence_ok','solpi_extension_loaded','pi_exit_code')
if not os.path.exists(ledger):
    open(ledger, 'w').write('\t'.join(cols) + '\n')
m['rep_status'] = 'OK' if m.get('valid') else 'ATTEMPT_INVALID'
row = []
for c in cols:
    v = m.get(c, '')
    if isinstance(v, list):
        v = ','.join(str(x) for x in v)
    row.append(str(v).replace('\t', ' ').replace('\n', ' ')[:70])
with open(ledger, 'a') as fh:
    fh.write('\t'.join(row) + '\n')
print('ledger += %s/%s/r%s attempt=%s valid=%s' % (
    m.get('arm'), m.get('workload'), m.get('rep'), m.get('attempt'), m.get('valid')))
PY

echo "== done: $ARM/$WL/r$REP try$ATTEMPT (rc=$RC timeout=$TIMED_OUT wall=${WALL}s net=$NET_PATH)"
