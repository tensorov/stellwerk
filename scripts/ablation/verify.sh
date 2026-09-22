#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# verify.sh [dir] — после restore: diff текущих конфигов против снапшота.
# Ненулевой exit при любом расхождении. Также проверяет, что не осталось
# *.ablation_bak_* (stale-состояние). По умолчанию снапшот из LATEST.

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

snap="$(resolve_snapshot "${1:-}")"
manifest="$snap/MANIFEST"
[ -f "$manifest" ] || die "MANIFEST не найден: $manifest"

fail=0
n=0
while IFS=$'\t' read -r kind path name _excl; do
  [ -n "$path" ] || continue
  [ "$kind" = file ] || [ "$kind" = dir ] || continue
  src="$snap/$name"
  base="${path%/}"
  n=$((n + 1))

  # stale-артефакты переименования
  shopt -s nullglob
  stale=("$base".ablation_bak_*)
  shopt -u nullglob
  if [ "${#stale[@]}" -gt 0 ]; then
    printf 'STALE-BAK %s -> %d артефакт(ов): %s\n' "$base" "${#stale[@]}" "${stale[*]}" >&2
    fail=1
  fi

  case "$kind" in
    file)
      if [ ! -e "$path" ]; then
        printf 'MISSING-FILE %s\n' "$path" >&2; fail=1
      elif ! cmp -s -- "$src" "$path"; then
        printf 'MISMATCH-FILE %s\n' "$path" >&2; fail=1
      fi
      ;;
    dir)
      if [ ! -d "$path" ]; then
        printf 'MISSING-DIR %s\n' "$path" >&2; fail=1
      elif [ -n "$_excl" ] && ! diff -rq --exclude="$_excl" "$src" "$path" >/dev/null 2>&1; then
        printf 'MISMATCH-DIR %s\n' "$path" >&2; fail=1
      elif [ -z "$_excl" ] && ! diff -rq "$src" "$path" >/dev/null 2>&1; then
        printf 'MISMATCH-DIR %s\n' "$path" >&2; fail=1
      fi
      ;;
  esac
done < <(grep -vE '^#|^$' "$manifest")

if [ "$fail" -ne 0 ]; then
  printf 'verify FAIL: %s (%d записей)\n' "$snap" "$n" >&2
  exit 1
fi
printf 'verify OK: %s (%d записей)\n' "$snap" "$n"