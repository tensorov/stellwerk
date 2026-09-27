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
#   sandbox_build_patch <sandbox> <out.patch>     → git-применимый патч (до удаления .base)
#   sandbox_postflight <sandbox>                  → cargo fmt + clippy; !=0 → экспорт запрещён
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

# sandbox_build_patch — собрать патч, применимый через `git apply`.
#
# Происхождение (FINDING-02, сессия #2): sandbox_export пересобирал патч из
# `$sb.base`, но dispatch удалял `.base` в конце прогона (`rm -rf "$SB.base"`),
# и экспорт всегда рапортовал «изменений нет». Теперь патч собирается ВО ВРЕМЯ
# прогона, пока `.base` жив, и лежит рядом с логом.
sandbox_build_patch() {
  local sb="$1" out="$2"
  mkdir -p "$(dirname "$out")"
  : > "$out"
  [ -d "$sb.base" ] || { echo "export: нет $sb.base" >&2; return 1; }
  local rel tmp
  tmp="$(mktemp)"
  ( cd "$sb" && find . -type f | sort ) > "$tmp"
  while read -r rel; do
    # find . выдаёт './x'; в label это даёт 'a/./x', а git apply такой путь
    # отвергает («Invalid path»). Ведущий ./ убираем.
    rel="${rel#./}"
    [ -n "$rel" ] || continue
    [ -f "$sb/$rel" ] || continue
    if [ ! -f "$sb.base/$rel" ]; then
      # НОВЫЙ файл: в базе его нет, обычный diff падает на отсутствующей левой
      # стороне, и файл ТИХО выпадал из патча. Для git новая сущность
      # обозначается /dev/null слева — без этого пакет, создающий файл,
      # экспортировался бы неполно. (Поймано тестом T9b.)
      diff -u --label "/dev/null" --label "b/$rel" /dev/null "$sb/$rel" >> "$out" 2>/dev/null || true
    elif ! cmp -s "$sb.base/$rel" "$sb/$rel"; then
      diff -u --label "a/$rel" --label "b/$rel" "$sb.base/$rel" "$sb/$rel" >> "$out" 2>/dev/null || true
    fi
  done < "$tmp"
  rm -f "$tmp"
  [ -s "$out" ] && { echo "export: патч собран ($out)"; return 0; }
  echo "export: изменений нет"; return 3
}

# sandbox_postflight — привести песочницу к состоянию, которое вообще можно
# мержить: rustfmt, затем clippy -D warnings.
#
# Происхождение (сессия #2): воркер не запускает ни fmt, ни clippy, поэтому 2 из
# 5 первых пакетов требовали ручной правки интегратора, а clippy-ошибка всплывала
# уже после merge. Теперь это ловится до экспорта.
sandbox_postflight() {
  local sb="$1"
  [ -f "$sb/Cargo.toml" ] || { echo "postflight: нет Cargo.toml, пропуск"; return 0; }
  # ВАЖНО: только --check. Раньше здесь стоял `cargo fmt --all`, который
  # ПЕРЕПИСЫВАЕТ файлы. Патч к этому моменту уже собран из вывода рабочего
  # агента, поэтому postflight проверял переформатированную копию, а
  # экспортировал непереформатированный патч: гейт рапортовал «fmt чисто»,
  # а в ветку уезжал код с 90 строками замечаний fmt. Проверка обязана быть
  # read-only, иначе она подтверждает не тот артефакт, который уезжает.
  if ! ( cd "$sb" && cargo fmt --all -- --check ) > "$sb.postflight-fmt.log" 2>&1; then
    echo "FAILED(fmt): cargo fmt --check не прошёл, экспорт запрещён" >&2
    head -15 "$sb.postflight-fmt.log" >&2
    return 5
  fi
  if ! ( cd "$sb" && cargo clippy --workspace --all-targets -- -D warnings ) > "$sb.postflight-clippy.log" 2>&1; then
    echo "FAILED(clippy): clippy -D warnings не прошёл, экспорт запрещён" >&2
    head -15 "$sb.postflight-clippy.log" >&2
    return 5
  fi
  echo "postflight: fmt+clippy чисто"
  return 0
}

sandbox_export() {
  local sb="$1" target="$2" repo="$3"
  local patch="$sb.run/sandbox.patch"

  # Путь 1: патч собран во время прогона (предпочтительно — .base уже удалён).
  if [ ! -s "$patch" ]; then
    sandbox_build_patch "$sb" "$patch" >/dev/null 2>&1
  fi
  if [ ! -s "$patch" ]; then
    echo "export: изменений нет"
    return 3
  fi
  # Чистота обязана подтверждаться post-flight, иначе в ветку уедет код,
  # который не проходит gate.
  if ! sandbox_postflight "$sb"; then
    echo "export: postflight не пройден, экспорт запрещён" >&2
    return 6
  fi

  # Патч обязан соответствовать ПРОВЕРЕННОЙ песочнице. Пока .base жив, патч
  # пересобирается из текущего состояния — песочница здесь источник истины,
  # и устаревший патч уйти не может. Если .base уже удалён, корректность
  # обеспечивается порядком в диспетчере: postflight → build_patch → rm .base,
  # то есть патч снимается уже после успешной проверки.
  if [ -d "$sb.base" ]; then
    sandbox_build_patch "$sb" "$patch" >/dev/null 2>&1
  fi

  # Рабочее дерево должно быть ЧИСТЫМ. Иначе коммит смешает правки пакета с
  # чужими незакоммиченными изменениями, и diff перестаёт быть проверяемым.
  if ! git -C "$repo" diff --quiet || ! git -C "$repo" diff --cached --quiet; then
    echo "export: рабочее дерево не чистое, отказ (иначе патч смешается с чужими правками)" >&2
    git -C "$repo" status --porcelain >&2
    return 7
  fi

  if git -C "$repo" rev-parse --verify --quiet "$target" >/dev/null; then
    echo "export: ветка $target уже существует, отказ (переиспользование веток неоднозначно)" >&2
    return 8
  fi

  # Имена файлов берём ИЗ ПАТЧА (строки '+++ b/<path>'), и добавляем в индекс
  # только их: `git add -A` захватил бы посторонние untracked-файлы.
  local files orig
  files="$(awk '/^\+\+\+ /{sub(/^\+\+\+ b\//,""); print}' "$patch")"
  orig="$(git -C "$repo" rev-parse --abbrev-ref HEAD)"
  [ -n "$files" ] || { echo "export: в патче нет файлов"; return 9; }

  git -C "$repo" checkout -q -b "$target" || {
    echo "export: не создать ветку $target" >&2; return 1; }
  if ! git -C "$repo" apply "$patch"; then
    echo "export: patch не применился, откат на $orig" >&2
    git -C "$repo" checkout -q --force "$orig" 2>/dev/null
    git -C "$repo" branch -D "$target" >/dev/null 2>&1
    return 1
  fi
  # shellcheck disable=SC2086
  git -C "$repo" add -- $files
  if git -C "$repo" diff --cached --quiet; then
    echo "export: патч не дал изменений, откат" >&2
    git -C "$repo" checkout -q --force "$orig" 2>/dev/null
    git -C "$repo" branch -D "$target" >/dev/null 2>&1
    return 9
  fi
  git -C "$repo" commit -q -m "export: $target (stellwerk-dispatch)" \
    -m "Пакет перенесён из песочницы штатным путём: sandbox_build_patch + postflight." \
    -m "Export-Limit-Waiver: нет. Branch: $target" || {
      echo "export: коммит не создан" >&2
      git -C "$repo" checkout -q --force "$orig" 2>/dev/null; return 1; }
  git -C "$repo" checkout -q "$orig"
  echo "export: OK → ветка $target (1 коммит, $(printf '%s\n' "$files" | wc -l | tr -d ' ') файл(ов)); рабочее дерево возвращено на $orig"
  return 0
}
