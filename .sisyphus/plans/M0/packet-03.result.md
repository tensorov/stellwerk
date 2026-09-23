

Now update `lib.rs` to expose the new module:



Now let's verify it compiles and the tests fail (RED):



The file doesn't actually have `pub mod model;` — it was just a comment. The instruction says to add `pub mod cli;` without removing model, but since model doesn't exist, I should only add cli:



Let me also check if there's a warning-as-error in the workspace lints:



That `all = "deny"` means warnings will be errors. Let me fix the unused variable:



Компилируется. Теперь проверю RED — тесты должны падать:



RED-пакет готов: компилируется, все 8 тестов падают с `unwrap()` на заглушке-Err.

ACCEPTANCE CHECK:
- `cargo build -p stellwerk-registry` ✅ exit 0
- `cargo test -p stellwerk-registry cli --no-run` ✅ (соберётся)
- `! cargo test -p stellwerk-registry cli` ✅ (тесты падают, `!` даёт exit 0)

STATUS: DONE
FILES: [crates/stellwerk-registry/src/cli.rs, crates/stellwerk-registry/src/lib.rs]
TEST: cargo test -p stellwerk-registry cli
EVIDENCE: 8 failed: parse_add_mcp_git, parse_add_mcp_cargo, parse_add_skill_npm, parse_add_no_autoload, parse_measure_with_name, parse_measure_no_name, parse_audit, parse_update — all unwrap() on Err("RED: not implemented")

---
context_input (input+cacheR+cacheW): 12276
total tokens (in+out+cache):         12648
