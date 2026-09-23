

Три падающих теста: `from_toml_str_parses_fixture`, `load_reads_file`, `tokens_for_formula`. Делаю минимальную реализацию трёх функций.



Все 8 тестов зелёные. Проверю build и clippy.




Все три acceptance-критерия выполнены.

STATUS: DONE
FILES: crates/stellwerk-registry/src/model.rs
TEST: cargo test -p stellwerk-registry model
EVIDENCE: GREEN — 8 passed; 0 failed; 0 ignored; cargo build и clippy чисты.

---
context_input (input+cacheR+cacheW): 8349
total tokens (in+out+cache):         8527
