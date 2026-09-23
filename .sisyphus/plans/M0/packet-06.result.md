

Now I have all the context. Let me create cmd_measure.rs and update lib.rs.



Now I'll create `cmd_measure.rs` with all types, stubs, and tests:



Wait — the tests that depend on `NotImplemented` will panic because they call `.expect("...")`. I need to fix the tests so they don't panic on `NotImplemented` for RED. Actually, re-reading the requirements:

> RED: `Err(NotImplemented)` — и тесты падают (RED).
> "DO NOT fix/red-green the test inside a RED packet."

The tests that `.expect()` will panic → `cargo test` exits non-zero → RED. That's correct. But tests like `upsert_lock_basic` and others call `.expect(...)` on `Err(NotImplemented)` which panics — those fail (RED). Good.

But `fixture_parse_schema_strings` also calls `.expect("must parse")` on the parse function that returns `Err(NotImplemented)` — that panics too. Same for `schema_bytes` test. Good, those are RED.

Let me build and test:



Build succeeded. Now verify RED:



Need to add `PartialEq` to `LockModule`:



**RED confirmed.** 6 tests fail (parse stub, schema_bytes stub, pinning via stub, upsert_lock 3 tests), 6 pass (invalid JSON, missing tools, garbage upsert, lockfile roundtrip via real toml, cli parses).



STATUS: DONE
FILES: crates/stellwerk-registry/src/lib.rs, crates/stellwerk-registry/src/cmd_measure.rs
TEST: cargo test -p stellwerk-registry measure
EVIDENCE: RED — 6 fail (fixture_parse_schema_strings, fixture_schema_bytes, pinning_813_roundtrip, upsert_lock_basic, upsert_lock_two_modules_sorted, upsert_lock_update_existing_no_dup), 6 pass (invalid_json, missing_result_tools, lockfile_roundtrip, upsert_lock_on_garbage, cli::tests::parse_measure_*). Acceptance wrapper exits 0.

---
context_input (input+cacheR+cacheW): 17876
total tokens (in+out+cache):         18256
