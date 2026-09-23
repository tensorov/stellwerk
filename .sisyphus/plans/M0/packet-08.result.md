

Now I'll create `cmd_audit.rs` with the trait, types, stubs, and RED tests:



Now I need to fix the tests. The `#[ignore]` tests and the smoke test logic need revision — all tests should fail in RED (by calling stubs that return `Err`) without needing `#[ignore]`:



Let me fix the FakeProbe's `call_tool` implementation — the drain logic is broken:



Now let me build and verify:



Let me clean up the unused imports:



The unused imports are at module level but only used in `#[cfg(test)]` — standard rust behavior. I'll allow them:



`#[allow]` is forbidden in the plan. Let me move the test-only imports inside the test module:



Now add the test-only imports inside the `mod tests`:



Clean build. Now let's verify the acceptance criteria:



I need to fix: missing `Cell` import in tests, type ambiguity, and borrow-after-move. Let me rewrite the whole test block:



`Cell` requires `Copy` for `get()`. Let me fix by storing outcomes in a plain `Vec` and using `Cell<usize>` for the index:



Все 8 тестов падают на `Err(NotImplemented)` — RED статус. Единственный passing — `cli::tests::parse_audit`, который из соседнего модуля.

Проверю acceptance-команду:



Статус достигнут. Все 8 тестов падают на `Err(NotImplemented)`, сборка чистая.



Все acceptance-критерии выполнены. Проверю финальную сводку:



8 тестов, 0 `#[ignore]`, все падают через `Err(NotImplemented)`.



STATUS: DONE
FILES: crates/stellwerk-registry/Cargo.toml, crates/stellwerk-registry/src/lib.rs, crates/stellwerk-registry/src/cmd_audit.rs, Cargo.lock
TEST: cargo test -p stellwerk-registry audit
EVIDENCE: `cargo test -p stellwerk-registry audit` — 8/8 cmd_audit tests fail with `Err(AuditError::NotImplemented)`: test1 (exact match), test2 (missing_c), test3 (extra_d), test4 (timeout/malformed), test5 (jsonrpc_error), test6 (skill_skipped), test7 (timeout_propagated), test8 (vacuous_pass). No `#[ignore]`, no async, no real transport. Compile clean.

---
context_input (input+cacheR+cacheW): 25872
total tokens (in+out+cache):         26287
