# TUI-00 — бутстрап приёмочного хоста

**label:** — · **mutation_gate:** no (инфраструктура, кода продукта нет)
**depends_on:** [] · **ADR:** не требуется

## Цель

Каталога `~/gits/stellwerk` на хосте **нет** — проверено: `ls -d ~/gits/stellwerk`
→ `No such file or directory`. Без этого пакета каждая команда приёмки
обрывается на `cd` и даёт ложный `EXIT=1`.

Хост: `ssh -p 228 zeitoven.ru`, hostname `headless`, cargo 1.98.0, rustc 1.98.0,
rsync 3.5.1, tmux и script на месте, 124 G свободно.

## production_call

Не применимо — пакет инфраструктурный.

## files_allowed

Только журнал пакета под `.omo/`. Файлов продукта пакет не меняет.

## Что делать

1. Создать каталог на хосте:

```sh
ssh -p 228 zeitoven.ru 'mkdir -p ~/gits/stellwerk'
```

2. Синхронизация — **точная команда**, обязательная перед каждой сборкой
в следующих пакетах:

```sh
rsync -az --delete \
  --exclude 'target/' \
  --exclude 'mutants.out/' \
  --exclude 'verify/' --exclude '.worktrees/' --exclude '.ablation/' --exclude '.opencode/' \
  -e 'ssh -p 228' \
  ./ zeitoven.ru:~/gits/stellwerk/
```

`--delete` обязателен: без него файл, удалённый локально, остаётся на хосте
и сборка проверяет устаревший код — молчаливый ложный проход. Исключения
обязательны: `target/` локально 7.2 G, `mutants.out/` 7.5 M, `verify/` 2.2 M.
`.git` (19 M) пересылается — на хосте нужна та же история.

3. Гейт свежести перед каждой сборкой:

```sh
LOCAL=$(git -C /home/izyabretatel/gits/stellwerk rev-parse HEAD)
REMOTE=$(ssh -p 228 zeitoven.ru 'git -C ~/gits/stellwerk rev-parse HEAD')
test "$LOCAL" = "$REMOTE" && echo "SYNCED=$LOCAL" || echo "STALE local=$LOCAL remote=$REMOTE"
```

`STALE` — сборку не запускать, повторить rsync.

## Запрещено трогать

Всё дерево продукта. `cargo` локально не запускать ни в каком виде.

## Приёмка

```sh
ssh -p 228 zeitoven.ru 'ls -d ~/gits/stellwerk && git -C ~/gits/stellwerk rev-parse HEAD && cargo --version'; echo "HOST_EXIT=$?"
```

`HOST_EXIT=0`, напечатаны sha и версия cargo.

Гейт свежести обязан напечатать `SYNCED`.

**Доказательство исполнения на хосте, а не локально:** после первой сборки
`target` на хосте существует; его mtime проверен так:

```sh
ssh -p 228 zeitoven.ru 'stat -c %y ~/gits/stellwerk/target 2>/dev/null || echo NO_TARGET'
```

Отсутствие `NO_TARGET` — доказательство, что сборка шла на хосте.