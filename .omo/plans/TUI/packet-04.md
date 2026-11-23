# TUI-04 — приёмка среза, включая реальный терминал

**label:** — · **mutation_gate:** no (приёмка, кода не добавляет)
**depends_on:** [TUI-03] · **ADR:** не требуется

## Цель

Снять приёмку целиком на хосте. Ключевая поправка ревью: `ssh -tt` **не
умеет** интерактив — он выделяет pty на одну команду и рвёт его на выходе,
агент не нажмёт `START`. На хосте есть `tmux` и `script`, поэтому процедура
задаётся явно.

## production_call

`stellwerk tui crates/stellwerk-cli/examples/pi-smoke.toml` на хосте, в tmux.

## files_allowed

Только журнал пакета под `.omo/`. Файлов продукта пакет не меняет.

## Что делать

1. Полная картина на хосте:

```sh
ssh -p <port> <host> 'cd ~/gits/stellwerk && cargo build --workspace --all-targets'; echo "BUILD_EXIT=$?"
ssh -p <port> <host> 'cd ~/gits/stellwerk && cargo test --workspace --no-fail-fast'; echo "WS_EXIT=$?"
ssh -p <port> <host> 'cd ~/gits/stellwerk && cargo fmt --all -- --check'; echo "FMT_EXIT=$?"
ssh -p <port> <host> 'cd ~/gits/stellwerk && cargo clippy --workspace --all-targets -- -D warnings'; echo "CLIPPY_EXIT=$?"
```

Все четыре — `0`.

2. **Реальный терминал — процедура на tmux** (взамен неисполнимого `ssh -tt`):

```sh
ssh -p <port> <host> "tmux kill-session -t tuiq 2>/dev/null; tmux new-session -d -s tuiq 'cd ~/gits/stellwerk && cargo run -p stellwerk-cli -- tui crates/stellwerk-cli/examples/pi-smoke.toml'"
sleep 6
ssh -p <port> <host> "tmux capture-pane -p -t tuiq"
ssh -p <port> <host> "tmux send-keys -t tuiq Enter"
sleep 6
ssh -p <port> <host> "tmux capture-pane -p -t tuiq" > /tmp/opencode/tui-after-start.txt
cat /tmp/opencode/tui-after-start.txt
ssh -p <port> <host> "tmux kill-session -t tuiq 2>/dev/null"
```

Снимок **до** `START` обязан показывать task-файл, агента и действие `START`,
и **не** показывать исполнение. Снимок **после** обязан показывать вердикт,
id receipt, риск и строку «ждёт человека». Оба снимка идут в evidence
дословно.

## Запрещено трогать

Всё дерево продукта. Локально `cargo` не запускать.

## Почему `mutation_gate: no`

Пакет приёмки, кода не добавляет.

## Приёмка

Критерий пакета — четыре нулевых exit-кода **плюс** два снимка tmux,
соответствующие описанному. Снимок после `START` без строки «ждёт человека»
— провал пакета: значит гейт не отображается.

**Доказательство, что всё исполнялось на хосте:**

```sh
ssh -p <port> <host> 'ls -d ~/gits/stellwerk/target >/dev/null && echo REMOTE_BUILD_OK || echo LOCAL_ONLY'
```

Обязан напечатать `REMOTE_BUILD_OK`.