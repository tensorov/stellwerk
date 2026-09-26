#!/usr/bin/env bash
# run-all.sh — Phase C. Матрица 2×2×3 = 12 прогонов, строго последовательно.
# Фаза B его НЕ запускает: только собирает. Запуск — после отдельного «ок».
set -uo pipefail
umask 077

REPO="${STELLWERK_REPO:-/home/izyabretatel/gits/stellwerk}"
ROOT="$REPO/.ablation-solpi"
TS="${1:?ts}"
export LEDGER="${LEDGER:-$ROOT/ledger.tsv}"

order=( "baseline W1" "solpi W1" "baseline W2" "solpi W2" )
REPS="${REPS:-3}"

echo "== integrity baseline =="
if [ -f "$ROOT/runs/.integrity/baseline.txt" ]; then
  echo "используем существующий эталон (не пересоздаём — иначе замаскируем дрейф)"
else
  bash "$ROOT/scripts/integrity.sh" save || exit 1
fi
bash "$ROOT/scripts/integrity.sh" secrets || exit 1

for combo in "${order[@]}"; do
  set -- $combo; ARM="$1"; WL="$2"
  for rep in $(seq 1 "$REPS"); do
    echo "---- $ARM/$WL/r$rep ----"
    if ! bash "$ROOT/scripts/run-one.sh" "$ARM" "$WL" "$rep" "$TS"; then
      echo "STOP: прогон $ARM/$WL/r$rep провалился; матрица прервана."
      exit 1
    fi
  done
done

echo "== activation gate =="
python3 - "$LEDGER" <<'PY2'
import sys
rows = [l.rstrip('\n').split('\t') for l in open(sys.argv[1])]
hdr = rows[0]
ai, oi, armi, wli = hdr.index('af_fired'), hdr.index('op_fired'), hdr.index('arm'), hdr.index('workload')
solpi = [(r[ai], r[oi], r[wli]) for r in rows[1:] if r[armi] == 'solpi']
bad = [(a, o, w) for a, o, w in solpi
       if not (int(a) > 0 if w == 'W1' else (int(a) > 0 or int(o) > 0))]
if not solpi:
    print('STOP: в ledger нет ни одного solpi-прогона.'); sys.exit(3)
if bad:
    print('STOP: solpi-прогоны без активации по правилу arm:', bad)
    print('      Диагностика: pi list в solpi-home, run.stderr (solpi_extension_errors), session dir.')
    print('      Отчёт с ложным выводом не пишем.')
    sys.exit(3)
print('activation gate OK: solpi (af, op, workload) =', solpi)
PY2
rc=$?
echo "== matrix finished (rc=$rc), ledger: $LEDGER =="
exit $rc
