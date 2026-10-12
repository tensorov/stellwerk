#!/bin/sh
# Fixture-агент для N1-05: печатает РОВНО те Pi-события, по которым раннер
# измеряет годность прогона. Это тестовый вход, а не имитация успеха: он
# существует, чтобы подсчёт `Progress`/`verdict_for` был доказуем.
#
# События и их доноры (runner.rs):
#   tool_execution_start → шаг инструмента (порог GATE_REQUIRE_TOOL_STEPS);
#   message_end          → ход (порог GATE_REQUIRE_TURNS: ходов должно быть > 1);
#   turn_end.message.stopReason == "stop" → чистая остановка (GATE_REQUIRE_STOP_OK).
cat <<'PI_STREAM'
{"type":"session","version":3,"id":"n1-05-fixture","timestamp":"2026-10-12T00:00:00Z"}
{"type":"tool_execution_start","toolCallId":"fixture-tool-1","toolName":"bash"}
{"type":"message_end","message":{"role":"assistant","usage":{"input":100,"output":10,"cacheRead":0,"cacheWrite":0},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":50,"output":5,"cacheRead":0,"cacheWrite":0},"stopReason":"stop"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":50,"output":5,"cacheRead":0,"cacheWrite":0},"stopReason":"stop"}}
PI_STREAM
