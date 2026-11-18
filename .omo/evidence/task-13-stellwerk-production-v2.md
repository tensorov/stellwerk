# Evidence — V2-09 (F-10): mark ADR-0015 superseded by ADR-0021

- Task packet: `.sisyphus/plans/V2/packet-09.md` (task_id V2-09)
- Commit: `b99cc31c618ad7599e70252eeaa6c716af1a3fb2` — `docs(adr): mark 0015 superseded by 0021`
- Files in commit: exactly one, `docs/adr/0015-execution-spec-not-task-spec.md` (1 insertion, 1 deletion)

## RED (before edit)

```
$ rg -n '^Статус: отменён \(superseded by ADR-0021\)$' docs/adr/0015-execution-spec-not-task-spec.md
EXIT=1
```
No match (non-zero exit). Current line 3 at that point:

```
$ rg -n '^Статус: принят$' docs/adr/0015-execution-spec-not-task-spec.md
3:Статус: принят
EXIT2=0
```

## GREEN (after edit)

Acceptance match, exactly line 3:

```
$ rg -n '^Статус: отменён \(superseded by ADR-0021\)$' docs/adr/0015-execution-spec-not-task-spec.md
3:Статус: отменён (superseded by ADR-0021)
EXIT=0
```

Old status must no longer match:

```
$ rg -n 'Статус: принят' docs/adr/0015-execution-spec-not-task-spec.md
EXIT=1
```

Exit codes: acceptance `rg` = 0; negative `rg` = 1.

## git diff of the commit

```
$ git diff -- docs/adr/0015-execution-spec-not-task-spec.md
diff --git a/docs/adr/0015-execution-spec-not-task-spec.md b/docs/adr/0015-execution-spec-not-task-spec.md
index f02df37..e81b2e1 100644
--- a/docs/adr/0015-execution-spec-not-task-spec.md
+++ b/docs/adr/0015-execution-spec-not-task-spec.md
@@ -1,6 +1,6 @@
 # ADR-0015: программа агента — отдельная `ExecutionSpec`, а не поле `TaskSpec`
 
-Статус: принят
+Статус: отменён (superseded by ADR-0021)
 Дата: 2026-10-12
 Решение владельца: 2026-10-11, п. 1 — `TaskSpec` **не расширяется**;
   вводится отдельная явная `ExecutionSpec` на стороне orchestration; CLI выбирает
```

## git show --numstat

```
$ git show --numstat --format='%H%n%s' HEAD
b99cc31c618ad7599e70252eeaa6c716af1a3fb2
docs(adr): mark 0015 superseded by 0021

1	1	docs/adr/0015-execution-spec-not-task-spec.md
```

1 file changed, 1 insertion(+), 1 deletion(-).

## ADR index note

There is no `docs/adr/README.md` (nor any other ADR index file) in this repo
(`ls docs/adr/` lists only `NNNN-*.md` ADRs). Per the packet ("add a superseding
link if the ADR index has one"), no link line was added and no index was created.
The cross-reference already exists from the superseding side:
`docs/adr/0021-single-execution-profile.md` carries
`Отменяет: ADR-0015 (docs/adr/0015-execution-spec-not-task-spec.md)`.
`docs/adr/0021-single-execution-profile.md` was not touched.

## PolyForge record and attestation boundary (declared, not hidden)

- `evidence_append(ModelClaim, task_id=V2/packet-09)` -> seq 399, hash
  `430cd89fe4eab1546a32e43ade64417776ba2dea7d4ed72ae6209e1035607cb7`,
  commit `b99cc31c618ad7599e70252eeaa6c716af1a3fb2`, diff_hash
  `5d0ba18deab18bb962df6ee328a20c6f88775110eda82111f1ee04558fd91683`.
- `gate_evaluate(required=["Verified"])` -> **passed=false**, `missing=["Verified"]`,
  chain tail `430cd89fe4eab1546a32e43ade64417776ba2dea7d4ed72ae6209e1035607cb7`.

Why no `Verified` is reachable here, stated plainly rather than worked around:

1. The packet's acceptance is `rg -n ... docs/adr/0015-...md`; `rg` is not on the
   PolyForge allowlist (`MCP error -32602: tool "rg" is not on the allowlist`).
2. No test in this workspace asserts ADR status lines, so no `cargo test` target
   exercises this artifact.
3. ADR-0006, section "Почему подстановка родственной команды запрещена", forbids
   substituting a related command to manufacture a green attestation. Non-allowlisted
   `python3` was tried by the orchestrator first and rejected; no substitute was used.

Precedent: docs-only ADR commits `b2d5b24e` (ADR-0006) and `0f218cc2` (ADR-0012)
carry no `ToolAttestation` either (`git log --name-only` + ledger cross-check).

Consequence: V2/09 is `ModelClaimed` with a declared boundary, NOT `Verified`.
The implementation itself is complete and checked by the two `rg` commands above,
whose exact exit codes (0 and 1) are recorded in this file.
