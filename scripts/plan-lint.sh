#!/usr/bin/env bash
# Проверяет .sisyphus/plans/*.md на утечку скоупа и формальные требования M0.
set -euo pipefail
shopt -s nullglob
plans=(.sisyphus/plans/*.md)
[[ ${#plans[@]} -gt 0 ]] || { echo "нет планов в .sisyphus/plans/"; exit 1; }
fail=0
for p in "${plans[@]}"; do
  echo "== $p"
  if grep -niE '\b(kanban|hermes|gateway|nag|crm|obsidian|sync|nats|casbin|authentik)\b' "$p" \
     | grep -viE 'deferred|appendix|не планир|запрещ'; then
    echo "  ✗ утечка скоупа (см. строки выше)"; fail=1
  fi
  n=$(grep -ciE '^#+ *(packet|пакет)' "$p" || true)
  echo "  packets: $n"; (( n <= 12 )) || { echo "  ✗ > 12 packets"; fail=1; }
  grep -qiE 'cargo test' "$p"      || { echo "  ✗ нет команд cargo test"; fail=1; }
  grep -qiE 'deferred' "$p"        || { echo "  ✗ нет раздела DEFERRED"; fail=1; }
  grep -qiE 'калибров|calibration' "$p" || { echo "  ✗ нет packet калибровки"; fail=1; }
  ! grep -qiE 'tokenizers?\b.*(hf|hugging)' "$p" || { echo "  ✗ токенизатор в плане"; fail=1; }
  awk '/cargo add/ {print "  ✗ cargo add: " $0; exit 1}' "$p" || fail=1
done
(( fail == 0 )) && echo "OK: план прошёл lint" || { echo "FAIL"; exit 1; }
