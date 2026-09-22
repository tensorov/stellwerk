#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# derive.sh — читает .ablation/results/ledger.tsv, считает производные метрики
# и вердикты, печатает итог и перезаписывает секцию Derived в TABLE.md.
#
# Набор метрик/вердиктов — по финальной постановке (поправки P1-P5 + уточнения Фазы 3):
#   omo_stack_tax        = V2 - V0        (ordinary OMO stack, НЕ «чистый OMO tax»)
#   agents_diet          = V2 - V4
#   mcp_diet             = V4 - V5
#   hooks_diet           = V5 - V5h
#   commands_diet        = V5h - V5x
#   residual_core        = V5x - V0       (налог, оставшийся после максимальной диеты)
#   micro_residual       = V6 - V1        (стек микро-воркера внутри OMO против чистого+polyforge)
#   sisyphus_override_delta = V7 - V5x
#   clean_advantage      = V6 - V9        (OMO на микро-агенте против чистого микро runtime)
#   session_repeat       = V6b - V6
#   qwen_scale (V1/B)    = 9415 / V1      (polyforge включён в обоих)
#   qwen_scale (V0/A)    = 8602 / V0      (sanity)
#   V6_qwen_est          = V6 * (9415 / V1)
#   V7_qwen_est          = V7 * (9415 / V1)
#
# Константы qwen (из брифа, модель neuraldeep/qwen3.6-unlim-noreason):
#   A_qwen = 8602  (clean OpenCode), B_qwen = 9415  (A + polyforge MCP)

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

ledger="$RESULTS_ROOT/ledger.tsv"
md="$RESULTS_ROOT/TABLE.md"
[ -f "$ledger" ] || die "нет ledger.tsv — сначала record.sh"

# --- загрузка [VAR]=TOKENS ---
declare -A T=()
while IFS=$'\t' read -r var tok model note ts; do
  [ "$var" = "VAR" ] && continue
  [ -n "$var" ] || continue
  case "$tok" in ''|*[!0-9]*) continue ;; esac
  case "$var" in
    *-r[0-9]*) continue ;;   # повторы run-one: только агрегат используется
  esac
  T["$var"]="$tok"
done < "$ledger"

A_QWEN=8602
B_QWEN=9415

# val K — значение варианта или пусто.
val() { printf '%s' "${T[$1]:-}"; }
have() { [ -n "${T[$1]:-}" ]; }
# req V1 V2 ... — именованные значения для m_missing: печатает 'V1,V2,...' из тех, что нет.
req() { local r="" x; for x in "$@"; do [ -n "${T[$x]:-}" ] || r="$r$x,"; done; r="${r%,}"; printf '%s' "$r"; }

DIFF() { awk -v a="$1" -v b="$2" 'BEGIN{ printf "%d", a-b }'; }
FDIV() { awk -v a="$1" -v b="$2" 'BEGIN{ if (b==0) { printf "inf" } else { printf "%.3f", a/b } }'; }
FDIV1() { awk -v a="$1" -v b="$2" 'BEGIN{ if (b==0) { printf "inf" } else { printf "%.1f", a/b } }'; }
ABS()  { awk -v a="$1" -v b="$2" 'BEGIN{ d=a-b; if(d<0)d=-d; printf "%d", d }'; }
MULT() { awk -v a="$1" -v b="$2" 'BEGIN{ printf "%.0f", a*b }'; }

mkdir -p -- "$RESULTS_ROOT"
inner="$ABL_ROOT/derive-inner.tmp"
{
  echo '### Производные метрики'
  echo
  echo '| Метрика | Формула | Значение |'
  echo '|---:|---:|---:|'

  m_row() { printf '| %s | %s | %s |\n' "$1" "$2" "$3"; }
  m_missing() { printf '| %s | %s | MISSING: нужны %s |\n' "$1" "$2" "$3"; }

  # omo_stack_tax = V2 - V0
  if have V2 && have V0; then m_row omo_stack_tax 'V2 - V0' "$(DIFF "${T[V2]}" "${T[V0]}")"; else m_missing omo_stack_tax 'V2 - V0' "$(req V2 V0)"; fi
  # agents_diet = V2 - V4
  if have V2 && have V4; then m_row agents_diet 'V2 - V4' "$(DIFF "${T[V2]}" "${T[V4]}")"; else m_missing agents_diet 'V2 - V4' "$(req V2 V4)"; fi
  # mcp_diet = V4 - V5
  if have V4 && have V5; then m_row mcp_diet 'V4 - V5' "$(DIFF "${T[V4]}" "${T[V5]}")"; else m_missing mcp_diet 'V4 - V5' "$(req V4 V5)"; fi
  # hooks_diet = V5 - V5h
  if have V5 && have V5h; then m_row hooks_diet 'V5 - V5h' "$(DIFF "${T[V5]}" "${T[V5h]}")"; else m_missing hooks_diet 'V5 - V5h' "$(req V5 V5h)"; fi
  # commands_diet = V5h - V5x
  if have V5h && have V5x; then m_row commands_diet 'V5h - V5x' "$(DIFF "${T[V5h]}" "${T[V5x]}")"; else m_missing commands_diet 'V5h - V5x' "$(req V5h V5x)"; fi
  # residual_core = V5x - V0
  if have V5x && have V0; then m_row residual_core 'V5x - V0' "$(DIFF "${T[V5x]}" "${T[V0]}")"; else m_missing residual_core 'V5x - V0' "$(req V5x V0)"; fi
  # micro_residual = V6 - V1
  if have V6 && have V1; then m_row micro_residual 'V6 - V1' "$(DIFF "${T[V6]}" "${T[V1]}")"; else m_missing micro_residual 'V6 - V1' "$(req V6 V1)"; fi
  # sisyphus_override_delta = V7 - V5x
  if have V7 && have V5x; then m_row sisyphus_override_delta 'V7 - V5x' "$(DIFF "${T[V7]}" "${T[V5x]}")"; else m_missing sisyphus_override_delta 'V7 - V5x' "$(req V7 V5x)"; fi
  # clean_advantage = V6 - V9
  if have V6 && have V9; then m_row clean_advantage 'V6 - V9' "$(DIFF "${T[V6]}" "${T[V9]}")"; else m_missing clean_advantage 'V6 - V9' "$(req V6 V9)"; fi
  # session_repeat = V6b - V6
  if have V6b && have V6; then m_row session_repeat 'V6b - V6' "$(DIFF "${T[V6b]}" "${T[V6]}")"; else m_missing session_repeat 'V6b - V6' "$(req V6b V6)"; fi

  # qwen_scale — коэффициент по V1/B (polyforge включён в обоих) и sanity V0/A
  if have V1 && have V0; then
    qsB="$(FDIV "$B_QWEN" "${T[V1]}")"
    qsA="$(FDIV "$A_QWEN" "${T[V0]}")"
    m_row 'qwen_scale (V1/B)' "B_qwen / V1 = 9415 / ${T[V1]}" "$qsB"
    m_row 'qwen_scale (V0/A, sanity)' "A_qwen / V0 = 8602 / ${T[V0]}" "$qsA"
  else
    if ! have V1 || ! have V0; then m_missing 'qwen_scale (V1/B)' 'B_qwen / V1' "$(req V1)"; m_missing 'qwen_scale (V0/A)' 'A_qwen / V0' "$(req V0)"; fi
  fi

  # V6_qwen_estimate, V7_qwen_estimate
  if have V6 && have V1; then
    qsB="$(FDIV "$B_QWEN" "${T[V1]}")"
    m_row V6_qwen_estimate "V6 * (B_qwen / V1) = ${T[V6]} * $qsB" "$(MULT "${T[V6]}" "$qsB")"
  else
    m_missing V6_qwen_estimate 'V6 * (B_qwen / V1)' "$(req V6 V1)"
  fi
  if have V7 && have V1; then
    qsB="$(FDIV "$B_QWEN" "${T[V1]}")"
    m_row V7_qwen_estimate "V7 * (B_qwen / V1) = ${T[V7]} * $qsB" "$(MULT "${T[V7]}" "$qsB")"
  else
    m_missing V7_qwen_estimate 'V7 * (B_qwen / V1)' "$(req V7 V1)"
  fi

  echo
  echo '### Вердикты по гипотезам'
  echo
  echo '| Гипотеза | Критерий | Значение | Вердикт |'
  echo '|---|---:|---:|---|'

  h_row() { printf '| %s | %s | %s | **%s** |\n' "$1" "$2" "$3" "$4"; }
  h_missing() { printf '| %s | %s | MISSING: нужны %s | **N/A** |\n' "$1" "$2" "$3"; }

  # H1 core-dominates: residual_core / (V2 - V0) >= 0.50
  if have V5x && have V2 && have V0; then
    num="$(DIFF "${T[V5x]}" "${T[V0]}")"; den="$(DIFF "${T[V2]}" "${T[V0]}")"
    v="$(FDIV1 "$num" "$den")"
    if [ "$den" -gt 0 ] && [ "$(awk -v a="$num" -v b="$den" 'BEGIN{ print (a/b>=0.50)?"PASS":"FAIL" }')" = PASS ]; then
      h_row H1 'residual_core/(V2-V0) >= 0.50' "$v" PASS
    elif [ "$den" -gt 0 ]; then
      h_row H1 'residual_core/(V2-V0) >= 0.50' "$v" FAIL
    else
      h_row H1 'residual_core/(V2-V0) >= 0.50' "den=$den (<=0)" INVALID
    fi
  else
    h_missing H1 'residual_core/(V2-V0) >= 0.50' "$(req V5x V2 V0)"
  fi
  # H2 agents-help: agents_diet >= 10000
  if have V2 && have V4; then d="$(DIFF "${T[V2]}" "${T[V4]}")"; [ "$d" -ge 10000 ] && h_row H2 'agents_diet >= 10000' "$d" PASS || h_row H2 'agents_diet >= 10000' "$d" FAIL; else h_missing H2 'agents_diet >= 10000' "$(req V2 V4)"; fi
  # H3 mcp-help: mcp_diet >= 5000
  if have V4 && have V5; then d="$(DIFF "${T[V4]}" "${T[V5]}")"; [ "$d" -ge 5000 ] && h_row H3 'mcp_diet >= 5000' "$d" PASS || h_row H3 'mcp_diet >= 5000' "$d" FAIL; else h_missing H3 'mcp_diet >= 5000' "$(req V4 V5)"; fi
  # H9 hooks-dominate: hooks_diet >= 0.5*(V2 - V5)
  if have V5 && have V5h && have V2; then
    hd=$(DIFF "${T[V5]}" "${T[V5h]}"); half=$(awk -v a="$(DIFF "${T[V2]}" "${T[V5]}")" 'BEGIN{ printf "%.1f", a*0.5 }')
    if [ "$(awk -v a="$hd" -v b="$half" 'BEGIN{ print (a>=b)?"PASS":"FAIL" }')" = PASS ]; then
      h_row H9 "hooks_diet >= 0.5*(V2-V5) = $half" "$hd" PASS
    else
      h_row H9 "hooks_diet >= 0.5*(V2-V5) = $half" "$hd" FAIL
    fi
  else
    h_missing H9 'hooks_diet >= 0.5*(V2-V5)' "$(req V5 V5h V2)"
  fi
  # H4 micro-in-omo-fat: V6 >= 40000
  if have V6; then [ "${T[V6]}" -ge 40000 ] && h_row H4 'V6 >= 40000' "${T[V6]}" PASS || h_row H4 'V6 >= 40000' "${T[V6]}" FAIL; else h_missing H4 'V6 >= 40000' "$(req V6)"; fi
  # H5 override-useless: |V7 - V6| < 2000 (значимой разницы между thin-sisyphus и кастомным агентом нет)
  if have V7 && have V6; then
    ab="$(ABS "${T[V7]}" "${T[V6]}")"
    [ "$ab" -lt 2000 ] && h_row H5 '|V7 - V6| < 2000' "$ab" PASS || h_row H5 '|V7 - V6| < 2000' "$ab" FAIL
  else h_missing H5 '|V7 - V6| < 2000' "$(req V7 V6)"; fi
  # H6 per-session-tax: |V6b - V6| < 2000
  if have V6b && have V6; then
    ab="$(ABS "${T[V6b]}" "${T[V6]}")"
    [ "$ab" -lt 2000 ] && h_row H6 '|V6b - V6| < 2000' "$ab" PASS || h_row H6 '|V6b - V6| < 2000' "$ab" FAIL
  else h_missing H6 '|V6b - V6| < 2000' "$(req V6b V6)"; fi
  # H7 clean-only: V9 < 12000 AND V6 > 30000
  if have V9 && have V6; then
    if [ "${T[V9]}" -lt 12000 ] && [ "${T[V6]}" -gt 30000 ]; then
      h_row H7 'V9 < 12000 AND V6 > 30000' "V9=${T[V9]}, V6=${T[V6]}" PASS
    else
      h_row H7 'V9 < 12000 AND V6 > 30000' "V9=${T[V9]}, V6=${T[V6]}" FAIL
    fi
  else h_missing H7 'V9 < 12000 AND V6 > 30000' "$(req V9 V6)"; fi

  echo
  echo '### Решение (по V6/V7_qwen_est и residual_core)'
  echo
  echo 'Правило (Фаза 3): PASS<=18000, FAIL>30000. Флаги:'
  echo '  PASS_OMO_CUSTOM_WORKER: V6_qwen_est <= 18000'
  echo '  PASS_OMO_SISYPHUS:      V7_qwen_est <= 18000'
  echo '  FAIL_OMO_WORKER:        min(V6_qwen_est, V7_qwen_est) > 30000'
  echo '  GREY — иначе.'
  echo
  if have V6 && have V1; then
    qsB="$(FDIV "$B_QWEN" "${T[V1]}")"
    v6="$(MULT "${T[V6]}" "$qsB")"
    v7=""
    if have V7; then v7="$(MULT "${T[V7]}" "$qsB")"; fi
    p6=""; p7=""; fl=""
    [ -n "$v6" ] && { [ "$v6" -le 18000 ] && p6=PASS_OMO_CUSTOM_WORKER || p6=n/a; }
    [ -n "$v7" ] && { [ "$v7" -le 18000 ] && p7=PASS_OMO_SISYPHUS || p7=n/a; }
    if [ -n "$v7" ]; then
      mn="$([ "$v6" -le "$v7" ] && echo "$v6" || echo "$v7")"
    else
      mn="$v6"
    fi
    if [ "$mn" -gt 30000 ]; then
      fl=FAIL_OMO_WORKER
    elif [ "$p6" = PASS_OMO_CUSTOM_WORKER ] || [ "$p7" = PASS_OMO_SISYPHUS ]; then
      fl="PASS ($p6 / $p7)"
    else
      fl=GREY
    fi
    echo "**$fl** — min(V6_qwen_est=$v6, V7_qwen_est=${v7:-n/a}) = $mn; флаги: custom-worker=$p6, sisyphus=$p7."
    if have V5x && have V2 && have V0; then
      rc="$(DIFF "${T[V5x]}" "${T[V0]}")"; st="$(DIFF "${T[V2]}" "${T[V0]}")"
      if [ "$st" -gt 0 ] && [ "$rc" -ge 0 ] && [ "$(awk -v a="$rc" -v b="$st" 'BEGIN{ print (a/b>=0.50)?"1":"0" }')" = 1 ]; then
        echo "Сопоставление: residual_core/(V2-V0) = $rc/$st: ядро (плагин/prompt/compaction) доминирует — тонкий Sisyphus даёт то же."
      else
        echo "Сопоставление: residual_core/(V2-V0) = $rc/$st (H1 FAIL/INVALID): диеты уносят большую часть налога."
      fi
    else
      echo "Сопоставление: residual_core или omo_stack_tax неизвестны — доминацию ядра определить нельзя."
    fi
  else
    echo '**N/A** — для решения нужны V6 и V1.'
  fi

  echo
  echo '### Неизмерено / неопределённо'
  echo
  echo '- grep_app недоступен по сети на момент старых замеров — стоимость НЕ измерена (в базу не попадает).'
  echo '- disabled_skills/disabled_tools — открытые массивы (без enum) в схеме OMO 4.19.4 -> NOT_APPLICABLE (см. PROBE.md).'
  echo '- Если qwen_scale по V0 (A) и по V1 (B) расходятся >5%, экстраполяция ненадёжна — нужен прямой замер на qwen.'
  echo '- CLI и scheduled-commands диеты реализованы только через disabled_commands (6 имён); остальное — ручная диета.'
  echo
  printf 'Производные рассчитаны: %s\n' "$(stamp)"
} > "$inner"

# --- пересборка TABLE.md ---
render_table "$ledger" "$md" "$(cat "$inner")"

# --- stdout: краткий итог ---
echo '=== derive: итог ==='
echo
cat "$inner"

# финальный вывод решения отдельно
if have V6 && have V1; then
  qsB="$(FDIV "$B_QWEN" "${T[V1]}")"
  v6="$(MULT "${T[V6]}" "$qsB")"
  if have V7; then v7="$(MULT "${T[V7]}" "$qsB")"; else v7=""; fi
  mn="$([ -n "$v7" ] && { [ "$v6" -le "$v7" ] && echo "$v6" || echo "$v7"; } || echo "$v6")"
  p6=""; p7=""; DEC=""
  [ "$v6" -le 18000 ] && p6=PASS_OMO_CUSTOM_WORKER || p6=no
  [ -n "$v7" ] && { [ "$v7" -le 18000 ] && p7=PASS_OMO_SISYPHUS || p7=no; }
  if [ "$mn" -gt 30000 ]; then DEC=FAIL_OMO_WORKER; elif [ "$p6" = PASS_OMO_CUSTOM_WORKER ] || [ "$p7" = PASS_OMO_SISYPHUS ]; then DEC=PASS; else DEC=GREY; fi
  echo
  printf 'DECISION=%s (min_tokens=%s; V6_est=%s, V7_est=%s)\n' "$DEC" "$mn" "$v6" "${v7:-n/a}"
  printf 'FLAGS: PASS_OMO_CUSTOM_WORKER=%s, PASS_OMO_SISYPHUS=%s\n' "$p6" "$p7"
else
  printf 'DECISION=NA (нужны V6 и V1)\n'
fi
rm -f -- "$inner"