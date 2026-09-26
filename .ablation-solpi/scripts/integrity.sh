#!/usr/bin/env bash
# integrity.sh — Phase B, B7 (исправленная семантика).
#
# Харнесс САМ создаёт файлы внутри .ablation-solpi/, поэтому буквальное сравнение
# полного `git status --porcelain` даёт ложный FAIL. Инвариант вместо этого:
#   1) HEAD основного репозитория не меняется;
#   2) за пределами .ablation-solpi/ рабочее дерево побайтово то же (tracked + untracked);
#   3) tracked-diff вне .ablation-solpi/ пуст;
#   4) .worktrees/m0/* не меняются (HEAD + porcelain каждого worktree);
#   5) регистрация worktree не меняется.
#
# Режимы:  save | check | secrets
#   save    — снять эталонный снимок
#   check   — снять текущий и сравнить с эталоном; exit 1 = FAIL
#   secrets — проверить, что pi-home игнорируются git (печатает только вердикт)
set -uo pipefail

REPO="${STELLWERK_REPO:-/home/izyabretatel/gits/stellwerk}"
ROOT="$REPO/.ablation-solpi"
STATE_DIR="${STELLWERK_INTEGRITY_STATE:-$ROOT/runs/.integrity}"
# пути для проверки: всё, КРОМЕ .ablation-solpi
OUTSIDE=( . ':(exclude).ablation-solpi/**' )
MODE="${1:-check}"

git_q() { git -C "$REPO" "$@"; }

snapshot() {
  echo "HEAD $(git_q rev-parse HEAD)"
  echo "--- status (tracked+untracked, вне .ablation-solpi) ---"
  git_q status --porcelain=v1 -- "${OUTSIDE[@]}"
  echo "--- tracked diff quiet (вне .ablation-solpi) ---"
  if git_q diff --quiet -- "${OUTSIDE[@]}"; then echo clean; else echo DIRTY; fi
  echo "--- untracked вне .ablation-solpi ---"
  local u
  u="$(git_q ls-files --others --exclude-standard -- "${OUTSIDE[@]}")"
  if [ -z "$u" ]; then echo none; else echo "$u"; fi
  echo "--- worktree registrations ---"
  if [ "${STELLWERK_IGNORE_RUN_WORKTREES:-0}" = "1" ]; then
    # рабочий worktree текущего прогона существует по устройству; исключаем ТОЛЬКО runs/
    # запись многострочная (worktree/HEAD/branch/detached+пустая строка) —
    # режем ЗАПИСЬ целиком, иначе остаются сиротские HEAD/detached
    git_q worktree list --porcelain | awk '
      /^worktree /{ skip = ($0 ~ /\.ablation-solpi\/runs\//) }
      skip { next }
      { print }'
  else
    git_q worktree list --porcelain
  fi
  echo "--- .worktrees/m0 ---"
  local d
  for d in "$REPO"/.worktrees/m0/*/; do
    [ -d "$d" ] || continue
    echo "m0 $(basename "$d") $(git -C "$d" rev-parse HEAD 2>/dev/null || echo nohead)"
    git -C "$d" status --porcelain=v1 2>/dev/null
  done
}

case "$MODE" in
  save)
    mkdir -p "$STATE_DIR"
    umask 077
    snapshot > "$STATE_DIR/baseline.txt"
    echo "baseline snapshot: $STATE_DIR/baseline.txt ($(wc -l < "$STATE_DIR/baseline.txt") lines)"
    ;;
  check)
    if [ ! -f "$STATE_DIR/baseline.txt" ]; then
      echo "INTEGRITY FAIL: нет эталонного снимка ($STATE_DIR/baseline.txt); сначала run: save"
      exit 1
    fi
    snapshot > "$STATE_DIR/current.txt"
    if diff -u "$STATE_DIR/baseline.txt" "$STATE_DIR/current.txt" > "$STATE_DIR/diff.txt" 2>&1; then
      echo "INTEGRITY OK: HEAD + дерево вне .ablation-solpi + .worktrees/m0 не изменились"
      exit 0
    fi
    echo "INTEGRITY FAIL: расхождение с эталоном (первые 40 строк diff):"
    head -40 "$STATE_DIR/diff.txt"
    echo "STOP: прогоны немедленно остановлены."
    exit 1
    ;;
  secrets)
    rc=0
    for d in pi-home-baseline pi-home-solpi; do
      p=".ablation-solpi/$d"
      if git_q check-ignore -q "$p"; then
        echo "IGNORED       $p"
      else
        echo "NOT-IGNORED!! $p"; rc=1
      fi
      # копии ключей не должны быть trackable
      for f in "$p/auth.json" "$p/models.json"; do
        if [ -e "$REPO/$f" ]; then
          perm=$(stat -c '%a' "$REPO/$f")
          if [ "$perm" = "600" ]; then echo "  perm 600    $f"
          else echo "  PERM $perm !! $f"; rc=1; fi
        fi
      done
    done
    exit $rc
    ;;
  *)
    echo "usage: integrity.sh {save|check|secrets}"; exit 2
    ;;
esac
