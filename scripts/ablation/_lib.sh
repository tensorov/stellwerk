#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# Общая библиотека для scripts/ablation.
# Зависимости: bash, coreutils, tar, jq (jq — только для gen-configs.sh).
# Константы старых замеров и логика метрик — в derive.sh, тут только общие утилиты.

set -euo pipefail

# Каталог с самими скриптами (для вызова gen-configs.sh и чтения variants/).
ABL_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

# Переносимость (используется QA-прогоном в песочнице; в бою не задаются):
#   ABL_HOME_OVERRIDE — вместо $HOME (снапшоты/варианты вне-репо конфигов)
#   ABL_REPO_OVERRIDE — вместо пути репозитория
if [ -n "${ABL_HOME_OVERRIDE:-}" ]; then
  ABL_HOME="$ABL_HOME_OVERRIDE"
else
  ABL_HOME="$HOME"
fi

# Корень репозитория: scripts/ablation/_lib.sh -> ../..
if [ -n "${ABL_REPO_OVERRIDE:-}" ]; then
  REPO_ROOT="$ABL_REPO_OVERRIDE"
else
  REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
fi
ABL_ROOT="$REPO_ROOT/.ablation"
SNAP_ROOT="$ABL_ROOT/snapshots"
RESULTS_ROOT="$ABL_ROOT/results"
VARIANTS_DIR="$ABL_ROOT/variants"
XDIR_ROOT="$ABL_ROOT/xdg"
LATEST_FILE="$SNAP_ROOT/LATEST"

log()  { printf '[ablation] %s\n' "$*" >&2; }
die()  { printf '[ablation] ERROR: %s\n' "$*" >&2; exit 1; }

# Санитизация тега: допустимы [A-Za-z0-9._-], прочее -> '_'
sanitize_tag() {
  printf '%s' "${1:?tag required}" | tr -c 'A-Za-z0-9._-' '_'
}

# Метка времени UTC: YYYYmmddTHHMMSSZ
stamp() { date -u +%Y%m%dT%H%M%SZ; }

# Кодирование абсолютного пути в уникальное имя копии (без '/')
# Пример: /home/u/.omo/omo.jsonc -> home__u__.omo__omo.jsonc
encode_path() {
  local p="$1"
  p="${p%/}"
  printf '%s' "${p#/}" | sed 's#/#__#g'
}

# Одноуровневое копирование дерева (без chown/chgrp), с опциональным исключением.
# arg: copy_dir SRC DST [EXCLUDE_GLOB]
copy_dir() {
  local src="$1" dst="$2" excl="${3:-}"
  mkdir -p -- "$dst"
  if [ -n "$excl" ]; then
    tar --exclude="$excl" -C "$src" -cf - . | tar -C "$dst" -xf -
  else
    tar -C "$src" -cf - . | tar -C "$dst" -xf -
  fi
}

# Источники для снапшотов (P1 + PROBE). Строка: kind<tab>path[<tab>exclude]
# kind: file | dir. Для dir третий столбец — glob исключения при копировании.
ablation_sources() {
  local home proj
  home="$ABL_HOME"
  proj="${PROJECT_DIR:-$REPO_ROOT}"
  printf 'file\t%s/.omo/omo.jsonc\n' "$home"                  # OMO-конфиг (вне репо; *.bak.* НЕ трогаем)
  printf 'file\t%s/.config/opencode/opencode.json\n' "$home"  # глобальный OpenCode (вне репо)
  printf 'file\t%s/opencode.json\n' "$proj"                   # проектный OpenCode (plugin + polyforge)
  printf 'dir\t%s/.opencode\tnode_modules\n' "$proj"          # проектный каталог .opencode (node_modules не копируем)
  if [ -f "$proj/.omo/omo.jsonc" ]; then
    printf 'dir\t%s/.omo\n' "$proj"                           # проектный .omo — только если появится omo.jsonc (P1)
  fi
  if [ -d "$home/.config/opencode/agents" ]; then
    printf 'dir\t%s/.config/opencode/agents\n' "$home"        # глобальные markdown-агенты, если появятся
  fi
}

# Разрешение цели снапшота: каталог или имя (ts_tag) -> абсолютный путь под SNAP_ROOT.
resolve_snapshot() {
  local target="${1:-}"
  if [ -z "$target" ]; then
    [ -f "$LATEST_FILE" ] || die "LATEST отсутствует; укажите каталог снапшота"
    target="$(cat "$LATEST_FILE")"
  fi
  local snap
  case "$target" in
    /*) snap="$target" ;;
    *)  snap="$SNAP_ROOT/$target" ;;
  esac
  case "$snap" in
    "$SNAP_ROOT"/*) : ;;
    *) die "снапшот вне SNAP_ROOT: $snap" ;;
  esac
  printf '%s\n' "$snap"
}

# Широкий заголовок V-таблицы. Аргумент: полный путь к ledger.tsv на месте.
table_header() {
  printf '# Ablation — V-таблица\n'
  printf '\n'
  printf 'Харнесс: opencode %s, OMO oh-my-openagent@4.19.4, polyforge-mcp v0.4.0.\n' "$(opencode --version 2>/dev/null || echo '?')"
  printf 'Все замеры: промпт "." на ОДНОЙ модели (фиксируется в каждой строке), счётчик input провайдера.\n'
  printf 'V-варианты по умолчанию на big-pickle (256k); qwen-строки помечаются моделью явно.\n'
  printf 'Логика: V0/V1 = повтор A/B (переводной коэффициент); V2 = ordinary OMO stack (OMO + текущий MCP поверх),\n'
  printf '  ближе к старому D/E, НЕ к C. Диеты: V4 agents, V5 mcps, V5h hooks(56), V5x commands(6).\n'
  printf 'Решения — по V6/V7_qwen_est и residual_core, см. derive.sh.\n'
  printf '\n## Measured\n'
  printf '| VAR | Tokens | Model | Note | TS |\n'
  printf '|---|---:|---|---|---|\n'
}

# Все строки таблицы из ledger (без шапки логгера).
table_rows() {
  local ledger="$1"
  tail -n +2 "$ledger" | while IFS=$'\t' read -r var tok model note ts; do
    printf '| %s | %s | %s | %s | %s |\n' \
      "${var:-?}" "${tok:-?}" "${model:-?}" "${note:--}" "${ts:-?}"
  done
}

# Текущее содержимое секции Derived между маркерами (пусто, если маркеров нет).
derived_inner_current() {
  local md="$1"
  if [ -f "$md" ]; then
    sed -n '/^<!-- DERIVED:BEGIN -->$/,/^<!-- DERIVED:END -->$/p' "$md" \
      | sed '1d;$d'
  fi
}

# Сборка TABLE.md: шапка + строки из ledger + секция Derived с given-внутренностью.
# args: LEDGER MD DERIVED_INNER
render_table() {
  local ledger="$1" md="$2" inner="$3"
  {
    table_header "$ledger"
    table_rows "$ledger"
    printf '\n## Derived\n'
    printf '<!-- DERIVED:BEGIN -->\n'
    [ -z "$inner" ] || printf '%s\n' "$inner"
    printf '<!-- DERIVED:END -->\n'
  } > "$md.tmp"
  mv -- "$md.tmp" "$md"
}

# ---------------------------------------------------------------------------
# Хелперы для apply-<VAR>.sh
# ---------------------------------------------------------------------------

# Релей-пути проектных конфигов, которые переименовываются на время
# no-OMO вариантов (V0/V1/V9). OMO-варианты (V2/V4/V5/V5h/V5x/V6/V7) требуют,
# чтобы они были на месте.
PROJECT_PREPEND_ITEMS=("opencode.json" ".opencode")

# ensure_renamed SUFFIX — спрятать «настоящие» проектные конфиги под
# *.ablation_bak_<SUFFIX>. Если файл уже переименован другим суффиксом —
# переименовать его в новый (сценарий V0 -> V1 -> V9).
ensure_renamed() {
  local suffix="$1" rel base m bak
  for rel in "${PROJECT_PREPEND_ITEMS[@]}"; do
    base="$REPO_ROOT/$rel"
    if [ -e "$base" ]; then
      mv -- "$base" "$base.ablation_bak_$suffix"
      log "renamed: $base -> $base.ablation_bak_$suffix"
      continue
    fi
    shopt -s nullglob
    m=("$base".ablation_bak_*)
    shopt -u nullglob
    case "${#m[@]}" in
      0)
        [ -e "$base.ablation_bak_$suffix" ] \
          || die "нет живого $base и нет bak для $suffix"
        ;;
      1)
        bak="${m[0]}"
        if [ "$bak" != "$base.ablation_bak_$suffix" ]; then
          mv -- "$bak" "$base.ablation_bak_$suffix"
          log "chained: $bak -> $base.ablation_bak_$suffix"
        fi
        ;;
      *)
        die "неоднозначные bak для $base: ${m[*]}"
        ;;
    esac
  done
}

# ensure_live_omo — вернуть проектные конфиги на место (для OMO-вариантов):
# любой *.ablation_bak_* -> живой. Падает, если оба состояния перепутаны.
ensure_live_omo() {
  local rel base m
  for rel in "${PROJECT_PREPEND_ITEMS[@]}"; do
    base="$REPO_ROOT/$rel"
    shopt -s nullglob
    m=("$base".ablation_bak_*)
    shopt -u nullglob
    if [ -e "$base" ]; then
      [ "${#m[@]}" -eq 0 ] || die "живой $base И bak одновременно: ${m[*]}"
      continue
    fi
    if [ "${#m[@]}" -eq 1 ]; then
      mv -- "${m[0]}" "$base"
      log "un-renamed: ${m[0]} -> $base"
    else
      die "нет живого $base и нет bak — OMO-варианту нужен живой конфиг: ${m[*]:-}"
    fi
  done
}

# install_global_omo — перегенерировать и установить единый MCP-вариант
# глобального конфига (P3: polyforge on, grep_app off, lsp off).
install_global_omo() {
  "$ABL_SCRIPT_DIR/gen-configs.sh"
  cp -- "$VARIANTS_DIR/global_opencode_omo.json" "$ABL_HOME/.config/opencode/opencode.json"
  log "global OpenCode -> OMO-MCP вариант (P3): $ABL_HOME/.config/opencode/opencode.json"
}

# install_omo_config FILE — цело-файловая замена ~/.omo/omo.jsonc.
install_omo_config() {
  local f="$1"
  [ -f "$f" ] || die "нет OMO-варианта: $f"
  cp -- "$f" "$ABL_HOME/.omo/omo.jsonc"
  log "OMO-конфиг: $ABL_HOME/.omo/omo.jsonc <- $f"
}

# print_guide AGENT VAR NOTE — финальная инструкция для Пользователя
# (ассистент opencode сам НЕ запускает; меряет только Пользователь).
print_guide() {
  local agent="$1" var="$2" note="${3:--}"
  cat <<EOF

== NEXT (делает Пользователь; ассистент opencode не запускается) ==
  opencode ~/gits/stellwerk
  (убедись: в строке состояния модель = big-pickle; активный агент = $agent)
  отправь ровно один символ: .
  скопируй число input-токенов и выполни:
  ./scripts/ablation/record.sh $var <tokens> big-pickle "$note"
EOF
}

# ---------------------------------------------------------------------------
# Фаза 3.5 — автоматизация run (opencode run --format json).
# ---------------------------------------------------------------------------

# Интерпретатор для extract-tokens.py и метрик, бинарь opencode. Переопределяемы
# для QA (фиксурный opencode) без изменения вызываемого кода.
PYTHON3="${PYTHON3:-python3}"
OPENCODE_BIN="${OPENCODE_BIN:-opencode}"

# ~/.ablation/current-run.env — какой вариант сейчас применён + агент + XDG.
CURRENT_RUN_ENV="$ABL_ROOT/current-run.env"

# emit_current_run_env VAR AGENT NOTE [XDG] — записать current-run.env.
# Значения пишутся через printf %q (shell-safe). Секретов тут нет по построению:
# XDG-путь сугубо внутренний, note — литеральное описание варианта.
emit_current_run_env() {
  local var="$1" agent="$2" note="$3" xdg="${4:-}"
  [ -n "$var" ] || die "emit_current_run_env: VAR пуст"
  mkdir -p -- "$ABL_ROOT"
  {
    printf 'ABLATION_VAR=%s\n' "$(printf '%q' "$var")"
    printf 'ABLATION_AGENT=%s\n' "$(printf '%q' "$agent")"
    printf 'ABLATION_NOTE=%s\n' "$(printf '%q' "$note")"
    printf 'ABLATION_XDG_CONFIG_HOME=%s\n' "$(printf '%q' "$xdg")"
  } > "$CURRENT_RUN_ENV"
  chmod 600 -- "$CURRENT_RUN_ENV"
  log "current-run.env: VAR=$var AGENT=$agent XDG='$xdg'"
}

# read_current_run_env — заполняет CUR_VAR/CUR_AGENT/CUR_NOTE/CUR_XDG.
read_current_run_env() {
  CUR_VAR=""; CUR_AGENT=""; CUR_NOTE=""; CUR_XDG=""
  if [ -f "$CURRENT_RUN_ENV" ]; then
    local abvar abagent abnote abxdg
    abvar=""; abagent=""; abnote=""; abxdg=""
    source "$CURRENT_RUN_ENV"
    CUR_VAR="${ABLATION_VAR:-}"
    CUR_AGENT="${ABLATION_AGENT:-}"
    CUR_NOTE="${ABLATION_NOTE:-}"
    CUR_XDG="${ABLATION_XDG_CONFIG_HOME:-}"
  fi
}

# median_of NUM... — медиана (sort-driven).
median_of() {
  local sorted n mid v i=0
  sorted="$(printf '%s\n' "$@" | sort -n)"
  n="$(printf '%s\n' "$sorted" | sed '/^$/d' | wc -l)"
  [ "$n" -gt 0 ] || { echo 0; return 1; }
  mid=$(((n + 1) / 2))
  while IFS= read -r v; do
    [ -n "$v" ] || continue
    i=$((i + 1))
    if [ "$i" -eq "$mid" ]; then printf '%s\n' "$v"; return 0; fi
  done <<<"$sorted"
  return 1
}

# spread_pct NUM... — (max-min)/median*100, печатает число или 0.
spread_pct() {
  local med lo hi
  med="$(median_of "$@" 2>/dev/null)" || med=0
  [ "$med" != "${med%%.*}" ] || [ -n "$med" ] || med=0
  lo="$(printf '%s\n' "$@" | sort -n | sed '/^$/d' | head -1)"
  hi="$(printf '%s\n' "$@" | sort -n | sed '/^$/d' | tail -1)"
  [ -n "$lo" ] && [ -n "$hi" ] || { echo 0; return 0; }
  awk -v lo="$lo" -v hi="$hi" -v med="$med" \
    'BEGIN { if (med > 0) printf "%.4f", (hi - lo) / med * 100; else printf "0" }'
}

# stability_for N SPREAD — STABLE | UNSTABLE | INSUFFICIENT (порог 2%).
stability_for() {
  local n="$1" sp="$2"
  [ "$n" -ge 3 ] || { echo INSUFFICIENT; return 0; }
  awk -v sp="$sp" 'BEGIN { if (sp > 2.0) print "UNSTABLE"; else print "STABLE" }'
}