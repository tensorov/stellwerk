#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# snapshot.sh <tag> — снять снапшот всех используемых конфигов/каталогов в
# .ablation/snapshots/<ts>_<tag>/, записать MANIFEST (TSV: kind<TAB>abs<TAB>copy),
# обновить .ablation/snapshots/LATEST.
#
# Только чтение источников; пишет только в .ablation/snapshots/. Идемпотентен.

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

[ $# -eq 1 ] || die "usage: snapshot.sh <tag>"

tag="$(sanitize_tag "$1")"
[ -n "$tag" ] || die "тег пуст после санитизации"

ts="$(stamp)"
snap="$SNAP_ROOT/${ts}_${tag}"
manifest="$snap/MANIFEST"
mkdir -p -- "$SNAP_ROOT" "$snap"

: > "$manifest.tmp"
n=0
while IFS=$'\t' read -r kind path excl; do
  [ -n "$path" ] || continue
  if [ ! -e "$path" ]; then
    log "skip (не существует): $path"
    continue
  fi
  name="$(encode_path "$path")"
  case "$kind" in
    file)
      cp -a -- "$path" "$snap/$name"
      ;;
    dir)
      copy_dir "$path" "$snap/$name" "$excl"
      ;;
    *)
      die "неизвестный kind='$kind' для $path"
      ;;
  esac
  printf '%s\t%s\t%s\t%s\n' "$kind" "$path" "$name" "$excl" >> "$manifest.tmp"
  n=$((n + 1))
done < <(ablation_sources)

mv -- "$manifest.tmp" "$manifest"
printf '%s\n' "${ts}_${tag}" > "$LATEST_FILE"

log "snapshot: $snap"
log "MANIFEST записей: $n"
printf '%s\n' "$snap"