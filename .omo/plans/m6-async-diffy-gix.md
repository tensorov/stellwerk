# m6-async-diffy-gix — Work Plan

## TL;DR (For humans)

**What you'll get:** M6 gets an object-safe async orchestration layer, a shared 12-slot run queue, in-memory diff/patch handling with explicit add/modify/delete, and gix-only worktree/head/status/commit/apply paths. The existing bwrap/opencode classic mode remains the default and keeps its PolyForge evidence chain.

**Why this approach:** The base M6 is a sync single-run state machine. This plan changes only the execution boundary: ports become object-safe boxed futures, one shared `AutoQueue` enforces 12 in-flight runs and one active VM, and the sync `lifecycle::advance` stays on `spawn_blocking`. gix owns repository mutation; PolyForge keeps its own `git diff-tree` for `change.diff`, so the no-host-git invariant is scoped honestly to Stellwerk code.

**What it will NOT do:** No direct Firecracker/vsock/Qwen work in this plan; that is a separate, explicitly blocked plan (`.omo/plans/m6b-firecracker-qwen.md`). No auto-merge, no 12 VMs, no host `git` in Stellwerk worktree/apply/evidence, no `#[allow]`/`#[ignore]`, no skipped tests, no weakening of PolyForge evidence.

**Effort:** L
**Risk:** High — base M6 is not present in this checkout; gix index/commit behavior is load-bearing; C2 needs a formal lockfile rule.
**Decisions to sanity-check:** classic mode stays default; async ports are boxed futures, not AFIT; one shared queue with 12 total slots and 1 active VM; `change.diff` remains PolyForge-generated.

Your next move: start work with `$start-work m6-async-diffy-gix` only after the preconditions below pass. Full execution detail follows.

---

> TL;DR (machine): L / high risk / contract-change C2-lockfile + M6 packet revision + gix worktree/head/status/commit/apply + diffy add-modify-delete/atomic apply + object-safe async ports + shared 12-slot queue + classic mode + C2-enforced packets. Direct mode is a separate plan.

## Scope

### Must have
- Hard precondition P0: the base plan's Wave 2-4 (todos 9-27) exists. After the base merge, its M6 packet files are archived under `.sisyphus/plans/M6/_archive/base/` by todo 1; the 12 revision packets below are then the canonical M6 set. If the base code is absent, stop with `BLOCKED: base M6 foundation missing`; do not add dependencies or create a parallel foundation.
- Canonical packet set and a populated `.omo/plans/M6/` mirror, created **before** any product code.
- A formal C2 rule for generated `Cargo.lock`: lockfile lines are dependency metadata, not product source; the rule is a contract-change + ADR, not a silent waiver.
- `diffy`, `gix`, and tokio `sync`/`net` added through contract-change packets.
- gix-only worktree creation/removal, branch creation, HEAD check, clean-status check, index update, and commit with trailers.
- diffy patch model with explicit `Add`/`Modify`/`Delete`, baseline and result hashes, and atomic multi-file apply with rollback.
- Object-safe async ports using `Pin<Box<dyn Future + Send + '_>>`; no `async-trait`, no AFIT, no `#[allow(async_fn_in_trait)]`.
- Shared `AutoQueue`: 12 total in-flight runs, `max_active_vms` default 1, hard ceiling 12, 13th enqueue `QueueError::Full` (mapped to `RunError::QueueFull`), cancellation `Aborted`, budget expiry `BudgetExceeded`.
- Classic mode preserved and adapted to the async ports; CLI `main` stays sync with one `Runtime::block_on`.
- PolyForge evidence per packet: merge -> `evidence_verify` -> `gate_evaluate(required=["Verified"])` -> separate evidence commit.

### Must NOT have
- No Firecracker, vsock, Qwen, `socat`, or direct-mode code in this plan; those belong to `m6b-firecracker-qwen.md`.
- No new dependency beyond `diffy`, `gix`, and tokio `sync`/`net`; no `cargo add`, no `git2`, no `libgit2`, no `async-trait`, no `tokio-vsock`.
- No host `git` subprocess in Stellwerk worktree/apply/evidence; PolyForge may use its own `git diff-tree` for `change.diff`, and that is outside the Stellwerk invariant.
- No claim that `change.diff` is gix-generated; gix 0.90 has no byte-identical `git diff-tree` formatter.
- No 12 concurrent VMs; the queue limit is total runs, not a license for 12 active workers.
- No auto-merge, no `opencode --auto`, no `Ask`-as-allow, no `#[allow]`/`#[ignore]`/skipped tests.
- No edits to `crates/stellwerk-core/src/contract.rs`, `modules.lock`, or `.github/workflows/`; root `Cargo.toml` only in the dependency packet.
- No expansion beyond C2; adjacent problems become `FINDING`.

## Preconditions

**P0 — base plan Wave 2-4.** This plan is a revision layer over `.omo/plans/stellwerk-attested-autonomy.md`. Execution may start only when all of these exist and the base plan's Wave 2-4 (todos 9-27) is complete and committed. The base M6 packet files are archived under `.sisyphus/plans/M6/_archive/base/` by todo 1; this plan's 12 packets are then the active M6 set.
- `crates/stellwerk-task/src/auto/mod.rs`
- `crates/stellwerk-task/src/auto/model.rs`
- `crates/stellwerk-task/src/auto/worker.rs`
- `crates/stellwerk-task/src/auto/config.rs`
- `crates/stellwerk-task/src/auto/bridge_wire.rs`
- `crates/stellwerk-task/src/auto/ports.rs`
- `crates/stellwerk-task/src/auto/run.rs`
- `crates/stellwerk-task/src/auto/worktree.rs`
- `crates/stellwerk-task/src/auto/manifest.rs`
- `crates/stellwerk-task/src/auto/apply.rs`
- `crates/stellwerk-task/src/auto/evidence.rs`
- `crates/stellwerk-task/src/auto/checks.rs`
- `crates/stellwerk-task/src/auto/bundle.rs`
- `crates/stellwerk-task/src/auto/accept.rs`
- `crates/stellwerk-cli/src/main.rs`
- `crates/stellwerk-cli/src/auto_planner.rs`
- `crates/stellwerk-cli/src/plan.rs`
- `crates/stellwerk-cli/src/planner_host.rs`

The plan intentionally replaces the base versions of `worktree.rs`, `manifest.rs`, `apply.rs`, `ports.rs`, `run.rs`, and the CLI 23a/23b surface; their presence is proof that the base orchestration exists, not a claim that they already satisfy this plan. If any is absent, todo 1 records `BLOCKED: base M6 foundation missing` and stops. No dependency commit, no packet execution, no workaround.

**P1 — checkout.** The working tree must be clean except for this plan/draft and known pre-existing unrelated changes. `git status --short` is recorded before every packet.

## Verification strategy
> Every check is agent-executed. No manual “looks good” step.

- **TDD:** every Rust path gets a failing test first, then the implementation. A test-only RED commit is allowed only for a split todo and is followed immediately by the implementation commit.
- **Commands:** `cargo test --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `python3 scripts/plan-lint.py --self-test`, `python3 scripts/plan-lint.py`, and scoped `python3 scripts/plan-lint.py .sisyphus/plans/M6`.
- **Non-vacuous selection:** every acceptance command names `--test <target>` or `--lib`; before treating it as green, run the same command with `-- --list` and assert at least one test. A zero-test selection is a failure, never a pass.
- **Evidence:** each packet gets `.omo/evidence/task-<N>-m6-async-diffy-gix.log`/`.json` with command, exit code, stdout/stderr hashes, and artifact path. The attestation is PolyForge, not the log.
- **PolyForge:** per packet, after merge to `main`, run the exact test command on `main`, then `evidence_append(ModelClaim)`, `evidence_verify(tool_name="cargo test", args=[...])`, `gate_evaluate(task_id, required=["Verified"])`, then commit `chore(evidence): attest <packet> after merge` with the chain tail. If the allowlist rejects `cargo test`, record `BLOCKED`; do not substitute an unrelated command.
- **No-git sentinel:** set `PATH` to a temp dir with a `git` executable that writes a marker; run Stellwerk worktree/apply/evidence tests; assert the marker is absent. The sentinel is scoped to Stellwerk production code; PolyForge is a separate process.
- **`change.diff`:** produced by PolyForge’s `git diff-tree --root --no-commit-id --no-ext-diff --binary -p`; Stellwerk tests assert it is not generated by Stellwerk code. No gix byte-parity gate.
- **C2:** measured by `plan-lint --budget <packet-merge-commit>`; `Cargo.lock` is excluded by the rule from packet 00a. No `max_diff_lines` substitute.

## Execution strategy
### Parallel execution waves
> Planning is parallel; **execution is strictly sequential in one worktree**. Wave labels group related work, not concurrent writers.

- **Wave 0 — plan artifacts and contract (3 packets):** C2-lockfile rule, dependency manifests, ADR.
- **Wave 1 — gix + diffy (5 packets):** worktree, head/status/commit, patch model, apply, evidence.
- **Wave 2 — async (3 packets):** object-safe ports, shared queue, async run state machine.
- **Wave 3 — classic wiring (1 packet):** CLI and planner/worker adapters.
- **Wave 4 — proof (F1-F4).**

### Dependency matrix
| Todo | Depends on | Blocks | Can parallelize with |
| --- | --- | --- | --- |
| 1 | P0 | 2-12 | - |
| 2 | 1 | 3 | - |
| 3 | 2 | 4,5,6 | - |
| 4 | 3 | 5,6,8 | - |
| 5 | 3,4 | 6,7,8 | - |
| 6 | 3,4,5 | 7,9 | - |
| 7 | 4,5,6 | 9 | - |
| 8 | 4,5,7 | 9 | - |
| 9 | 7,8 | 10,11 | - |
| 10 | 9 | 11,12 | - |
| 11 | 10 | 12 | - |
| 12 | 11 | F1-F4 | - |

## Packet set

Canonical packets live in `.sisyphus/plans/M6/`; the mirror is generated into `.omo/plans/M6/` and must be byte-identical.

| Packet | Files (max 3) | Purpose |
| --- | --- | --- |
| `packet-00a` | `scripts/plan-lint.py`, `docs/adr/0023-c2-lockfile.md` | Exclude generated `Cargo.lock` from C2 product-source count. |
| `packet-00b` | `Cargo.toml`, `Cargo.lock` | Add `diffy`, `gix`, tokio `sync`/`net`. |
| `packet-00c` | `crates/stellwerk-task/Cargo.toml`, `docs/adr/0022-async-diffy-gix.md` | Wire task dependencies and record the architecture. |
| `packet-14a` | `crates/stellwerk-task/src/auto/worktree.rs`, `crates/stellwerk-task/src/auto/mod.rs`, `crates/stellwerk-task/tests/auto_worktree.rs` | gix worktree create/remove. |
| `packet-14b` | `crates/stellwerk-task/src/auto/git.rs`, `crates/stellwerk-task/src/auto/mod.rs`, `crates/stellwerk-task/tests/auto_git.rs` | gix head/status/index/commit. |
| `packet-15a` | `crates/stellwerk-task/src/auto/manifest.rs`, `crates/stellwerk-task/src/auto/mod.rs`, `crates/stellwerk-task/tests/auto_manifest.rs` | Add/modify/delete patch model and validation. |
| `packet-16a` | `crates/stellwerk-task/src/auto/apply.rs`, `crates/stellwerk-task/src/auto/mod.rs`, `crates/stellwerk-task/tests/auto_apply.rs` | Atomic apply + gix commit. |
| `packet-19a` | `crates/stellwerk-task/src/evidence.rs`, `crates/stellwerk-task/src/auto/mod.rs`, `crates/stellwerk-task/tests/evidence.rs` | gix head/status helpers; keep PolyForge client. |
| `packet-21a` | `crates/stellwerk-task/src/auto/ports.rs`, `crates/stellwerk-task/src/auto/mod.rs`, `crates/stellwerk-task/tests/auto_ports.rs` | Object-safe boxed-future ports + runtime bridge. |
| `packet-21b1` | `crates/stellwerk-task/src/auto/queue.rs`, `crates/stellwerk-task/src/auto/mod.rs`, `crates/stellwerk-task/tests/auto_queue.rs` | Shared 12-slot queue and active-VM permit. |
| `packet-21b2` | `crates/stellwerk-task/src/auto/run.rs`, `crates/stellwerk-task/src/auto/mod.rs`, `crates/stellwerk-task/tests/auto_run.rs` | Async state machine, cancellation, lifecycle bridge. |
| `packet-23c` | `crates/stellwerk-cli/src/main.rs`, `crates/stellwerk-cli/src/auto_planner.rs`, `crates/stellwerk-cli/src/plan.rs` | CLI, boxed-future planner adapters, queue construction. |

`depends_on` chain: `00a -> 00b -> 00c -> 14a -> 14b -> 15a -> 16a -> 19a -> 21a -> 21b1 -> 21b2 -> 23c`; `15a` also depends on `00c`; `19a` depends on `14a,14b,16a,N1-07`; `21a` depends on `16a,19a`; `23c` depends on `21b2`. Frontmatter `depends_on` uses these short IDs; `norm_dep` resolves them to the canonical `packet-<id>.md` stems.
Cross-milestone `depends_on`: `00b` includes `N1-01`, `TUI-01`, `N2-00g1`; `00c` includes `N2-00g1` and `N1-08`; `19a` includes `N1-07` because it touches `crates/stellwerk-task/src/evidence.rs` and `crates/stellwerk-task/tests/evidence.rs`; `23c` includes `N1-01`, `N1-10`, `N2-00h2b`, `N2-00h3a`, `N2-00h3d`, `N2-00h3e`, `TUI-02`, `TUI-03`. This is the C9 ordering declaration; no `overlap_waiver` is used. `00a` touches `scripts/plan-lint.py`, which C9 does not classify as a `crates/` overlap; H1-01/02/03 remain the preceding plan-lint owners and are listed in the packet body for sequencing.

## Todos

- [ ] 1. Preflight, canonical packets, and mirror
  What to do / Must NOT do: Verify P0 and P1. If base M6 is missing, write `BLOCKED: base M6 foundation missing` and stop; do not create packets or dependencies. Otherwise archive the base M6 packet files under `.sisyphus/plans/M6/_archive/base/` (and the mirror copy), then create the 12 canonical packet files above with frontmatter `packet`, `milestone: M6`, `depends_on` (short IDs as listed), `files_allowed`, `max_diff_lines`, `acceptance`, `qa`, `commit`, and `contract-change` on `00a`, `00b`, `00c` where required; each body names exact behavior, tests, and C2 split rule. Rewrite the base plan’s M6 todos 14/15/16/19/21/23 to point at these packets; keep the master plan exemption documented: `.omo/plans/stellwerk-attested-autonomy.md` is the master plan, not a packet mirror. Regenerate `.omo/plans/M6/` from `.sisyphus/plans/M6/` with a byte-identical copy; do not hand-edit the mirror. Do not write product code; do not add dependencies; do not change PolyForge todos 1-3.
  Parallelization: Wave 0 | Blocked by: P0 | Blocks: 2-12
  References: `AGENTS.md` (canonical source, C2, C8, C9); `.omo/plans/stellwerk-attested-autonomy.md:106` (Atlas gate); `scripts/plan-lint.py:596-650` (C8).
  Acceptance criteria: `test -f .sisyphus/plans/M6/packet-00a.md` through `packet-23c.md`; `find .sisyphus/plans/M6 -name 'packet-*.md' -not -path '*_archive*' | wc -l` is 12; `find .omo/plans/M6 -name 'packet-*.md' -not -path '*_archive*' | wc -l` is 12; the base set exists under `.sisyphus/plans/M6/_archive/base/`; `diff -r .sisyphus/plans/M6 .omo/plans/M6` is empty (ignore `_archive` if present); `python3 scripts/plan-lint.py .sisyphus/plans/M6` has no C1/C3/C4; `python3 scripts/plan-lint.py` reports C8 clean and no new M6 C9; `git diff --check` clean.
  QA scenarios: happy — mirror equals canonical; failure — add a mirror-only packet and assert C8 fails, then remove it.
  Commit: Y | `docs(m6): add async/gix/diffy packet set and mirror`

- [ ] 2. C2 lockfile rule
  What to do / Must NOT do: In `scripts/plan-lint.py`, add `is_generated_lockfile(path)` for `Cargo.lock` and skip it in `added_counts`, exactly as `.sisyphus/plans` is skipped. Add a self-test proving `Cargo.lock` additions do not consume the src budget while `src/*.rs` still does. Add `docs/adr/0023-c2-lockfile.md`: C2 measures product source and test lines; generated dependency metadata is not product code; this is not a lint suppression and does not change the 200/250 product limits. This is a `contract-change` packet. Do not add a generic waiver field; do not raise the 200/250 limits; do not touch `AGENTS.md` in this packet.
  Parallelization: Wave 0 | Blocked by: 1 | Blocks: 3
  References: `AGENTS.md` (C2, ADR-0003); `scripts/plan-lint.py:667-706` (`added_counts`, `measure_commit`); `docs/adr/0003-*.md`.
  Acceptance criteria: `python3 scripts/plan-lint.py --self-test` passes including the new case; a synthetic commit that adds 500 `Cargo.lock` lines and 10 `src` lines reports 10 src lines, not 510; `python3 scripts/plan-lint.py --budget <synthetic>` is clean; `rg -n 'Cargo.lock' scripts/plan-lint.py docs/adr/0023-c2-lockfile.md` shows the rule.
  QA scenarios: happy — lockfile-only commit passes C2; failure — a 201-line `src/*.rs` addition still fails C2.
  Commit: Y | `build(lint): exclude generated lockfile from C2 source budget`

- [ ] 3. Dependency manifests and ADR
  What to do / Must NOT do: `packet-00b` is a `contract-change` packet: it adds `diffy = "=0.5.2"`, `gix = { version = "=0.90.0", default-features = false, features = ["basic", "status", "worktree-mutation", "excludes", "attributes", "sha1"] }`, and `sync`/`net` to workspace tokio; update `Cargo.lock`. `packet-00c` is also `contract-change`; it wires `diffy.workspace = true` and `gix.workspace = true` into `crates/stellwerk-task/Cargo.toml` and records ADR-0022: gix feature choice, object-safe boxed-future ports, shared queue, classic-only scope, C2 lockfile rule, and the fact that direct mode is a separate plan. Do not add any other dependency; do not use `cargo add`; do not change tokio features except adding `sync` and `net`; do not edit forbidden files.
  Parallelization: Wave 0 | Blocked by: 2 | Blocks: 4-6
  References: `Cargo.toml:12-35`; `crates/stellwerk-task/Cargo.toml`; `docs/adr/0020-http-transport.md` (ADR shape); `docs/adr/0023-c2-lockfile.md`.
  Acceptance criteria: `cargo metadata --no-deps` succeeds; `cargo tree -p stellwerk-task -i diffy` and `-i gix` show one pinned version each; `cargo check -p stellwerk-task` succeeds; `python3 scripts/plan-lint.py --budget <00b-merge-commit>` and `<00c-merge-commit>` are clean; `rg` shows the exact entries.
  QA scenarios: happy — `cargo check -p stellwerk-task`; failure — remove `sync` and assert a later `tokio::sync` compile probe fails (todo 9).
  Commit: Y | `build(workspace): add async, diff, and gix dependencies`; `docs(adr): record async diff gix decision`

- [ ] 4. gix worktree create/remove
  What to do / Must NOT do: Replace `auto/worktree.rs` with a gix-only implementation. Build the branch as a full ref: `let full = gix::refs::FullName::try_from(format!("refs/heads/{branch}"))?`; create it with `repo.reference(full.clone(), base_commit, gix::refs::transaction::PreviousValue::MustNotExist, "stellwerk worktree")?`; add with `repo.add_worktree(destination, gix::worktree::add::Head::Attached(full), gix::progress::Discard, &AtomicBool::new(false))?`; on failure delete the branch. Remove with `repo.remove_worktree(destination, gix::worktree::remove::Force::DiscardChanges, gix::progress::Discard)?`; do not override locks. Reject a destination inside the source repo, an existing registered worktree, a missing base commit, and a non-local branch. Keep the existing `Worktree` API shape. Do not call `std::process::Command` for `git`; do not use `git2`.
  Parallelization: Wave 1 | Blocked by: 3 | Blocks: 5,6,8
  References: `gix-0.90.0/src/worktree/add.rs:19-24,103`; `.../worktree/remove.rs:13-20,300`; `.../repository/reference.rs`; base plan `auto/worktree.rs` shape.
  Acceptance criteria: `cargo test -p stellwerk-task --test auto_worktree` green; create/remove leaves `HEAD` at base commit and no registration; existing destination -> `DestinationExists` with no branch; `PATH` git sentinel marker absent.
  QA scenarios: happy — create/remove linked worktree; failure — existing destination, branch cleanup.
  Commit: Y | `feat(stellwerk-task): create and remove gix worktrees`

- [ ] 5. gix head, status, and commit
  What to do / Must NOT do: Add `auto/git.rs`. `head_id` is `repo.head()?.id().map(|id| id.detach()).ok_or(GitError::UnbornHead)`. `is_clean` is `!repo.is_dirty()?` plus a status iterator check that rejects untracked entries. `stage_and_commit` must not use `repo.index()?` (a read-only `SharedFileSnapshot`). Build the commit tree with `repo.edit_tree(parent_tree_id)?` (the gix wrapper over `gix::objs::tree::Editor`): for each Add/Modify, write the worktree bytes as a blob via `repo.write_object(gix::objs::Blob { data })?.detach()` and `editor.upsert(path_str, gix::objs::tree::EntryKind::Blob, blob_id)` where `path_str` is a `&str`/`BString` (never `&Path`/`PathBuf`); for each Delete, `editor.remove(path_str)`; then `let tree_id = editor.write()?` (the wrapper calls `repo.write_object(tree)` for each changed tree). After the tree is written, reset the index from the committed tree with `let tree_oid = tree_id.detach(); let mut index = repo.index_from_tree(tree_oid.as_ref())?; index.write(gix::index::write::Options::default())?;` so the index exactly matches the tree; do not synthesize entries or call a nonexistent `write_tree`/index-to-tree helper. Build `gix::actor::Signature` from policy owner identity and the run clock; then `let mut time = gix::date::parse::TimeBuf::default(); let sig = signature.to_ref(&mut time);` and call `repo.commit_as(sig, sig, full_ref, message, tree_id, parents)`; keep `time` alive through the call. Never read global git config; append `Stellwerk-Run` and `Stellwerk-Policy` trailers. Do not shell out to `git`; do not stage unadmitted files; do not commit if tree write or index write fails.
  Parallelization: Wave 1 | Blocked by: 3,4 | Blocks: 6,7,8
  References: `gix-0.90.0/src/repository/object.rs:19` (`edit_tree`), `:232` (`write_object`), `:334-355` (`commit_as`), `:424-439` (`commit`); `.../object/tree/editor.rs:20-34,145-165,210-250` (`Editor::new`, `upsert`, `remove`, `write`); `.../head/mod.rs:94`; `.../status/mod.rs:140` (`is_dirty`); `.../status/index_worktree.rs:542`.
  Acceptance criteria: `cargo test -p stellwerk-task --test auto_git` green; commit one changed file; `repo.head()?.id()` equals returned id; `repo.head_commit()?.tree_id()?` equals the tree returned by `editor.write()` and contains exactly the admitted paths; both trailers present; second `is_clean` true; unadmitted file absent; `PATH` sentinel absent; unborn HEAD -> `GitError::UnbornHead`.
  QA scenarios: happy — stage/commit one admitted file; failure — unadmitted path -> `PathNotAdmitted`, no commit.
  Commit: Y | `feat(stellwerk-task): add gix head, status, and commit helpers`

- [ ] 6. diffy add/modify/delete model
  What to do / Must NOT do: Replace `auto/manifest.rs` with `FileOperation { Add, Modify, Delete }`; `FilePatch { path, operation, baseline_sha256: Option<[u8;32]>, result_sha256: Option<[u8;32]>, patch: String }`; `ChangeSet { files: Vec<FilePatch> }`. `from_baseline_current(baseline: BTreeMap<PathBuf, Vec<u8>>, current: BTreeMap<PathBuf, Vec<u8>>)` emits Add for a new path, Delete for a removed path, Modify for changed bytes, and no entry for unchanged bytes. `validate(policy, admitted_paths)` rejects binary/non-UTF8, symlinks, mode changes, `.git`, paths outside the worktree, forbidden paths, `max_files`, `max_diff_lines`, and a missing/extra baseline hash. Do not write to disk; do not apply patches here; do not use `similar`.
  Parallelization: Wave 1 | Blocked by: 3,4,5 | Blocks: 7,9
  References: `diffy-0.5.2/src/diff/mod.rs:268` (`create_patch`); base plan manifest/pathspec/policy.
  Acceptance criteria: `cargo test -p stellwerk-task --test auto_manifest` green; add/modify/delete each produce the correct operation and hashes; binary, symlink, `.git`, unadmitted, too-many-lines each return a distinct error; no file written.
  QA scenarios: happy — add/modify/delete round-trip; failure — symlink -> `ManifestError::Symlink`.
  Commit: Y | `feat(stellwerk-task): model add, modify, and delete patches`

- [ ] 7. Atomic diffy apply and gix commit
  What to do / Must NOT do: Replace `auto/apply.rs` with `apply(worktree, changes, policy, admitted_paths) -> Result<Commit, ApplyError>`. For each patch: verify the baseline hash against the worktree bytes, parse `diffy::Patch::from_str`, apply in memory, verify `result_sha256`, and reject drift/conflict/malformed/non-UTF8 before any write. For all files, write temp files in the destination directory and rename; for Delete, remove the file; for Add, create it. If any rename/remove fails, restore every original from saved bytes and return `ApplyError::ChecksFailed`. Then call `git::stage_and_commit` with explicit paths. Do not write a partial result; do not call `git`; do not commit if any file fails.
  Parallelization: Wave 1 | Blocked by: 4,5,6 | Blocks: 9
  References: `diffy-0.5.2/src/apply.rs:96`; `.../patch/mod.rs`; `auto/git.rs` (todo 5).
  Acceptance criteria: `cargo test -p stellwerk-task --test auto_apply` green; valid add/modify/delete commits; wrong baseline hash -> `ChecksFailed` and no write; multi-file conflict writes nothing; injected rename failure restores originals; `PATH` sentinel absent.
  QA scenarios: happy — apply one patch and commit; failure — wrong context -> `ChecksFailed` before write.
  Commit: Y | `feat(stellwerk-task): apply diffy patches atomically and commit with gix`

- [ ] 8. gix evidence head/status
  What to do / Must NOT do: Update `crates/stellwerk-task/src/evidence.rs` (the local helper module, not the PolyForge client `auto/evidence.rs`) with gix-only `evidence_head(repo) -> ObjectId`, `evidence_repo_status(repo) -> StatusSummary`, and `commit_sha_for(repo, reference)`. Use `repo.head()?.id()` with explicit unborn handling, `repo.is_dirty()?` plus untracked check, and `repo.find_reference(...).peel_to_commit()?.id()`. Keep the PolyForge `PfEvidence` client unchanged. This packet also depends on `N1-07`, which owns the current `crates/stellwerk-task/src/evidence.rs`/`crates/stellwerk-task/tests/evidence.rs`; the C9 order is `N1-07 -> 19a`. Do not shell out to `git`; do not write a local ledger; do not change the `Evidence` trait.
  Parallelization: Wave 1 | Blocked by: 4,5,7 | Blocks: 9
  References: base plan evidence client; `crates/stellwerk-task/src/evidence.rs:58-176`; gix `head/mod.rs:94`, `status/mod.rs:140`.
  Acceptance criteria: `cargo test -p stellwerk-task --test evidence` green; head/status after commit; detached/unborn -> explicit error; existing PolyForge argv tests pass; `PATH` sentinel absent.
  QA scenarios: happy — read head/status after commit; failure — unborn HEAD -> `EvidenceError::Head`.
  Commit: Y | `feat(stellwerk-task): read evidence head and status with gix`

- [ ] 9. Object-safe async ports and runtime bridge
  What to do / Must NOT do: Replace `auto/ports.rs` with `pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;`. Define object-safe traits: `Planner::plan<'a>(&'a self, ...) -> BoxFuture<'a, Result<AutoPlan, PlannerError>>`, `Worker::preflight<'a>(&'a self) -> BoxFuture<'a, Result<(), WorkerError>>`, `Worker::execute<'a>(&'a self, plan: &'a AutoPlan, worktree: &'a Path, deadline: Instant, cancel: tokio::sync::watch::Receiver<bool>) -> BoxFuture<'a, Result<WorkerResult, WorkerError>>`, `Checks::run<'a>(...) -> BoxFuture<'a, Result<CheckReport, CheckError>>`, `Evidence::claim/verify/gate` with boxed futures. The cancel receiver is the only cancellation input; `execute` must select on `cancel.changed()`, kill and await its child/VM, and return an error; dropping the boxed future is not teardown. Keep `RunStore` sync and `AutoDeps` as `Box<dyn ...>`. Add `RuntimeBridge::run_blocking<F,T>(f) -> impl Future<Output=Result<T,BridgeError>>` that awaits `spawn_blocking`; `lifecycle::advance` runs through it. Add `run_with_deadline` using one monotonic `Instant`. Do not use AFIT, `async-trait`, `#[allow]`, or `Arc<dyn TaskStore>`; do not change `run.rs` yet.
  Parallelization: Wave 2 | Blocked by: 7,8 | Blocks: 10,11
  References: base plan `ports.rs`; `tokio::task::spawn_blocking`; `tokio::time::timeout`; `AGENTS.md` (no `#[allow]`).
  Acceptance criteria: `cargo test -p stellwerk-task --test auto_ports` green; a `Box<dyn Planner>` accepts a boxed-future impl; a blocking closure runs on a different thread; expired deadline -> `Timeout`; source scan finds no `async fn` in the trait definitions, no `async-trait`, no `Arc<dyn TaskStore>`.
  QA scenarios: happy — boxed planner/worker round-trip; failure — expired deadline -> `Timeout`.
  Commit: Y | `refactor(stellwerk-task): make async ports object-safe`

- [ ] 10. Shared 12-slot queue
  What to do / Must NOT do: Add `auto/queue.rs` with `AutoQueue { slots: Arc<Semaphore>, active: Arc<Semaphore> }`. `new(max_total, max_active)` requires `1 <= max_active <= max_total <= 12`; defaults are 12 total and 1 active. `new` returns `Result<Self, QueueError>` with `QueueError::Limit` on violation; define `pub enum QueueError { Limit, Full }`; `enter()` returns `Result<QueuePermit, QueueError>`, tries a total slot, and no slot -> `QueueError::Full`; then awaits an active permit. The permit holds both until the run ends. `max_total` is total in-flight (1 active + 11 waiting), not 12 waiting. A 13th enqueue is `QueueError::Full`; `run_auto` maps it to `RunError::QueueFull`. The queue does not own cancellation or budget; `run_auto` maps cancellation to `Aborted` and budget expiry to `BudgetExceeded`, with no either/or. Do not create a per-run semaphore; do not allow `max_active > 12`.
  Parallelization: Wave 2 | Blocked by: 9 | Blocks: 11,12
  References: `tokio::sync::Semaphore`; `AGENTS.md` (one active VM, hard ceiling).
  Acceptance criteria: `cargo test -p stellwerk-task --test auto_queue` green; 12 concurrent `enter()` calls -> 1 active, 11 waiting; 13th -> `QueueError::Full`; release drains FIFO; `max_active=13` -> `QueueError::Limit`. Cancellation/deadline assertions belong to `auto_run`, not `auto_queue`.
  QA scenarios: happy — 12 slots drain one active at a time; failure — 13th `QueueFull`.
  Commit: Y | `feat(stellwerk-task): add a shared 12-slot run queue`

- [ ] 11. Async run_auto state machine
  What to do / Must NOT do: Replace `auto/run.rs` with `pub async fn run_auto(..., queue: &AutoQueue, task_store: Arc<SqliteStore>, cancel: tokio::sync::watch::Receiver<bool>) -> Result<RunReport, RunError>`. Add `QueueFull` to `RunError` for the 13th enqueue; keep every other variant explicit. Acquire `queue.enter().await` before creating the task. Keep the base plan’s exact state/failure table: `BudgetExceeded`, `ChecksFailed`, `EvidenceFailed`, `AwaitingHumanValidation` only with `bundle_path = Some`; `RunState::AwaitingHumanValidation` without a bundle is a bug. Pin the worker future and use `tokio::select!` over it, the deadline, and `cancel.changed()`; on cancellation set a local `cancelled` flag, let the worker’s `execute` observe the same receiver and kill/await its child/VM, await the worker future, then return `Aborted`. Do not drop the worker future as a teardown mechanism; if the worker does not finish within the run budget, return `Aborted` with a teardown failure, never success. Bridge every `lifecycle::advance` through `RuntimeBridge::run_blocking` with a cloned `Arc<SqliteStore>`; await `TaskStore` methods directly. Keep `RunStore` borrowed sync. Do not create 12 VMs; do not call `block_on` inside `run_auto`; do not auto-merge.
  Parallelization: Wave 2 | Blocked by: 10 | Blocks: 12
  References: base plan state machine `:344-348`; failure table `:344-348`; `tokio::select!`, `process::Child::kill`.
  Acceptance criteria: `cargo test -p stellwerk-task --test auto_run` green; 12 concurrent runs with one queue -> 1 active, 11 waiting, 13th `QueueFull`; cancellation -> `Aborted` only and the worker child/VM is killed and reaped (no orphan PID); budget -> `BudgetExceeded`; `lifecycle::advance` wrapped; no `Arc<dyn TaskStore>`; `AwaitingHumanValidation` always has `bundle_path`.
  QA scenarios: happy — queue drains one active VM at a time; failure — 13th `QueueFull`, hung worker killed.
  Commit: Y | `feat(stellwerk-task): orchestrate async runs with a shared queue`

- [ ] 12. Async CLI and classic adapters
  What to do / Must NOT do: Update `crates/stellwerk-cli/src/auto_planner.rs` so `CliPlanner` and `FixturePlanner` implement the boxed-future `Planner`; add `pub fn validate_limits(max_sandboxes: usize, max_active_vms: usize) -> Result<(), QueueError>` as the single pure limit validator. Update `crates/stellwerk-cli/src/plan.rs` only for the parser visibility needed by the planner. Update `crates/stellwerk-cli/src/main.rs`: `main` stays sync, creates one `tokio::runtime::Runtime`, parses `--max-sandboxes`/`--max-active-vms` with defaults 12/1, calls `validate_limits(max_sandboxes, max_active_vms)` for `1 <= max_active_vms <= max_sandboxes <= 12`, then constructs one `AutoQueue::new(max_sandboxes, max_active_vms)`, creates `let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);`, and calls `block_on(run_auto(..., &queue, Arc::new(task_store), cancel_rx))`; `accept` stays sync. Classic mode remains default; no `--worker-mode` here. Do not use `#[tokio::main]`; do not change the direct-mode path; do not add a new model provider default; do not change `AutonomyPolicy`.
  Parallelization: Wave 3 | Blocked by: 11 | Blocks: F1-F4
  References: base plan CLI `:369-385`; `crates/stellwerk-cli/src/main.rs`; `crates/stellwerk-cli/src/auto_planner.rs`; `crates/stellwerk-task/src/auto/queue.rs`.
  Acceptance criteria: `cargo test -p stellwerk-cli --lib auto_planner` and `cargo test -p stellwerk-task --test auto_run` green; a classic fake run reaches `AwaitingHumanValidation`; CLI creates one queue; `validate_limits(13, 1)` -> `QueueError::Limit`; `validate_limits(13, 13)` -> `QueueError::Limit`; `validate_limits(0, 0)` -> `QueueError::Limit`; `main` passes both flags to `validate_limits` before `AutoQueue::new`; `--fake-model` still works; no `#[tokio::main]`; `main` has one `block_on`; no `AutonomyPolicy` field change.
  QA scenarios: happy — classic fake run; failure — invalid limits -> `QueueError::Limit` before worktree.
  Commit: Y | `feat(stellwerk-cli): run the async classic path`

## Final verification wave
> Runs after ALL todos. F1-F4 must pass; a `BLOCKED` prerequisite is reported as `BLOCKED`, never as PASS.

- [ ] F1. Plan compliance
  Run `python3 scripts/plan-lint.py --self-test`, `python3 scripts/plan-lint.py`, `python3 scripts/plan-lint.py .sisyphus/plans/M6`, and `git diff --check`. Verify 12 active canonical packets (base set under `_archive/base/` excluded), 12 mirror packets, byte-identical mirror (C8 non-vacuous), acyclic `depends_on`, every `files_allowed` matches the todo, and no forbidden path. Require M6 zero C1/C3/C4, C8 clean, C9 zero new M6; record the N1/N2/TUI baseline verbatim. Verify no `contract.rs`/`modules.lock`/`.github` edits; no `#[allow]`/`#[ignore]`/`opencode --auto`; no dependency beyond packet 00b.
  Evidence: `.omo/evidence/f1-m6-async-diffy-gix.log`.

- [ ] F2. Code quality
  Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets`. All exit 0. Evidence: `.omo/evidence/f2-m6-async-diffy-gix.log`.

- [ ] F3. Real QA
  Run `cargo test -p stellwerk-task --test auto_run`, `cargo test -p stellwerk-cli --lib auto_planner`, the 12-slot queue test with 1 active and 13th `QueueFull`, the cancellation test asserting `Aborted`, the diff conflict test asserting unchanged bytes, the add/modify/delete test, and the no-`git` sentinel; run each with `-- --list` first and fail on zero selected tests. If `cargo test` is not allowlisted for `evidence_verify`, record `BLOCKED`; do not substitute. Evidence: `.omo/evidence/f3-m6-async-diffy-gix.json`.

- [ ] F4. C2 fidelity
  For each packet, run `python3 scripts/plan-lint.py --budget <packet-merge-commit>`; assert ≤3 files, ≤200 src, ≤250 tests; assert `Cargo.lock` is excluded only by the ADR-0023 rule; assert no packet exceeds its declared split. Evidence: `.omo/evidence/f4-m6-async-diffy-gix.log`.

## Commit strategy
- One commit per packet, in dependency order. `00a` (C2 rule) must land before `00b` (lockfile); `00b` before `00c`.
- Test-only RED commits are allowed only for a split packet and must be followed immediately by the implementation.
- Before merge: run the full suite on `main` (not only the worktree), then `git merge --no-ff`.
- After merge: run the exact test command on `main`, `evidence_append(ModelClaim)`, `evidence_verify(tool_name="cargo test", args=[...])`, `gate_evaluate(required=["Verified"])`, then commit `chore(evidence): attest <packet> after merge` with the chain tail. Never attest a worktree-only revision.
- Never commit `target/`, tokens, or generated run bundles.

## Success criteria
- Base M6 precondition is recorded; if absent, the plan stops `BLOCKED` and adds nothing.
- 12 active canonical packets and a byte-identical mirror exist; the base packet set is archived; C8 is non-vacuous; C9 has no new M6 violation.
- gix owns worktree/head/status/commit; Stellwerk never invokes host `git`; `change.diff` remains PolyForge-generated.
- diffy owns add/modify/delete; apply is atomic, baseline-bound, and rolls back on failure.
- Object-safe boxed-future ports compile under `-D warnings`; no AFIT/`async-trait`/`#[allow]`.
- One shared queue enforces 12 total, 1 active, 13th `QueueError::Full` (mapped to `RunError::QueueFull`); cancellation `Aborted`, budget `BudgetExceeded`.
- Classic mode reaches `AwaitingHumanValidation`; PolyForge evidence is per packet.
- Direct mode is explicitly out of scope and lives in `m6b-firecracker-qwen.md`, which starts `BLOCKED` until its prerequisites exist.
