# Evidence — V2/14 (F-14): symlink target change is Dirty

Packet: `V2/packet-14` — "make deletion/symlink candidates Dirty and lock them".
Branch: `main`, base HEAD `1cec241` (`chore(evidence): attest V2/packet-13 after merge`).

## RED (test written first, before the src fix)

Command: `cargo test -p stellwerk-cli --test candidate symlink_target_change_is_dirty`

```
   Compiling stellwerk-cli v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-cli)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.07s
     Running tests/candidate.rs (target/debug/deps/candidate-225573b8971480fc)

running 1 test
test symlink_target_change_is_dirty ... FAILED

failures:

---- symlink_target_change_is_dirty stdout ----

thread 'symlink_target_change_is_dirty' (544264) panicked at crates/stellwerk-cli/tests/candidate.rs:49:36:
ожидался отказ, получено Clean
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    symlink_target_change_is_dirty

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p stellwerk-cli --test candidate`
```

Failure mode: old `collect_files` handled only `is_dir()`/`is_file()` and silently
dropped symlinks on both sides, so `link -> target-b.txt` was indistinguishable from
"nothing happened" and the verdict was `Clean` (law 6: unknown collapsed to Clean).

## New test (`crates/stellwerk-cli/tests/candidate.rs`)

```rust
/// Смена цели симлинка — это правка, а не «ничего не произошло»: снимок хранит digest
/// текста ссылки, поэтому `link -> target-b.txt` отличается от `link -> target-a.txt`.
#[test]
fn symlink_target_change_is_dirty() {
    let root = scratch("symlink");
    let base = root.join("base");
    let work = root.join("work");
    write(&base, "target-a.txt", b"a\n");
    write(&base, "target-b.txt", b"b\n");
    std::os::unix::fs::symlink("target-a.txt", base.join("link")).expect("ссылка базы");
    copy_tree(&base, &work);
    std::fs::remove_file(work.join("link")).expect("удаление копии ссылки");
    std::os::unix::fs::symlink("target-b.txt", work.join("link")).expect("ссылка work");

    let snapshot = snapshot_workspace(&base).expect("снимок базы");
    let verdict = accept_candidate(&work, &snapshot, &[]).expect("проверка кандидата");

    assert_eq!(dirty_files(verdict), vec!["link".to_string()]);

    let _ = std::fs::remove_dir_all(&root);
}
```

`copy_tree` uses `std::fs::copy`, which follows the link, so `target-a.txt` and
`target-b.txt` exist in `base` to make the helper copy succeed; the copied `work/link`
becomes a regular file and is removed, then recreated as `link -> target-b.txt`.
`deleted_base_file_is_refused` was left byte-identical.

## Source diff (`crates/stellwerk-cli/src/build_runner.rs`)

```diff
@@ -11,6 +11,7 @@ use std::collections::{BTreeMap, BTreeSet};
 use std::ffi::OsStr;
 use std::future::Future;
 use std::io;
+use std::os::unix::ffi::OsStrExt;
 use std::path::{Component, Path, PathBuf};
 use std::pin::Pin;
 use std::sync::atomic::{AtomicBool, Ordering};
@@ -130,8 +131,9 @@ pub struct CandidateFile {
     pub digest: String,
 }
 
-/// Все обычные файлы под `root` с digest байтов, кроме `.git`. Симлинки не
-/// раскрываются: чтение «за» них вынесло бы в кандидат содержимое вне workspace.
+/// Все файлы под `root` с digest, кроме `.git`: обычный файл — digest байтов, симлинк —
+/// digest текста его цели. Симлинк не раскрывается: чтение «за» него вынесло бы в
+/// кандидат содержимое вне workspace.
 fn workspace_files(root: &Path) -> io::Result<BTreeMap<PathBuf, ContentDigest>> {
     let mut files = BTreeMap::new();
     collect_files(root, Path::new(""), &mut files)?;
@@ -155,6 +157,11 @@ fn collect_files(
             collect_files(root, &path, files)?;
         } else if kind.is_file() {
             files.insert(path, ContentDigest::of(&std::fs::read(entry.path())?));
+        } else if kind.is_symlink() {
+            // Digest текста цели, а не содержимого за ссылкой: смена цели или удаление
+            // ссылки обязаны стать `Dirty`, а не совпасть с отсутствием.
+            let link_target = std::fs::read_link(entry.path())?;
+            files.insert(path, ContentDigest::of(link_target.as_os_str().as_bytes()));
         }
     }
     Ok(())
@@ -181,13 +188,14 @@ fn changed_files(work: &Path, before: &WorkspaceSnapshot) -> io::Result<Vec<Cand
     Ok(changed)
 }
 
-/// База сравнения кандидата: обычные файлы workspace с digest байтов. Пустой базы по
-/// умолчанию нет: снимок строится только обходом дерева.
+/// База сравнения кандидата: файлы workspace с digest байтов, а для симлинка — с digest
+/// текста его цели. Пустой базы по умолчанию нет: снимок строится только обходом дерева.
 #[derive(Debug, Clone)]
 pub struct WorkspaceSnapshot(BTreeMap<PathBuf, ContentDigest>);
 
 /// Снимает базу с того же места, откуда берётся рабочая копия: `.git` пропускается,
-/// симлинки не раскрываются.
+/// симлинк представлен digest своей цели, а не содержимым за ней. Поэтому смена цели или
+/// удаление ссылки различимы как `Dirty`, а не как «ничего не произошло».
 pub fn snapshot_workspace(root: &Path) -> io::Result<WorkspaceSnapshot> {
     workspace_files(root).map(WorkspaceSnapshot)
 }
```

Only `kind.is_symlink()` was added to `collect_files`; `symlink_metadata` is retained,
`metadata()`/`canonicalize()`/target reads are absent. `CandidateVerdict` keeps exactly
two arms.

## GREEN

`cargo test -p stellwerk-cli --test candidate`

```
running 8 tests
test candidate_paths_outside_the_workspace_are_refused ... ok
test clean_workspace_is_accepted ... ok
test deleted_base_file_is_refused ... ok
test candidate_override_is_accepted ... ok
test dirty_workspace_is_refused_with_file_list ... ok
test digest_mismatch_is_refused ... ok
test symlink_target_change_is_dirty ... ok
test git_and_harness_files_are_not_false_dirt ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`cargo test -p stellwerk-cli` — all result lines ok, exit 0:

```
test result: ok. 3 passed; 0 failed; ...
test result: ok. 0 passed; 0 failed; ...
test result: ok. 7 passed; 0 failed; ...
test result: ok. 17 passed; 0 failed; ...
test result: ok. 8 passed; 0 failed; ...
test result: ok. 5 passed; 0 failed; ...
test result: ok. 5 passed; 0 failed; ...
test result: ok. 4 passed; 0 failed; ...
test result: ok. 1 passed; 0 failed; ...
test result: ok. 41 passed; 0 failed; ...
test result: ok. 1 passed; 0 failed; ...
test result: ok. 7 passed; 0 failed; ...
test result: ok. 13 passed; 0 failed; ...
test result: ok. 7 passed; 0 failed; ...
test result: ok. 7 passed; 0 failed; ...
test result: ok. 1 passed; 0 failed; ...
test result: ok. 2 passed; 0 failed; ...
test result: ok. 1 passed; 0 failed; ...
test result: ok. 1 passed; 0 failed; ...
test result: ok. 0 passed; 0 failed; ...
EXIT=0
```

`cargo clippy -p stellwerk-cli --all-targets -- -D warnings` -> `CLIPPY_EXIT=0`
`cargo fmt --all -- --check` -> `FMT_EXIT=0`

## C2 counts (authoritative: `scripts/plan-lint.py::added_counts`)

```
C2 (src, tests) = (4, 22)
```

Limits 200 src / 250 tests — under budget. Raw numstat: `build_runner.rs` 13+/5-,
`candidate.rs` 22+/0-.

## Enum shape

`rg -n "enum CandidateVerdict" -A 5 crates/stellwerk-cli/src/build_runner.rs`

```
213:pub enum CandidateVerdict {
214-    Clean,
215-    Dirty(DirtyWorkspace),
216-}
```

No `Removed` variant.

## PolyForge chain (V2/packet-14)

- Product commit: `d2d1ce6455eaf461693247244ba9fbe7a8fd050b` (`fix(cli): treat symlink target changes as dirty`), 2 files (`build_runner.rs` 13+/5-, `candidate.rs` 22+/0-).
- `diff_hash`: `3823f633fb031693d14eb4537497302b3f4c2d7132d1ee853b88ba236a016bfb`
- `evidence_append(ModelClaim)`: entry 410, hash `6e498ab27cfbcb82ce3c2d4b787793fff1e321319d0e7c510e2041985f2a428c`, state `ModelClaimed`.
- `evidence_verify(cargo test -p stellwerk-cli --test candidate)`: entry 411, exit 0, state `Verified`, stdout_hash `c52fb7ae84213cf41a25d5de38f9af8eb137273a95c65661b41bd2c6b7aef273`, hash `430a4264dad463ffa2b9b2575aa5bb6fb6a0896413c8888e57b4d90dd3d44914`.
- `gate_evaluate(required=[Verified])`: `passed=true`, verified=1, claimed=1, validated=0, missing=[].
- `chain_tail`: `430a4264dad463ffa2b9b2575aa5bb6fb6a0896413c8888e57b4d90dd3d44914`

Orchestrator independent verification: full diff read line by line; `git show --numstat d2d1ce6` -> exactly 2 declared files; `cargo test -p stellwerk-cli --test candidate` -> 8 passed / 0 failed (7 existing + `symlink_target_change_is_dirty`); `cargo test -p stellwerk-cli` -> every result line ok (audit_live 7, build_runner 17, candidate 8, …); `cargo clippy -p stellwerk-cli --all-targets -- -D warnings` exit 0; `cargo fmt --all -- --check` exit 0; C2 recomputed by the orchestrator with `scripts/plan-lint.py::added_counts` on `git diff d2d1ce6^1 d2d1ce6 --unified=0` -> src 4 / tests 22, `budget_violations` `[]`; `rg "enum CandidateVerdict" -A 6` shows only `Clean`/`Dirty`; `rg "allow\(|#\[ignore\]"` over both files exit 1.
