#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# restore.sh [dir] — восстановить конфиги строго по MANIFEST снапшота.
# По умолчанию — из LATEST. Порядок:
#   1) раз-rename всех *.ablation_bak_* (файл: <abs>.ablation_bak_<V>; каталог: <abs>.ablation_bak_<V>);
#   2) копирование содержимого снапшота на место (файлы — cp, каталоги — tar-merge).
# Директории: node_modules (если исключались) остаются нетронутыми в неноф-дереве.

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

snap="$(resolve_snapshot "${1:-}")"
manifest="$snap/MANIFEST"
[ -f "$manifest" ] || die "MANIFEST не найден: $manifest"

log "restore: $snap"

# --- 1) раз-rename всех *.ablation_bak_* для путей из MANIFEST ---
while IFS=$'\t' read -r kind path name _excl; do
  [ -n "$path" ] || continue
  [ "$kind" = file ] || [ "$kind" = dir ] || continue
  base="${path%/}"
  shopt -s nullglob
  matches=("$base".ablation_bak_*)
  shopt -u nullglob
  for bak in "${matches[@]}"; do
    rm -rf -- "$base"
    log "un-rename: $bak -> $base"
    mv -- "$bak" "$base"
  done
done < <(grep -vE '^#|^$' "$manifest")

# --- 2) восстановление содержимого из снапшота ---
while IFS=$'\t' read -r kind path name excl; do
  [ -n "$path" ] || continue
  [ "$kind" = file ] || [ "$kind" = dir ] || continue
  src="$snap/$name"
  case "$kind" in
    file)
      mkdir -p -- "$(dirname -- "$path")"
      cp -a -- "$src" "$path"
      ;;
    dir)
      # Точное восстановление: удаляем лишнее в каталоге, сохраняя исключённое
      # снапшотом (например node_modules), затем слияние из снапшота.
      # Защита от случайного rm корневых каталогов.
      case "${path%/}" in
        "$ABL_HOME"|"$REPO_ROOT"|"$ABL_ROOT"|"$SNAP_ROOT"|"$RESULTS_ROOT")
          die "restore: отказ удалять корневой каталог: $path" ;;
      esac
      mkdir -p -- "$path"
      if [ -n "$excl" ]; then
        find "$path" -mindepth 1 -maxdepth 1 ! -name "$excl" -exec rm -rf -- {} + 2>/dev/null || true
      else
        find "$path" -mindepth 1 -maxdepth 1 -exec rm -rf -- {} + 2>/dev/null || true
      fi
      copy_dir "$src" "$path" "$excl"
      ;;
  esac
  log "restored: $path"
done < <(grep -vE '^#|^$' "$manifest")

log "restore завершён: $snap"