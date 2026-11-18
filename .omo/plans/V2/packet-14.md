---
id: V2/14
task_id: V2-14
milestone: V2
status: proposed
depends_on: [N2/00h3b, N2/00h3c, N2/00h3d, N2/00j1, V1/a02, V1/a02b, V1/c01, V1/c02, V1/r01]
files_allowed: [crates/stellwerk-cli/src/build_runner.rs, crates/stellwerk-cli/tests/candidate.rs]
max_files: 3
max_diff_lines: 200
acceptance: cargo test -p stellwerk-cli --test candidate
overlap_waiver: C9 — V1/a02, V1/a02b, V1/c01, V1/c02, V1/r01, N2/00h3b, N2/00h3c, N2/00h3d, N2/00j1 — общие файлы с более ранними milestone, строго последовательное исполнение в одном worktree
---

# V2/14 — F-14: make deletion/symlink candidates Dirty and lock them

## Цель
Цель: V2-14 F-14: make deletion/symlink candidates Dirty and lock them

## Шаг 1 — падающий тест
RED: тест `symlink_target_change_is_dirty` падает до symlink-ветки; `deleted_base_file_is_refused` остаётся как есть.

## Что делать
make symlink entries visible to the candidate check without following them. In `workspace_files`/`collect_files` (`crates/stellwerk-cli/src/build_runner.rs:128-160`), add a symlink branch that inserts the relative path into the map with `ContentDigest::of(link_target.as_os_str().as_bytes())` from `std::fs::read_link(entry.path())?` (import `std::os::unix::ffi::OsStrExt`), and never read the target file contents; update the `workspace_files`/`WorkspaceSnapshot`/`snapshot_workspace` comments to say a symlink is represented by the digest of its link target path, so a change to that path or removal of the link becomes `Dirty`, not `Removed`. Leave the existing `deleted_base_file_is_refused` test as-is: it already asserts the exact deleted path (`crates/stellwerk-cli/tests/candidate.rs:136-151`). Add one `symlink_target_change_is_dirty` test that first writes `base/target-a.txt` and `base/target-b.txt` (so `copy_tree` can follow the original link), creates `base/link -> target-a.txt` with `std::os::unix::fs::symlink`, copies the tree, removes the copied `work/link`, recreates it as `link -> target-b.txt`, and asserts `CandidateVerdict::Dirty` contains `link`; do not duplicate the deletion test. Must NOT add a `Removed` variant. Must NOT treat deletion or a symlink target change as `Clean`. Must NOT follow a symlink while snapshotting or accepting.

## Приёмка
`cargo test -p stellwerk-cli --test candidate`; `deleted_base_file_is_refused` asserts the exact deleted path; `symlink_target_change_is_dirty` passes; `rg -n "enum CandidateVerdict" -A 8 crates/stellwerk-cli/src/build_runner.rs` shows no `Removed` arm (or `! rg -n "Removed\\s*[,}]" crates/stellwerk-cli/src/build_runner.rs` succeeds).

## QA
happy - the new symlink test and the enum-shape check; failure - a deleted file or a changed symlink target reported as `Clean` must fail. Evidence `.omo/evidence/task-18-stellwerk-production-v2.md`.

## Запрещено
- do not duplicate the deletion test.
- Must NOT add a `Removed` variant.
- Must NOT treat deletion or a symlink target change as `Clean`.
- Must NOT follow a symlink while snapshotting or accepting.

## Ссылки
`crates/stellwerk-cli/src/build_runner.rs:128-230` (`CandidateFile`, `snapshot_workspace`, `accept_candidate`); `crates/stellwerk-cli/tests/candidate.rs`; `AGENTS.md` law 6.
