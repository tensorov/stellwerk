#!/usr/bin/env bash
# retry-matrix.sh — Фаза C по политике владельца от 2026-09-26.
#
#   максимум 2 retry на rep (всего ≤3 попытки);
#   invalid-прогоны НЕ удаляются из ledger: valid=false, retry_of=<attempt>;
#   пауза 45 с между ЛЮБЫМИ прогонами (бесплатный эндпойнт, длинный хвост latency);
#   rep, невалидный после 3 попыток → rep_status=FAILED, матрица продолжается;
#   FAILED reps > 2 из 12 → СТОП с отчётом.
#   ВАЛИДНОСТЬ определяет гейт по метрикам JSONL, а НЕ код выхода Pi.
set -uo pipefail
umask 077

REPO="${STELLWERK_REPO:-/home/izyabretatel/gits/stellwerk}"
ROOT="$REPO/.ablation-solpi"
TS="${1:?ts}"
MAX_RETRY=2
MAX_ATTEMPTS=$((MAX_RETRY + 1))
PAUSE=45
FAILED_LIMIT=2
export LEDGER="${LEDGER:-$ROOT/ledger.tsv}"

# CELLS="baseline:W1:3 solpi:W1:3 baseline:W2:2" — arm:workload:reps (по : )
REPS="${REPS:-2}"
CELLS="${CELLS:-baseline:W1:$REPS solpi:W1:$REPS baseline:W2:$REPS solpi:W2:$REPS}"
order=(); reps=()
for c in $CELLS; do
  arm="${c%%:*}"; rest="${c#*:}"; wl="${rest%%:*}"; rp="${rest##*:}"
  order+=("$arm $wl"); reps+=("${rp:-$REPS}")
done
TOTAL=0
for r in "${reps[@]}"; do TOTAL=$((TOTAL + r)); done

if [ -f "$ROOT/runs/.integrity/baseline.txt" ]; then
  echo "== используем существующий эталон integrity =="
else
  bash "$ROOT/scripts/integrity.sh" save || exit 1
fi
bash "$ROOT/scripts/integrity.sh" secrets || exit 1

is_valid() {  # $1 = metrics.json
  python3 -c "
import json,sys
m=json.load(open(sys.argv[1]))
ok = bool(m.get('valid')) and m.get('stop_reason_last') != 'error' \
     and m.get('total_input_cum',0) > 0 and m.get('tool_steps',0) > 0 \
     and not (m.get('timeout') and m.get('turns',0) < 2)
sys.exit(0 if ok else 1)" "$1"
}

first=1
for ci in "${!order[@]}"; do
  set -- ${order[$ci]}; ARM="$1"; WL="$2"; N="${reps[$ci]}"
  for rep in $(seq 1 "$N"); do
    if [ "$first" -eq 0 ]; then sleep "$PAUSE"; fi
    first=0
    prev=""
    for attempt in $(seq 1 "$MAX_ATTEMPTS"); do
      echo "---- $ARM/$WL/r$rep attempt=$attempt/$MAX_ATTEMPTS ----"
      bash "$ROOT/scripts/run-one.sh" "$ARM" "$WL" "$rep" "$TS" "$attempt" "$prev"
      rc=$?
      METRICS="$ROOT/runs/$TS/$ARM/$WL/r$rep/metrics.json"
      [ "$attempt" -gt 1 ] && METRICS="$ROOT/runs/$TS/$ARM/$WL/r$rep/try$attempt/metrics.json"
      if [ "$rc" -eq 2 ]; then
        echo "STOP: ранняя integrity-поймала запись вне worktree ($ARM/$WL/r$rep)."
        exit 2
      fi
      if [ "$rc" -ne 0 ]; then
        echo "WARN: run-one вернул $rc; rep=$ARM/$WL/r$rep attempt=$attempt"
      fi
      if [ -f "$METRICS" ] && is_valid "$METRICS"; then
        echo "OK: $ARM/$WL/r$rep валиден на попытке $attempt"
        break
      fi
      reason=$( [ -f "$METRICS" ] && python3 -c "
import json,sys;print(json.load(open(sys.argv[1])).get('invalid_reason','?'))" "$METRICS" || echo no-metrics )
      echo "INVALID: attempt=$attempt reason=$reason"
      prev="$attempt"
      if [ "$attempt" -lt "$MAX_ATTEMPTS" ]; then sleep "$PAUSE"; fi
    done
    if [ -f "$METRICS" ] && ! is_valid "$METRICS"; then
      python3 - "$LEDGER" "$ARM" "$WL" "$rep" <<'PY'
import sys
ledger, arm, wl, rep = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
rows = [l.rstrip('\n').split('\t') for l in open(ledger)]
hdr = rows[0]; i = hdr.index('rep_status')
for r in rows[1:]:
    if r[hdr.index('arm')] == arm and r[hdr.index('workload')] == wl and r[hdr.index('rep')] == rep:
        r[i] = 'FAILED'
with open(ledger, 'w') as fh:
    fh.write('\n'.join('\t'.join(r) for r in rows) + '\n')
print('rep %s/%s/r%s помечен FAILED' % (arm, wl, rep))
PY
    fi
  done
done

echo "== СОГЛАСОВАННОСТЬ СЕТЕВОГО ПУТИ =="
python3 - "$LEDGER" <<'PY2'
import sys
rows=[l.rstrip('\n').split('\t') for l in open(sys.argv[1])]
hdr=rows[0]
pi=hdr.index('net_path'); ti=hdr.index('net_tun')
paths=sorted({r[pi] for r in rows[1:]}); tuns=sorted({r[ti] for r in rows[1:]})
print('net_path:', paths); print('net_tun :', tuns)
if len(paths)>1:
    print('ВНИМАНИЕ: смена net_path внутри матрицы — прогоны разных путей НЕ сравниваются.')
else:
    print('net_path одинаков во всех прогонах — сравнение корректно.')
PY2

echo "== итог: FAILED reps =="
python3 - "$LEDGER" "$FAILED_LIMIT" "$TOTAL" <<'PY'
import sys
rows=[l.rstrip('\n').split('\t') for l in open(sys.argv[1])]
hdr=rows[0]; i=hdr.index('rep_status')
failed=sorted({(r[hdr.index('arm')],r[hdr.index('workload')],r[hdr.index('rep')]) for r in rows[1:] if r[i]=='FAILED'})
print('FAILED reps (%d):'%len(failed), failed)
if len(failed) > int(sys.argv[2]):
    print('STOP: FAILED reps > %d из %d — эндпойнт непригоден сегодня.' % (int(sys.argv[2]), int(sys.argv[3])))
    sys.exit(3)
print('лимит FAILED не превышен')
PY
rc=$?
echo "== matrix finished (rc=$rc), ledger: $LEDGER =="
exit $rc
