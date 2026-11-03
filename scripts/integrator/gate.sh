#!/bin/sh
# gate.sh — CI-гейт PolyForge. Запускает MCP-сервер по stdio и зовёт
# `gate_evaluate` для переданного task_id. Использование:
#   scripts/integrator/gate.sh <task_id>
#
# Код возврата — это вердикт, а не подсказка:
#   0  gate_evaluate вернул result.ok == true (аттестовано);
#   1  гейт не пройден: ok:false, битый ответ, сбой протокола или ненулевой child;
#   2  PolyForge отсутствует или modules.toml не даёт команду (fail-closed).
#
# Пакет i01: гейт, который печатает TODO, не отличает проверенный бандл от
# непроверенного. Отсутствие PolyForge — провал, никогда не разрешение.
set -u

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd) || {
    printf '%s\n' 'MissingPolyForge: cannot locate repository root' >&2
    exit 2
}

exec python3 - "$ROOT" "$@" <<'PY'
import json
import os
import re
import shlex
import shutil
import subprocess
import sys

ROOT = sys.argv[1]
ARGS = sys.argv[2:]


def fail(code, marker, detail):
    sys.stderr.write("%s: %s\n" % (marker, detail))
    sys.exit(code)


def command_from_modules(root):
    """Read only `command = "..."` under `[module.polyforge]`.

    Any parse failure returns None, which the caller turns into exit 2 — never
    into a default that could look like success.
    """
    try:
        with open(os.path.join(root, "modules.toml"), encoding="utf-8") as handle:
            text = handle.read()
    except OSError:
        return None
    section = None
    for raw in text.splitlines():
        line = raw.split("#", 1)[0]
        header = re.match(r"^\s*\[\s*([^\]]+?)\s*\]\s*$", line)
        if header:
            section = header.group(1)
            continue
        if section != "module.polyforge":
            continue
        command = re.match(r'^\s*command\s*=\s*"([^"]*)"\s*$', line)
        if command:
            return command.group(1)
    return None


def resolve_command():
    override = os.environ.get("POLYFORGE_CMD", "").strip()
    raw = override if override else command_from_modules(ROOT)
    if not raw:
        fail(2, "MissingPolyForge", "modules.toml has no command for [module.polyforge]")
    try:
        argv = shlex.split(raw)
    except ValueError as exc:
        fail(2, "MissingPolyForge", "invalid command %r: %s" % (raw, exc))
    if not argv or shutil.which(argv[0]) is None:
        fail(2, "MissingPolyForge", "command %r is absent or not executable" % raw)
    return argv


if not ARGS or not ARGS[0].strip():
    fail(2, "MissingTaskId", "usage: gate.sh <task_id>")
TASK_ID = ARGS[0]
COMMAND = resolve_command()

try:
    child = subprocess.Popen(
        COMMAND,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        bufsize=1,
    )
except OSError as exc:
    fail(2, "MissingPolyForge", "cannot start %r: %s" % (COMMAND[0], exc))


def send(message):
    child.stdin.write(json.dumps(message, separators=(",", ":")) + "\n")
    child.stdin.flush()


def read_response(wanted_id):
    while True:
        line = child.stdout.readline()
        if line == "":
            fail(1, "ProtocolFailure", "server closed stdout before id=%d" % wanted_id)
        line = line.strip()
        if not line:
            continue
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            fail(1, "ProtocolFailure", "malformed response: %s" % line)
        if message.get("id") != wanted_id:
            continue  # notification or a late reply to an earlier id
        if "error" in message:
            fail(1, "ProtocolFailure", line)
        if "result" not in message:
            fail(1, "ProtocolFailure", line)
        return message, line


def finish():
    try:
        child.stdin.close()
    except OSError:
        pass
    try:
        child.wait(timeout=10)
    except subprocess.TimeoutExpired:
        child.terminate()
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait()


try:
    send(
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "stellwerk-gate", "version": "1"},
            },
        }
    )
    read_response(1)
    send({"jsonrpc": "2.0", "method": "notifications/initialized"})
    send(
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "gate_evaluate",
                "arguments": {"task_id": TASK_ID, "required": ["Verified"]},
            },
        }
    )
    message, body = read_response(2)
except (BrokenPipeError, OSError) as exc:
    fail(1, "ProtocolFailure", "server I/O failed: %s" % exc)

result = message["result"]
ok = result.get("ok") if isinstance(result, dict) else None
finish()

if ok is True:
    sys.exit(0)

sys.stdout.write(body + "\n")
sys.exit(1)
PY
