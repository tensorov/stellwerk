#!/usr/bin/env bash
# sandbox.sh — изоляция packet'а: git archive в /tmp, без .git, без симлинков наружу.
#
# ПРОИСХОЖДЕНИЕ: инцидент F1 ablation (2026-09-24). Воркер на packet-07a ушёл из
# worktree и писал 12 edit'ов по абсолютным путям в ~/gits/stellwerk, потому что
# worktree содержал .git, указывающий на хост-репо. Здесь песочница физически не
# содержит .git, а prompt-guard запрещает абсолютные пути репо.
#
# Функции:
#   sandbox_create <repo> <ref> <tag>            → echoes sandbox path
#   sandbox_path_guard <sandbox> <prompt...>      → 0 ok, 4 BLOCKED
#   sandbox_integrity_snapshot <repo> <out>       → снимок состояния репо
#   sandbox_integrity_check <repo> <snapshot>     → 0 ok, 5 BLOCKED (пишем вне песочницы)
#   sandbox_diff <base_copy> <sandbox> <out>      → numstat + unified diff
#   sandbox_export <sandbox> <target_branch>      → ЯВНЫЙ экспорт после приёмки
set -uo pipefail

sandbox_create() {
  local repo="$1" ref="$2" tag="$3" root="${SANDBOX_ROOT:-/tmp/stellwerk-sandbox}"
  local sb="$root/$tag"
  rm -rf "$sb" "$sb.base"
  mkdir -p "$sb" "$sb.base"
  git -C "$repo" archive "$ref" | tar -x -C "$sb" || return 1
  git -C "$repo" archive "$ref" | tar -x -C "$sb.base" || return 1
  if [ -e "$sb/.git" ]; then
    echo "BLOCKED: в песочнице есть .git" >&2
    return 4
  fi
  printf '%s' "$sb"
}

sandbox_path_guard() {
  local sandbox="$1"; shift
  local repo
  repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  if [ -L "$sandbox" ]; then
    echo "BLOCKED: песочница является симлинком" >&2
    return 4
  fi
  local bad
  bad="$(find "$sandbox" -type l 2>/dev/null | while read -r l; do
           t="$(readlink -f "$l" 2>/dev/null)"
           case "$t" in "$sandbox"/*) ;; *) printf '%s -> %s\n' "$l" "$t" ;; esac
         done)"
  if [ -n "$bad" ]; then
    echo "BLOCKED: симлинки уводят из песочницы:" >&2
    echo "$bad" >&2
    return 4
  fi
  local src
  for src in "$@"; do
    if [ -f "$src" ] && grep -qE "$repo|/home/[a-z]+/" "$src" 2>/dev/null; then
      echo "BLOCKED: в prompt есть абсолютный путь репо ($(basename "$src"))" >&2
      return 4
    fi
  done
  return 0
}

sandbox_integrity_snapshot() {
  local repo="$1" out="$2"
  { echo "HEAD $(git -C "$repo" rev-parse HEAD)"
    echo "--- status ---"
    git -C "$repo" status --porcelain=v1
    echo "--- diff quiet ---"
    if git -C "$repo" diff --quiet; then echo clean; else echo DIRTY; fi
  } > "$out"
}

sandbox_integrity_check() {
  local repo="$1" snap="$2" cur="$2.current"
  sandbox_integrity_snapshot "$repo" "$cur"
  if diff -u "$snap" "$cur" > "$cur.diff" 2>&1; then
    return 0
  fi
  echo "BLOCKED: воркер писал вне песочницы (HEAD/статус репо изменились):" >&2
  head -20 "$cur.diff" >&2
  return 5
}

sandbox_diff() {
  local base="$1" sb="$2" out="$3"
  ( cd "$base" && find . -type f | sort ) > "$out.files.base"
  ( cd "$sb"   && find . -type f | sort ) > "$out.files.new"
  diff -ruN "$base" "$sb" > "$out.diff" 2>/dev/null
  local ins dele
  ins=$(grep -c '^+[^+]' "$out.diff" 2>/dev/null || echo 0)
  dele=$(grep -c '^-[^-]' "$out.diff" 2>/dev/null || echo 0)
  printf 'files_changed=%s insertions=%s deletions=%s\n' \
    "$(diff "$out.files.base" "$out.files.new" | grep -c '^>' || echo 0)" "$ins" "$dele" \
    > "$out.numstat"
  cat "$out.numstat"
}

sandbox_export() {
  local sb="$1" target="$2" repo="$3"
  local patch
  patch="$(mktemp /tmp/stellwerk-export.XXXXXX.patch)"
  ( cd "$sb" && find . -type f | sort ) > /tmp/.sb.new
  ( cd "$sb.base" && find . -type f | sort ) > /tmp/.sb.base
  local rel
  while read -r rel; do
    [ -f "$sb/$rel" ] || continue
    if ! cmp -s "$sb.base/$rel" "$sb/$rel"; then
      diff -u --label "a/$rel" --label "b/$rel" "$sb.base/$rel" "$sb/$rel" >> "$patch" || true
    fi
  done < /tmp/.sb.new
  if [ ! -s "$patch" ]; then
    echo "export: изменений нет"
    return 0
  fi
  echo "export: применяю patch в $target"
  git -C "$repo" apply --check "$patch" || { echo "export: patch не применился, отказ"; return 1; }
  git -C "$repo" apply "$patch" && echo "export: OK"
}
