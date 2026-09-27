#!/usr/bin/env bash
# Слияние решения пакета в main по --no-ff.
# Обход дефекта лаунчера: `stellwerk-dispatch --export` пересобирает патч из
# каталога .base, который к моменту экспорта уже удалён, и всегда рапортует
# «изменений нет». Код лаунчера менять нельзя, поэтому патч собирается отсюда
# из сохранённого .run/sandbox.diff + содержимого песочницы.
set -eu
SB="$1"; PKT="$2"; BR="$3"; SUBJ="$4"
ROOT=/home/izyabretatel/gits/stellwerk
cd "$ROOT"
# список изменённых файлов из сохранённого diff
mapfile -t FILES < <(grep -E '^\+\+\+ ' "$SB.run/sandbox.diff" | sed "s|.*$SB/||" | awk '{print $1}' | sort -u)
echo "  файлов в патче: ${#FILES[@]}"
git checkout -q -b "$BR" main
for f in "${FILES[@]}"; do
  mkdir -p "$(dirname "$f")"
  cp "$SB/$f" "$f"
done
cargo fmt --all   # FINDING-01: воркер не форматирует (вставляет в branch-коммит)
git add "${FILES[@]}"
git commit -q -m "$SUBJ"
git checkout -q main
git merge --no-ff "$BR" -m "merge: $PKT ($BR)" >/dev/null
echo "  слито: $(git log --oneline -1)"
