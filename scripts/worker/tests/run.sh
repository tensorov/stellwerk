#!/usr/bin/env bash
# run.sh — регрессионные тесты лаунчера. Одна команда: scripts/worker/tests/run.sh
#
# Каждый тест обязан ЗЕЛЕНЫМ на исправленном лаунчере и ПАДАЮЩИМ на исправленном
# ещё нет. Контракт (T1 export, T2 export пустой, T3/T4 budget, T5 норма и
# детектор цикла, T6 fmt post-flight, T7 clippy post-flight) зафиксирован
# ADR docs/adr/0001-manual-async-fn.md и разбором сессии #3 в .night/STATE.md.
#
# Только stdlib: bash + python3. Никаких внешних зависимостей, никакого сети.
set -uo pipefail

WORKER="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$WORKER/../.." && pwd)"
TMP="$(mktemp -d /tmp/worker-tests-XXXXXX)"
trap 'rm -rf "$TMP"' EXIT
export SANDBOX_ROOT="$TMP/sandboxes"

PASS=0; FAIL=0; FAILED_NAMES=""

ok()   { PASS=$((PASS+1)); printf '  \033[32mok\033[0m   %s\n' "$1"; }
bad()  { FAIL=$((FAIL+1)); FAILED_NAMES="$FAILED_NAMES $1"; printf '  \033[31mFAIL\033[0m %s\n' "$1"; shift; [ $# -gt 0 ] && printf '        %s\n' "$@"; }
head2(){ printf '\n\033[1m%s\033[0m\n' "$1"; }

# ── фикстуры ────────────────────────────────────────────────────────────────
# mk_jsonl <file> <tool_steps> <input_cum> <turns> [identical_tool_name]
# Детерминированный синтез потока, который читают loop_guard и validity_gate.
mk_jsonl() {
  python3 - "$@" <<'PY'
import json, sys
path, steps, input_cum, turns = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4])
# 5-й позиционный аргумент — РЕЖИМ ('ident' | 'varied'), не имя инструмента.
mode = sys.argv[5] if len(sys.argv) > 5 else 'ident'
name = 'read'
varied = (mode == 'varied')
# input_cum — ЦЕЛЕВОЕ значение: делим так, чтобы сумма по turn_end была
# не меньше цели (округление вниз иначе даёт ровно порог, а нужен «строго выше»).
per = -(-input_cum // max(1, turns))
with open(path, 'w') as fh:
    def ev(o):
        fh.write(json.dumps(o) + '\n')
    ev({'type': 'session', 'version': 3, 'id': 'fix', 'timestamp': '2026-09-27T00:00:00Z'})
    g = 0
    for t in range(turns):
        for s in range(steps // turns + (1 if t < steps % turns else 0)):
            # Именованный инструмент одинаков на каждом шаге: это и есть «цикл».
            if varied:
                # Аргументы уникальны ГЛОБАЛЬНО: иначе внутри хода повторятся
                # одинаковые вызовы и consec-guard сработает ложно.
                ev({'type': 'tool_execution_start', 'toolName': name,
                    'args': {'path': 'src/f%d.rs' % g, 'pattern': 'SYM%d' % g}})
            else:
                ev({'type': 'tool_execution_start', 'toolName': name,
                    'args': {'path': 'crates/stellwerk-core/src/contract.rs', 'pattern': 'TODO'}})
            g += 1
            ev({'type': 'message_update', 'usage': {'input': per, 'output': 20,
                                                    'cacheRead': 0, 'cacheWrite': 0}})
        ev({'type': 'turn_end', 'toolResults': [],
            'message': {'stopReason': 'stop', 'usage': {'input': per, 'output': 20,
                                                       'cacheRead': 0, 'cacheWrite': 0}}})
    ev({'type': 'message_end', 'message': {'role': 'system', 'content': ''}})
    for _ in range(turns):
        ev({'type': 'message_end', 'message': {'role': 'assistant', 'content': 'STATUS: DONE'}})
PY
}

# ── T1: export с реальным изменением ────────────────────────────────────────
head2 "T1 export: sandbox с изменением → ветка получает коммит"
T1REPO="$TMP/repo1"; mkdir -p "$T1REPO"
git -C "$T1REPO" init -q
printf 'fn main() {}\n' > "$T1REPO/main.rs"
git -C "$T1REPO" add -A && git -C "$T1REPO" -c user.email=t@t -c user.name=t commit -qm init
# shellcheck disable=SC1091
source "$WORKER/sandbox.sh"
SB1="$(sandbox_create "$T1REPO" HEAD t1 2>/dev/null)"
printf 'fn main() { println!("hi"); }\n' > "$SB1/main.rs"
# Шаг прогона: патч собирается, ПОКА .base жив (это и есть исправление).
sandbox_build_patch "$SB1" "$SB1.run/sandbox.patch" >/dev/null 2>&1
# Имитация конца прогона: .base удаляется, как это делает dispatch.
mv "$SB1.base" "$TMP/base1-gone"
( sandbox_export "$SB1" t1-branch "$T1REPO" ) > "$TMP/t1.log" 2>&1
T1RC=$?
# Контракт export: ветка создана, изменение в ней, дерево не запачкано.
T1BR=$(git -C "$T1REPO" rev-parse --verify --quiet t1-branch >/dev/null && echo yes || echo no)
T1IN=$(git -C "$T1REPO" show t1-branch:main.rs 2>/dev/null | grep -c println || echo 0)
T1CLEAN=$(git -C "$T1REPO" status --porcelain | wc -l | tr -d ' ')
if [ "$T1RC" -eq 0 ] && [ "$T1BR" = yes ] && [ "$T1IN" -gt 0 ] && [ "$T1CLEAN" -eq 0 ]; then
  ok "T1 export создал ветку с изменением, дерево осталось чистым"
else
  bad "T1 export не выполнил контракт" "rc=$T1RC branch=$T1BR изменение_в_ветке=$T1IN дерево_грязное=$T1CLEAN" "лог: $(tail -2 "$TMP/t1.log" | tr '\n' ' ')"
fi

# ── T2: export без изменений ────────────────────────────────────────────────
head2 "T2 export пустой: честный отказ, ветка не создаётся"
T2REPO="$TMP/repo2"; mkdir -p "$T2REPO"
git -C "$T2REPO" init -q
printf 'fn main() {}\n' > "$T2REPO/main.rs"
git -C "$T2REPO" add -A && git -C "$T2REPO" -c user.email=t@t -c user.name=t commit -qm init
SB2="$(sandbox_create "$T2REPO" HEAD t2 2>/dev/null)"
( sandbox_export "$SB2" t2-branch "$T2REPO" ) > "$TMP/t2.log" 2>&1
T2RC=$?
if [ "$T2RC" -ne 0 ] && grep -qi 'изменений нет\|no changes' "$TMP/t2.log"; then
  ok "T2 export без изменений отказал (rc=$T2RC) и не создал ветку"
else
  bad "T2 export без изменений отработал неверно" "rc=$T2RC (ожидался != 0)" "лог: $(tail -2 "$TMP/t2.log" | tr '\n' ' ')"
fi

# ── T3: budget по tool_steps ────────────────────────────────────────────────
head2 "T3 budget: tool_steps=31 → FAILED(budget)"
mk_jsonl "$TMP/budget-steps.jsonl" 31 20000 8
python3 "$WORKER/validity_gate.py" --jsonl "$TMP/budget-steps.jsonl" \
  --out "$TMP/t3.json" --max-tool-steps 30 > "$TMP/t3.log" 2>&1
T3RC=$?
T3VALID=$(python3 -c "import json;print(json.load(open('$TMP/t3.json'))['valid'])" 2>/dev/null || echo missing)
if [ "$T3RC" -ne 0 ] && [ "$T3VALID" = "False" ]; then
  ok "T3 превышение шагов отвергнуто (rc=$T3RC, valid=False)"
else
  bad "T3 превышение шагов НЕ отвергнуто" "rc=$T3RC valid=$T3VALID (ожидался rc!=0, False)"
fi

# ── T4: budget по input_cum ────────────────────────────────────────────────
head2 "T4 budget: input_cum=300001 → FAILED(budget)"
mk_jsonl "$TMP/budget-input.jsonl" 5 300001 8
python3 "$WORKER/validity_gate.py" --jsonl "$TMP/budget-input.jsonl" \
  --out "$TMP/t4.json" --max-input-cum 300000 > "$TMP/t4.log" 2>&1
T4RC=$?
T4VALID=$(python3 -c "import json;print(json.load(open('$TMP/t4.json'))['valid'])" 2>/dev/null || echo missing)
if [ "$T4RC" -ne 0 ] && [ "$T4VALID" = "False" ]; then
  ok "T4 превышение контекста отвергнуто (rc=$T4RC, valid=False)"
else
  bad "T4 превышение контекста НЕ отвергнуто" "rc=$T4RC valid=$T4VALID (ожидался rc!=0, False)"
fi

# ── T5: норма проходит, зацикленные прогоны ловятся ────────────────────────
head2 "T5 норма → VALID; зациклленные M1 (38–43 шага) → пойманы"
mk_jsonl "$TMP/norm.jsonl" 9 40000 5
python3 "$WORKER/validity_gate.py" --jsonl "$TMP/norm.jsonl" \
  --out "$TMP/t5n.json" --max-tool-steps 30 --max-input-cum 300000 > "$TMP/t5n.log" 2>&1
T5RC=$?
if [ "$T5RC" -eq 0 ]; then
  ok "T5 норма (9 шагов, 40k input) прошла валидность"
else
  bad "T5 норма отвергнута" "$(tail -2 "$TMP/t5n.log" | tr '\n' ' ')"
fi
# Метрики взяты из validity.json прогонов сессии #2 (реальные run.jsonl из /tmp
# не сохранились — в репозиторий попадал только validity.json, поэтому фикстуры
# восстановлены по зафиксированным числам: steps и input_cum).
# Реальные прогоны M1: 38–43 шага, input_cum 480–790k, вызовы РАЗНЫЕ.
# Consec-guard их не ловил (validity.json: valid=true) — ловить обязан budget.
T5LOOP=0; T5LOOPD=0
for spec in "38 650318" "43 787190" "31 481021"; do
  set -- $spec
  mk_jsonl "$TMP/loop-$1.jsonl" "$1" "$2" 40 varied
  python3 "$WORKER/validity_gate.py" --jsonl "$TMP/loop-$1.jsonl" \
    --out "$TMP/t5l-$1.json" --max-tool-steps 30 --max-input-cum 300000 >/dev/null 2>&1
  V=$(python3 -c "import json;print(json.load(open('$TMP/t5l-$1.json'))['valid'])" 2>/dev/null || echo err)
  if [ "$V" = "False" ]; then T5LOOP=$((T5LOOP+1)); else T5LOOPD=$((T5LOOPD+1)); fi
done
if [ "$T5LOOP" = "3" ]; then
  ok "T5 все 3 разношаговых прогона M1 (38/43/31 шагов) пойманы budget-guard"
else
  bad "T5 зацикленные прогоны НЕ пойманы budget-guard" "поймано $T5LOOP из 3, пропущено $T5LOOPD"
fi
# Разношаговый прогон НЕ должен ловиться consec-guard — это подтверждает, что
# бюджетные пороги закрывают реальный режим отказа, а не только очевидный цикл.
mk_jsonl "$TMP/varied-25.jsonl" 25 200000 10 varied
if python3 "$WORKER/loop_guard.py" --jsonl "$TMP/varied-25.jsonl" --trigger "$TMP/lg2.json" \
     --consec-limit 20 --total-limit 0 --dry-run >/dev/null 2>&1; then
  bad "T5 consec-guard ловит разношаговый прогон" "это ложное срабатывание, отладка фикстуры"
else
  ok "T5 consec-guard корректно молчит на разношаговом прогоне (ловец — budget)"
fi

# ── T5d: калибровка бюджета зафиксирована тестом ───────────────────────────
# Порог шагов обязан быть выше легитимной работы (31 шаг у M2 packet-03) и
# порог контекста — ниже всех наблюдавшихся runaway (480k). Если эти числа
# изменятся молча, тест упадёт.
head2 "T5d калибровка: шаги выше легитимной работы, контекст ниже всех runaway"
# Числа берём из строки присваивания, а не любым вхождением имени: в dispatch.conf
# есть и комментарий с этими именами, и grep по нему давал пустое.
MAXS=$(grep -E '^MAX_TOOL_STEPS=\$\{MAX_TOOL_STEPS:-[0-9]+\}' "$WORKER/dispatch.conf" | grep -oE '[0-9]+' | tail -1)
MAXI=$(grep -E '^MAX_INPUT_CUM=\$\{MAX_INPUT_CUM:-[0-9]+\}'  "$WORKER/dispatch.conf" | grep -oE '[0-9]+' | tail -1)
if [ -n "$MAXS" ] && [ -n "$MAXI" ] && [ "$MAXS" -ge 40 ] && [ "$MAXI" -le 400000 ]; then
  ok "калибровка: MAX_TOOL_STEPS=$MAXS (>=40), MAX_INPUT_CUM=$MAXI (<=400k)"
else
  bad "калибровка нарушена" "MAX_TOOL_STEPS=${MAXS:-пусто} (нужно >=40), MAX_INPUT_CUM=${MAXI:-пусто} (нужно <=400000)"
fi

# ── T6: post-flight форматирования ─────────────────────────────────────────
head2 "T6 fmt: неотформатированный .rs приводится к rustfmt ДО экспорта"
if declare -f sandbox_postflight >/dev/null 2>&1; then
  T6REPO="$TMP/repo6"; mkdir -p "$T6REPO"
  git -C "$T6REPO" init -q
  printf '[package]\nname = "x"\nversion = "0.0.1"\nedition = "2021"\n' > "$T6REPO/Cargo.toml"
  mkdir -p "$T6REPO/src"
  printf 'fn  main( ) {let x=1;   println!("{}",x);}\n' > "$T6REPO/src/main.rs"
  git -C "$T6REPO" add -A && git -C "$T6REPO" -c user.email=t@t -c user.name=t commit -qm init
  SB6="$(sandbox_create "$T6REPO" HEAD t6 2>/dev/null)"
  T6PRE=$( cd "$SB6" && cargo fmt --all --check >/dev/null 2>&1; echo $? )
  ( sandbox_postflight "$SB6" ) > "$TMP/t6.log" 2>&1
  T6RC=$?
  ( cd "$SB6" && cargo fmt --all --check ) >/dev/null 2>&1
  T6POST=$?
  if [ "$T6PRE" -ne 0 ] && [ "$T6POST" -eq 0 ] && [ "$T6RC" -eq 0 ]; then
    ok "T6 postflight отформатировал файл (было неотформатировано)"
  else
    bad "T6 postflight НЕ отформатировал" "fmt до=$T6PRE после=$T6POST postflight_rc=$T6RC"
  fi
else
  bad "T6 sandbox_postflight не объявлена" "правка sandbox.sh не добавила post-flight функцию"
fi

# ── T7: post-flight clippy ─────────────────────────────────────────────────
head2 "T7 clippy: clippy-warning → валидность FAILED(clippy), экспорт запрещён"
if declare -f sandbox_postflight >/dev/null 2>&1; then
  T7REPO="$TMP/repo7"; mkdir -p "$T7REPO"
  git -C "$T7REPO" init -q
  printf '[package]\nname = "y"\nversion = "0.0.1"\nedition = "2021"\n' > "$T7REPO/Cargo.toml"
  mkdir -p "$T7REPO/src"
  # Ровно то, на что ругается clippy::all: ручной split_once + сравнение символов.
  cat > "$T7REPO/src/main.rs" <<'RS'
fn split(s: &str) -> bool {
    let p = s.splitn(2, "://").nth(1);
    p.map(|r| r.find(|c| c == '/' || c == '?' || c == '#').is_none()).unwrap_or(false)
}
fn main() { let _ = split("a://b"); }
RS
  git -C "$T7REPO" add -A && git -C "$T7REPO" -c user.email=t@t -c user.name=t commit -qm init
  SB7="$(sandbox_create "$T7REPO" HEAD t7 2>/dev/null)"
  ( sandbox_postflight "$SB7" ) > "$TMP/t7.log" 2>&1
  T7RC=$?
  if [ "$T7RC" -ne 0 ] && grep -qi 'clippy' "$TMP/t7.log"; then
    ok "T7 clippy-warning отвергнут post-flight (rc=$T7RC)"
  else
    bad "T7 clippy-warning НЕ отвергнут" "rc=$T7RC (ожидался != 0) лог: $(tail -3 "$TMP/t7.log" | tr '\n' ' ')"
  fi
else
  bad "T7 sandbox_postflight не объявлена" "правка sandbox.sh не добавила post-flight функцию"
fi

# ── T8: replay реальных трасс (калибровка обязана быть воспроизводимой) ─────
# Четыре настоящих run.jsonl, сжатых gzip, в tests/traces/. Калибровка
# MAX_INPUT_CUM=400000 стоила двух провалов и одной неверной гипотезы, поэтому
# проверяется на фактических трассах, а не на синтетике.
head2 "T8 replay: реальные трассы дают ожидаемые вердикты"
TR="$WORKER/tests/traces"
T8OK=0; T8BAD=0
replay() { # <файл> <ожидаем valid: true|false> <описание>
  local f="$TR/$1.jsonl.gz" want="$2" desc="$3" gz
  gz="$TMP/replay.jsonl"
  gzip -dc "$f" > "$gz" 2>/dev/null || { bad "T8 $desc: не распаковать"; T8BAD=$((T8BAD+1)); return; }
  python3 "$WORKER/validity_gate.py" --jsonl "$gz" --out "$TMP/replay.json" \
    --max-tool-steps 60 --max-input-cum 400000 >/dev/null 2>&1
  local got
  got=$(python3 -c "import json;print(str(json.load(open('$TMP/replay.json'))['valid']).lower())" 2>/dev/null || echo err)
  if [ "$got" = "$want" ]; then
    T8OK=$((T8OK+1)); ok "T8 $desc → valid=$got (ожидался $want)"
  else
    T8BAD=$((T8BAD+1)); bad "T8 $desc → valid=$got, ожидался $want"
  fi
}
if [ -d "$TR" ] && ls "$TR"/*.jsonl.gz >/dev/null 2>&1; then
  replay skeleton-packet-01-20260927-071915 true  "M2-01  36k  норма"
  replay cli-red-packet-03-20260927-081527    true  "M2-03 328k ЛЕГИТИМНО (при 300k убивался)"
  replay taskfile-packet-02-20260927-080839   false "M2-02 530k runaway"
  replay taskfile-packet-02-20260927-080409   false "M2-02 591k runaway"
  # Регрессия самого решения: на 300000 легитимная трасса обязана отвергаться.
  # Это и есть причина, по которой лимит изменили — тест не даст вернуть 300k.
  gzip -dc "$TR/cli-red-packet-03-20260927-081527.jsonl.gz" > "$TMP/legit.jsonl"
  python3 "$WORKER/validity_gate.py" --jsonl "$TMP/legit.jsonl" --out "$TMP/old.json" \
    --max-tool-steps 60 --max-input-cum 300000 >/dev/null 2>&1
  OLD=$(python3 -c "import json;print(str(json.load(open('$TMP/old.json'))['valid']).lower())" 2>/dev/null || echo err)
  if [ "$OLD" = "false" ]; then
    ok "T8 регрессия: при 300000 легитимная трасса отвергается (значит смена лимита была необходима)"
  else
    bad "T8 регрессия не воспроизводится" "при 300000 трасса прошла — основание для смены лимита не подтверждается"
  fi
else
  bad "T8 нет корпуса трасс" "каталог $TR пуст — калибровка непроверяема"
fi

# ── T9: export создаёт ветку, а не просто пачкает дерево ────────────────────
# Canary сессии #3 поймал: экспорт печатал 'export: OK', НЕ создавал ветку и
# применял патч прямо в рабочее дерево. Аргумент <branch> использовался только
# в тексте сообщения. Проверяем явно все три следствия.
head2 "T9 export: ветка создаётся, дерево остаётся чистым"
T9REPO="$TMP/repo9"; mkdir -p "$T9REPO"
git -C "$T9REPO" init -q
printf 'fn main() {}\n' > "$T9REPO/main.rs"
git -C "$T9REPO" add -A && git -C "$T9REPO" -c user.email=t@t -c user.name=t commit -qm init
SB9="$(sandbox_create "$T9REPO" HEAD t9 2>/dev/null)"
printf 'fn main() { println!("canary"); }\n' > "$SB9/main.rs"
printf 'fn extra() {}\n' > "$SB9/extra.rs"          # новый файл — тоже должен попасть
sandbox_build_patch "$SB9" "$SB9.run/sandbox.patch" >/dev/null 2>&1
mv "$SB9.base" "$TMP/base9-gone"
( sandbox_export "$SB9" t9-branch "$T9REPO" ) > "$TMP/t9.log" 2>&1
T9RC=$?
# a) ветка существует
if git -C "$T9REPO" rev-parse --verify --quiet t9-branch >/dev/null; then T9A=ok; else T9A=bad; fi
# b) в ветке ровно один коммит сверх базы и ровно 2 файла
T9N=$(git -C "$T9REPO" rev-list --count main2..t9-branch 2>/dev/null || git -C "$T9REPO" rev-list --count HEAD..t9-branch 2>/dev/null || echo err)
T9F=$(git -C "$T9REPO" show --name-only --format="" t9-branch 2>/dev/null | grep -c . || echo 0)
# c) рабочее дерево осталось на исходной ветке и чистое
T9CLEAN=$(git -C "$T9REPO" status --porcelain | wc -l | tr -d ' ')
T9CUR=$(git -C "$T9REPO" rev-parse --abbrev-ref HEAD)
[ "$T9A" = ok ] && ok "T9a ветка t9-branch создана" || { bad "T9a ветка НЕ создана (rc=$T9RC)"; sed 's/^/        /' "$TMP/t9.log"; }
if [ "$T9N" = "1" ] && [ "$T9F" = "2" ]; then
  ok "T9b в ветке 1 коммит и 2 файла (main.rs + новый extra.rs)"
else
  bad "T9b состав неверный" "коммитов=$T9N файлов=$T9F (ожидались 1 и 2)"
fi
if [ "$T9CLEAN" = "0" ] && [ "$T9CUR" != "t9-branch" ]; then
  ok "T9c дерево чистое и возвращено на $T9CUR (патч не остался в нём)"
else
  bad "T9c дерево загрязнено" "незакоммиченных=$T9CLEAN текущая ветка=$T9CUR"
fi
# d) повторный export в существующую ветку отвергается
( sandbox_export "$SB9" t9-branch "$T9REPO" ) > "$TMP/t9d.log" 2>&1
if [ $? -ne 0 ] && grep -qi 'уже существует' "$TMP/t9d.log"; then
  ok "T9d повторный export в существующую ветку отвергнут (без тихой перезаписи)"
else
  bad "T9d повторный export не отвергнут" "$(tail -2 "$TMP/t9d.log" | tr '\n' ' ')"
fi
# e) грязное рабочее дерево отвергается (иначе коммит смешает правки)
printf '// stray\n' >> "$T9REPO/main.rs"
( sandbox_export "$SB9" t9-dirty "$T9REPO" ) > "$TMP/t9e.log" 2>&1
T9ERC=$?
git -C "$T9REPO" restore main.rs 2>/dev/null || git -C "$T9REPO" checkout -q -- main.rs
if [ "$T9ERC" -ne 0 ] && grep -qi 'не чистое' "$TMP/t9e.log"; then
  ok "T9e export при грязном дереве отвергнут"
else
  bad "T9e export прошёл при грязном дереве" "rc=$T9ERC"
fi

# ── итог ───────────────────────────────────────────────────────────────────
printf '\n────────────────────────────────────────\n'
printf 'passed: %d   failed: %d\n' "$PASS" "$FAIL"
[ "$FAIL" -gt 0 ] && { printf 'failing:%s\n' "$FAILED_NAMES"; exit 1; }
exit 0
