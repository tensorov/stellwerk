# m6b-firecracker-qwen — Work Plan

## TL;DR (For humans)

**What you'll get:** an explicit direct mode for M6: Firecracker microVM lifecycle, a host Unix-socket <-> guest vsock bridge, Qwen chat transport, a direct worker, and `--worker-mode direct`. The classic bwrap/opencode path stays the default. Direct mode is never silently downgraded: real direct requires P1-VM + P1-Qwen; `--fake-qwen` uses the real VM with a fake Qwen backend and requires P1-VM only; any missing required set produces `BLOCKED`.

**Why this approach:** Firecracker 1.17 is installed and `/dev/kvm` exists, but the guest kernel, rootfs, guest agent, `socat`, and Qwen endpoint are operator artifacts. This plan makes those artifacts a hard precondition instead of pretending a real VM can be tested without them. Host-side tests use Unix socket pairs and a fake agent; the real integration check is a QA script that exits `BLOCKED` when the environment is absent.

**What it will NOT do:** No new Rust dependency. No `tokio-vsock`; the guest uses `socat` to bridge vsock to the agent’s Unix socket. No 12 VMs: direct mode still uses the shared queue with `max_active_vms = 1`. No auto-merge, no host `git`, no `#[allow]`/`#[ignore]`, no skipped tests, no “VM result is trusted” path.

**Effort:** L
**Risk:** High — rootfs reproducibility, Firecracker API, guest agent protocol, Qwen transport.
**Decisions to sanity-check:** rootfs is an operator artifact with a pinned manifest; guest agent listens on a Unix socket; `socat` bridges vsock; Qwen is task-local, not CLI-owned; host diffy apply remains authoritative.

Your next move: do **not** start until `m6-async-diffy-gix` is complete and the rootfs/Qwen prerequisites below are present. If they are not, record `BLOCKED` and stop.

---

> TL;DR (machine): L / high risk / Firecracker 1.17 + KVM + rootfs manifest + vsock/socat bridge + Qwen transport + direct worker + dual-mode CLI. No new deps. Real direct is BLOCKED without P1-VM/P1-Qwen; `--fake-qwen` needs P1-VM only. Host apply remains authoritative.

## Scope

### Must have
- Precondition P0: `m6-async-diffy-gix` is complete and merged; `AutoQueue`, `run_auto`, diffy apply, and gix helpers exist.
- Precondition P1-VM: Firecracker binary, `/dev/kvm`, kernel image, rootfs image, rootfs SHA-256, and vsock UDS path are present. Precondition P1-Qwen: Qwen base URL, model, and token file are present. Real direct mode requires P1-VM + P1-Qwen; `--fake-qwen` requires P1-VM only. Missing required set -> `DirectError::Blocked` before any VM/worktree.
- A pinned rootfs manifest and verifier: required binaries (`/usr/local/bin/stellwerk-vm-agent`, `/usr/bin/socat`, offline Rust toolchain, `cargo`, `rustc`), required directories (`/work`, `/run`), agent SHA-256, rootfs SHA-256. The manifest is the reproducibility contract; the plan does not build an unpinned image.
- Firecracker lifecycle: generated `config.json`, per-run writable rootfs copy, API socket, `InstanceStart`, graceful stop, SIGKILL fallback, cleanup on every path.
- Host vsock transport: Firecracker `vsock.uds_path` -> `tokio::net::UnixStream`; guest `socat VSOCK-LISTEN:1024,fork,reuseaddr UNIX-CONNECT:/run/stellwerk-agent.sock`; length-prefixed JSON protocol v1, max frame 16 MiB, request/response correlation by `run_id`.
- Guest tree seed/export: before Qwen, the host sends the sandbox text tree into guest `/work` over vsock (`TreeStart`/`TreeFile`/`TreeEnd`); after the guest applies the patch, it exports the final `/work` tree back, and the host writes it into its sandbox copy before computing the final manifest. No shared filesystem, no virtiofs/9p, no host mount of the rootfs.
- Qwen transport: task-local `QwenConfig`, public `HttpBackend::send_chat`, `max_tokens`, `finish_reason`, timeout, input/output byte caps, token via `read_token_outside(path, visible, name)`; no CLI dependency.
- Direct worker: implements the base `Worker` port; preflight -> start VM -> Qwen plan -> guest agent applies in `/work` -> stop VM -> return `WorkerResult`; `run_auto` remains authoritative for host manifest diff, diffy apply, gix commit, checks, and evidence. The direct worker never commits or returns `RunReport`.
- Dual-mode CLI: `WorkerMode::{Classic, Direct}`, default `Classic`; `--worker-mode`, `--fake-qwen`, `--max-active-vms`, `--max-sandboxes`; direct missing prerequisites -> `BLOCKED`.
- Per-packet PolyForge evidence: merge -> `evidence_verify` -> `gate_evaluate(required=["Verified"])` -> evidence commit.

### Must NOT have
- No new dependency; no `tokio-vsock`, `libc`, `nix`, `virtiofsd`, `async-trait`, `git2`.
- No 12 active VMs; `max_active_vms` default 1, hard ceiling 12; total runs still 12.
- No host `git` in Stellwerk code; no `git` subprocess for worktree/apply/evidence.
- No shared filesystem, virtiofs, 9p, or host mount of the rootfs; guest `/work` is populated and exported only over the framed vsock protocol. No binary or symlink transfer: reject with `DirectError::Unsupported`.
- No `#[allow]`/`#[ignore]`; no test that silently returns early as a skip. The real VM check is a QA script that exits `BLOCKED` when env is absent.
- No trusting the guest result; `run_auto` (not `DirectWorker`) performs the authoritative host diffy apply and gix commit. The direct worker returns `WorkerResult` only.
- No automatic fallback from direct to classic; an explicit `BLOCKED` is the only outcome when prerequisites are missing.
- No edits to `contract.rs`, `modules.lock`, `.github/workflows/`, or root `Cargo.toml`.

## Preconditions

**P0 — async/gix/diffy base.** `m6-async-diffy-gix` is merged; these exist:
- `crates/stellwerk-task/src/auto/{ports,queue,run,apply,git}.rs`
- `AutoQueue::new(12, 1)`, `run_auto(..., queue, task_store)`
- diffy `FileOperation::{Add,Modify,Delete}` and atomic apply
- gix `Worktree`, `head_id`, `is_clean`, `stage_and_commit`

If not, stop `BLOCKED: m6-async-diffy-gix incomplete`.

**P1-VM — direct VM environment.** All must be set and valid:
- `STELLWERK_FIRECRACKER_BIN` (default `/usr/bin/firecracker`, v1.17.0)
- `STELLWERK_FIRECRACKER_KERNEL` (existing file)
- `STELLWERK_ROOTFS` (existing file)
- `STELLWERK_ROOTFS_SHA256` (64 hex chars; must match `sha256sum`)
- `STELLWERK_VSOCK_UDS` (absolute path in a writable run dir)

**P1-Qwen — real Qwen environment.** Required for real direct mode, not for `--fake-qwen`:
- `STELLWERK_QWEN_BASE_URL` (http(s) URL)
- `STELLWERK_QWEN_MODEL`
- `STELLWERK_QWEN_TOKEN_FILE` (existing file, mode 0600)
- `STELLWERK_QWEN_MAX_INPUT_BYTES` (default 65536)
- `STELLWERK_QWEN_MAX_OUTPUT_TOKENS` (default 4096, max 8192)
- `STELLWERK_QWEN_TIMEOUT_SECONDS` (default 300)

If any required item for the selected mode is absent or invalid, `DirectError::Blocked { missing }`; no VM start, no worktree, no Qwen request. Real direct requires P1-VM + P1-Qwen; `--fake-qwen` requires P1-VM only. The error lists every missing item, not the first.

## Verification strategy
> Every check is agent-executed. Real VM QA is `BLOCKED`, never “green by assumption”.

- **TDD:** every Rust path gets a failing test first. Host-side tests use a Unix socket pair and a fake agent; no test is `#[ignore]` or silently skipped.
- **Unit:** config validation, rootfs manifest/hash, Firecracker config JSON, API request framing, vsock frame limits, Qwen request/response parsing, direct worker blocked when env missing, fake Qwen + fake sandbox.
- **Integration:** `scripts/qa-firecracker.sh` starts a real VM only when P1-VM is present; exit 0 = pass, exit 2 = `BLOCKED`, any other exit = failure. It is not a test and does not fake a skip.
- **Commands:** `cargo test -p stellwerk-task --test direct_firecracker`, `cargo test -p stellwerk-task --test direct_vsock`, `cargo test -p stellwerk-task --test direct_vsock_tree`, `cargo test -p stellwerk-task --test direct_worker_vm`, `cargo test -p stellwerk-task --lib auto::qwen`, `cargo test -p stellwerk-task --lib auto::direct::worker`, `cargo test -p stellwerk-cli --test direct_cli`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `python3 scripts/plan-lint.py --self-test`, `python3 scripts/plan-lint.py`.
- **Non-vacuous selection:** every acceptance command names `--test <target>` or `--lib`; before treating it as green, run the same command with `-- --list` and assert at least one test. A zero-test selection is a failure, never a pass.
- **Evidence:** per packet, merge -> exact test on `main` -> `evidence_append(ModelClaim)` -> `evidence_verify(tool_name="cargo test", args=[...])` -> `gate_evaluate(required=["Verified"])` -> evidence commit. If `cargo test` is not allowlisted, `BLOCKED`.
- **No-git sentinel:** direct worktree/apply/evidence tests run with a poisoned `git` on `PATH`; marker must be absent.

## Execution strategy
### Parallel execution waves
> Execution is strictly sequential in one worktree.

- **Wave 0a — preflight (0 packets):** canonical M6b packet set and byte-identical mirror.
- **Wave 0 — rootfs manifest (1 packet):** pinned rootfs contract and verifier.
- **Wave 1 — Firecracker (1 packet):** config/lifecycle.
- **Wave 2 — vsock (2 packets):** host transport/frame + guest tree seed/export/apply protocol.
- **Wave 3 — Qwen (1 packet):** public chat transport.
- **Wave 4 — direct worker (2 packets):** sandbox/seed/result + VM/Qwen/export path.
- **Wave 5 — dual mode (1 packet):** CLI.
- **Wave 6 — real VM QA (1 packet):** QA script.
- **Wave 7 — proof (F1-F4).**

### Dependency matrix
| Todo | Depends on | Blocks |
| --- | --- | --- |
| 1 | P0 | 2-8 |
| 2 | 1 | 3-8 |
| 3 | 2 | 4,6 |
| 4 | 2,3 | 6 |
| 5 | 2,3,4 | 6 |
| 6 | 3,4,5 | 7 |
| 7 | 6 | 8,F1-F4 |
| 8 | 7 | F1-F4 |

## Packet set

All packets use frontmatter `milestone: M6b`; the canonical directory is `.sisyphus/plans/M6b/` and the mirror is `.omo/plans/M6b/`.

| Packet | Files (max 3) | Purpose |
| --- | --- | --- |
| `packet-31a` | `infra/vm-rootfs/manifest.txt`, `infra/vm-rootfs/README.md`, `scripts/vm-rootfs-check.sh` | Pin and verify the operator rootfs. |
| `packet-31b` | `crates/stellwerk-task/src/auto/firecracker.rs`, `crates/stellwerk-task/src/auto/mod.rs`, `crates/stellwerk-task/tests/direct_firecracker.rs` | Firecracker config/lifecycle; register `pub mod firecracker;`. |
| `packet-31c1` | `crates/stellwerk-task/src/auto/vsock.rs`, `crates/stellwerk-task/src/auto/mod.rs`, `crates/stellwerk-task/tests/direct_vsock.rs` | Host UDS transport, framing, correlation, timeout; register `pub mod vsock;`. |
| `packet-31c2` | `crates/stellwerk-task/src/auto/vsock.rs`, `crates/stellwerk-task/tests/direct_vsock_tree.rs` | Guest tree seed/export and patch apply protocol. |
| `packet-31d` | `crates/stellwerk-task/src/m5/model_backend.rs`, `crates/stellwerk-task/src/auto/mod.rs`, `crates/stellwerk-task/src/auto/qwen.rs` | Public Qwen chat transport; register `pub mod qwen;`; tests inline. |
| `packet-31e1` | `crates/stellwerk-task/src/auto/mod.rs`, `crates/stellwerk-task/src/auto/direct/mod.rs`, `crates/stellwerk-task/src/auto/direct/worker.rs` | Sandbox copy, baseline manifest, seed, fake worker; register `pub mod direct;`; tests inline. |
| `packet-31e2` | `crates/stellwerk-task/src/auto/direct/worker.rs`, `crates/stellwerk-task/tests/direct_worker_vm.rs` | VM/Qwen/apply/export path and `WorkerResult`. |
| `packet-31f` | `crates/stellwerk-cli/src/main.rs`, `crates/stellwerk-task/src/auto/config.rs`, `crates/stellwerk-cli/tests/direct_cli.rs` | `WorkerMode`, flags, CLI limits, direct CLI tests. |
| `packet-31g` | `scripts/qa-firecracker.sh` | Real VM QA script; exits 2 when P1-VM is absent. |

`depends_on` chain: `31a -> 31b -> 31c1 -> 31c2 -> 31e1 -> 31e2 -> 31f -> 31g`; `31d` depends on `31c1`; `31e1` depends on `31c2,31d`; `31e2` depends on `31b,31c2,31d,31e1`; `31f` depends on `31e2`; `31g` depends on `31f`.
Cross-milestone `depends_on`: `31b`, `31c1`, `31d`, and `31e1` include `M6-14a`, `M6-14b`, `M6-15a`, `M6-16a`, `M6-19a`, `M6-21a`, `M6-21b1`, `M6-21b2` because each touches `crates/stellwerk-task/src/auto/mod.rs`; `31d` also includes `N2-00g3b`, `N2-00j4`, `N2-00j5` because it touches `crates/stellwerk-task/src/m5/model_backend.rs`; `31f` includes `M6-23c`, `N1-01`, `N1-10`, `N2-00h2b`, `TUI-02`, `TUI-03`. This is the C9 ordering declaration; no `overlap_waiver`.

## Todos

- [ ] 1. Preflight, canonical packets, and mirror
  What to do / Must NOT do: Verify P0. If P0 is missing, write `BLOCKED: m6-async-diffy-gix incomplete` and stop; do not create packets. Otherwise create the nine canonical packet files above under `.sisyphus/plans/M6b/` with frontmatter `packet`, `milestone: M6b`, `depends_on` (short IDs as listed), `files_allowed`, `max_diff_lines`, `acceptance`, `qa`, and `commit`; regenerate `.omo/plans/M6b/` as a byte-identical copy; do not hand-edit the mirror. Do not write product code; do not add dependencies.
  Parallelization: Wave 0a | Blocked by: P0 | Blocks: 2-8
  References: `AGENTS.md` (canonical source, C8, C9); `.sisyphus/plans/M6b/`; `.omo/plans/M6b/`.
  Acceptance criteria: all nine canonical packets exist (`31a`, `31b`, `31c1`, `31c2`, `31d`, `31e1`, `31e2`, `31f`, `31g`); 9 canonical packets and 9 mirror packets; `diff -r .sisyphus/plans/M6b .omo/plans/M6b` is empty; `python3 scripts/plan-lint.py .sisyphus/plans/M6b` has no C1/C3/C4; C8 clean.
  QA scenarios: happy — mirror equals canonical; failure — add a mirror-only packet and assert C8 fails, then remove it.
  Commit: Y | `docs(m6b): add Firecracker/Qwen packet set and mirror`

- [ ] 2. Pinned rootfs manifest and verifier
  What to do / Must NOT do: Add `infra/vm-rootfs/manifest.txt` with exact required paths, modes, and SHA-256 for `/usr/local/bin/stellwerk-vm-agent` and `/usr/bin/socat`; required dirs `/work`, `/run`; offline Rust toolchain paths; rootfs format `ext4`; kernel version contract; and a placeholder for the operator’s pinned OCI digest. Add `infra/vm-rootfs/README.md` documenting that the image is an operator artifact, how to build it from a pinned digest, that no unpinned network install is allowed at run time, and the guest command `socat VSOCK-LISTEN:1024,fork,reuseaddr UNIX-CONNECT:/run/stellwerk-agent.sock`. Add `scripts/vm-rootfs-check.sh` that validates `STELLWERK_ROOTFS`, `STELLWERK_ROOTFS_SHA256`, every manifest path, agent hash, and `socat` presence; exit 2 with `BLOCKED: <missing>` if the rootfs is absent, 1 on mismatch, 0 on pass. Do not build an unpinned image; do not add a new Rust dependency; do not use `#[ignore]`.
  Parallelization: Wave 0 | Blocked by: 1 | Blocks: 3-8
  References: `docs/adr/0022-async-diffy-gix.md`; Firecracker rootfs requirements; `socat` vsock bridge.
  Acceptance criteria: `scripts/vm-rootfs-check.sh` exits 2 when `STELLWERK_ROOTFS` is unset; exits 1 on a wrong hash; exits 0 for a fixture rootfs with the manifest paths; `manifest.txt` has a 64-hex rootfs hash line; no network install in the script.
  QA scenarios: happy — fixture rootfs passes; failure — wrong hash -> 1; absent -> 2.
  Commit: Y | `docs(vm): pin the rootfs contract and verifier`

- [ ] 3. Firecracker lifecycle
  What to do / Must NOT do: Add `auto/firecracker.rs` and register `pub mod firecracker;` in `auto/mod.rs`. `FirecrackerConfig::from_env()` validates P1-VM, generates `config.json` with `boot-source.kernel_image_path`, `drives[0].path_on_host` (per-run rootfs copy), `machine-config { vcpu_count: 2, mem_size_mib: 512, smt: false }`, `vsock { guest_cid: 3, uds_path }`, and no network. Start `firecracker --api-sock <run_dir>/firecracker.sock --config-file <run_dir>/config.json`; wait for the API socket; send a minimal HTTP/1.1 `PUT /actions` `{"action_type":"InstanceStart"}` over `UnixStream`; do not add an HTTP crate. Stop with SIGTERM, wait, SIGKILL fallback, remove the socket and rootfs copy. Copy the base rootfs to a per-run file; never mount the base writable. Do not start a VM if any prerequisite is missing; do not run 12 VMs.
  Parallelization: Wave 1 | Blocked by: 2 | Blocks: 4,6
  References: Firecracker v1.17 API; `tokio::process::Command`; `tokio::net::UnixStream`; `tokio::time::timeout`.
  Acceptance criteria: `cargo test -p stellwerk-task --test direct_firecracker` green; fake API socket receives `InstanceStart`; missing env -> `Blocked` with all names; per-run rootfs copy removed on drop; SIGTERM path stops; no network config.
  QA scenarios: happy — fake API accepts start; failure — missing `STELLWERK_ROOTFS` -> `Blocked`.
  Commit: Y | `feat(stellwerk-task): manage Firecracker VM lifecycle`

- [ ] 4. Vsock transport and guest tree protocol
  What to do / Must NOT do: Split into two sequential packets. **Packet 31c1 — transport/frame:** add `auto/vsock.rs` and register `pub mod vsock;` in `auto/mod.rs`; host connects to `STELLWERK_VSOCK_UDS` with `tokio::net::UnixStream`; frame is `u32` big-endian length + JSON; max 16 MiB; reject wrong version, oversized frame before allocation, mismatched `run_id`, and timeout. The guest command is documented by `31a` in `infra/vm-rootfs/README.md`: `socat VSOCK-LISTEN:1024,fork,reuseaddr UNIX-CONNECT:/run/stellwerk-agent.sock`; the agent listens on `/run/stellwerk-agent.sock`; the host sends `CONNECT 1024\n` and expects `OK 1024\n` before framing. **Packet 31c2 — tree/apply:** add `TreeStart { version, run_id, count, total_bytes }`, `TreeFile { path, mode, sha256, text }`, `TreeEnd { count }`, `Apply { patch, timeout_ms }`, `ApplyAck { status, stdout, stderr }`, and `ExportTree`/`TreeFile`/`TreeEnd` for the final guest tree. The host sends the sandbox text tree before Qwen, the guest writes it to `/work` after clearing only `/work`, and the guest exports the final tree after apply. Validate relative paths, reject `..`, absolute paths, `.git`, symlinks, binary/non-UTF-8 files, more than 4096 files, more than 64 MiB total, and any file over 16 MiB. No shared filesystem, virtiofs, 9p, host rootfs mount, `tokio-vsock`, `nix`, or `libc`.
  Parallelization: Wave 2 | Blocked by: 2,3 | Blocks: 6
  References: Firecracker vsock UDS; `socat` guest bridge; `tokio::net::UnixStream`; `tokio::io::{AsyncReadExt,AsyncWriteExt}`; `auto/manifest.rs` path rules.
  Acceptance criteria: `cargo test -p stellwerk-task --test direct_vsock --test direct_vsock_tree` green; frame round-trip; oversized frame rejected before allocation; wrong `run_id` rejected; timeout -> `VsockError::Timeout`; seed/export round-trip preserves paths/modes/hashes; traversal/symlink/binary/oversize rejected; guest command documented; no new dependency.
  QA scenarios: happy — fake agent seeds, applies one-line patch, exports changed tree; failure — 17 MiB frame or `../escape` -> `FrameTooLarge`/`PathRejected`.
  Commit: Y | `feat(stellwerk-task): bridge host UDS to guest vsock`

- [ ] 5. Qwen transport
  What to do / Must NOT do: In `crates/stellwerk-task/src/m5/model_backend.rs`, add `pub struct ChatReply { pub frames: Vec<Frame>, pub finish_reason: String }`; extend the private `send` to accept `max_tokens: Option<u32>` and return `(Vec<Frame>, String)`, with existing `turn`/`turn_request` passing `None`; add `pub fn send_chat(&self, request: &TurnRequest, max_tokens: u32) -> Result<ChatReply, BackendError>` that passes `Some(max_tokens)`, adds `max_tokens` to the JSON body, and parses `finish_reason`. In `auto/qwen.rs`, add `QwenConfig::from_env()` with P1-Qwen fields and `QwenBackend` using `HttpBackend::send_chat`; `finish_reason` must be `stop` (accept `length` only with `QwenError::Truncated`); reject empty content, invalid JSON, non-2xx, oversized input, and timeout. Token comes from the free function `read_token_outside(path, visible, name)` with `visible = &[<run>/sandbox]` (the existing `visible: &[PathBuf]` signature), the direct run sandbox path; the token file must be outside that sandbox. No token in argv/env of the worker. Do not depend on `stellwerk-cli`; do not add a provider default; do not log the token.
  Parallelization: Wave 3 | Blocked by: 2,3,4 | Blocks: 6
  References: `crates/stellwerk-task/src/m5/model_backend.rs`; `auto/qwen.rs`; `TokenProxy`; `HttpBackend`.
  Acceptance criteria: `cargo test -p stellwerk-task --lib auto::qwen` green; request includes `max_tokens`; `finish_reason=length` -> `Truncated`; 500 -> `Http`; timeout -> `Timeout`; token absent from errors; `cargo tree -p stellwerk-task` has no `stellwerk-cli`.
  QA scenarios: happy — fake HTTP returns `stop`; failure — `length` -> `Truncated`.
  Commit: Y | `feat(stellwerk-task): add Qwen chat transport for direct mode`

- [ ] 6. Direct worker
  What to do / Must NOT do: Split into two sequential packets. **Packet 31e1 — sandbox/seed:** add `auto/direct/worker.rs`, register `pub mod direct;` in `auto/mod.rs`, and add `pub mod worker;` to `auto/direct/mod.rs`; define `DirectError` with `Blocked { missing: Vec<String> }` and `Unsupported { detail: String }`, derive `Debug`, and implement `Display`/`std::error::Error`; define `DirectWorker` (`state_dir`, `qwen: Option<QwenConfig>`, Firecracker paths, `require_qwen: bool`), `pub fn check_prerequisites(&self, require_qwen: bool) -> Result<(), DirectError>` that checks the P1-VM/P1-Qwen env names and paths directly (not via fallible `QwenConfig::from_env`) and returns `DirectError::Blocked { missing }` for every missing item; `execute` constructs `QwenConfig::from_env()` only after preflight passes; real direct uses `require_qwen = true` (P1-VM + P1-Qwen), `with_fake_qwen` sets it to `false` (P1-VM only); `copy_sandbox` (no `.git`), baseline manifest, and seed tree over vsock. Define `FakeDirectWorker` (host-only, no VM/Qwen/P1) and `impl Worker for FakeDirectWorker` with `preflight` returning `Box::pin(async { Ok(()) })` and `execute` with the same five-argument signature, ignoring `cancel`, returning `Box::pin(async move { Ok(fake_worker_result) })`; add a `Box<dyn Worker>` compile test for `FakeDirectWorker`. **Packet 31e2 — VM/execute:** implement `impl Worker for DirectWorker` with `fn preflight<'a>(&'a self) -> BoxFuture<'a, Result<(), WorkerError>>`: compute `let result = self.check_prerequisites(self.require_qwen);` once, then return `Box::pin(async move { match result { Ok(()) => Ok(()), Err(DirectError::Blocked { missing }) => Err(WorkerError::Unconfigured { missing }), Err(DirectError::Unsupported { detail }) => Err(WorkerError::Unsupported { detail }) } })`; the non-`Blocked` arm is a distinct failure, never `Ok(())`; then `fn execute<'a>(&'a self, plan: &'a AutoPlan, worktree: &'a Path, deadline: Instant, cancel: tokio::sync::watch::Receiver<bool>) -> BoxFuture<'a, Result<WorkerResult, WorkerError>>`; add a `Box<dyn Worker>` compile test for `DirectWorker`; support `DirectWorker::with_fake_qwen()` (real VM + real vsock, fake Qwen, P1-VM only) alongside real Qwen (P1-VM + P1-Qwen); start Firecracker, seed, Qwen, apply in guest, export final tree, write it into the sandbox, `tokio::select!` on `cancel.changed()` and `tokio::time::sleep_until(deadline)`, stop VM and await its exit on every path including cancellation and deadline, return `WorkerError::Timeout` on deadline, and return `WorkerResult { sandbox, baseline_manifest, final_manifest }`; `run_auto` validates the host diff. It must not call `auto::apply::apply`, gix commit, `run_auto`, or construct `RunReport`; `run_auto` consumes the result and performs the authoritative host manifest diff, diffy apply, gix commit, checks, and evidence. A guest success followed by host drift is detected by `run_auto` as an apply failure, never as success. No fallback to classic; no auto-merge; no 12 VMs.
  Parallelization: Wave 4 | Blocked by: 3,4,5 | Blocks: 7
  References: `auto/firecracker.rs`, `auto/vsock.rs`, `auto/qwen.rs`, `auto/ports.rs`, `auto/run.rs`, `auto/manifest.rs`.
  Acceptance criteria: `cargo test -p stellwerk-task --lib auto::direct::worker` green; fake sandbox + fake Qwen -> `WorkerResult` with three paths; `run_auto` reaches `AwaitingHumanValidation`; missing required P1-VM/P1-Qwen env -> `DirectError::Blocked` with all names; real VM/Qwen path is exercised only by `31g`; guest success + host drift -> `run_auto` `ChecksFailed`; VM stopped on error; deadline -> `WorkerError::Timeout` and VM stopped/reaped; worker never commits and never returns `RunReport`; no classic fallback.
  QA scenarios: happy — fake VM + fake Qwen -> `WorkerResult`, then `run_auto` apply/commit; failure — host drift -> `ChecksFailed`, VM stopped.
  Commit: Y | `feat(stellwerk-task): orchestrate direct Firecracker and Qwen runs`

- [ ] 7. Dual-mode CLI
  What to do / Must NOT do: Add `WorkerMode { Classic, Direct }` (default `Classic`) to `auto/config.rs`; do not change `AutonomyPolicy`. In `crates/stellwerk-cli/src/main.rs`, parse `--worker-mode classic|direct` and `--fake-qwen` on the existing `Auto` subcommand; add `crates/stellwerk-cli/tests/direct_cli.rs` for clap parse and dispatch tests; reuse the `--max-active-vms`/`--max-sandboxes` flags added by `M6-23c` and do not add duplicate clap args or a second source of truth; pass them to `AutoQueue::new`, which validates `1 <= max_active_vms <= max_sandboxes <= 12` and returns `QueueError::Limit` on violation. In `Direct` mode additionally require `max_active_vms == 1`; a larger value is `QueueError::Limit` before VM start, while classic keeps the 12-VM queue cap. Construct `AutoQueue::new(max_sandboxes, max_active_vms)`; for real `Direct`, call `DirectWorker::check_prerequisites(true)` (P1-VM + P1-Qwen) before `run_auto` and print `BLOCKED: <missing names>` on `DirectError::Blocked`; for `--fake-qwen`, construct `DirectWorker::with_fake_qwen()` (real VM, fake Qwen) which calls `check_prerequisites(false)` and requires only P1-VM; `FakeDirectWorker` remains test-only and requires no P1. Both `DirectWorker` and `FakeDirectWorker` implement `Worker`, including `preflight`; box the selected worker as `AutoDeps.worker`, create a `tokio::sync::watch::channel(false)` cancellation token, and pass its receiver to the same `run_auto` state machine; Ctrl-C wiring is out of scope (no tokio `signal` feature). `--fake-model` in direct mode only selects `FixturePlanner`; it must not require an HTTP worker URL because `DirectWorker` does not use the classic model transport. `run_auto` remains authoritative for host apply/commit/checks/evidence; there is no separate direct orchestration path. `--fake-qwen` is deterministic and never touches a real Qwen endpoint. `Classic` is unchanged. Do not auto-downgrade; do not add a new provider; do not change the classic default.
  Parallelization: Wave 5 | Blocked by: 6 | Blocks: 8, F1-F4
  References: `crates/stellwerk-cli/src/main.rs`; `crates/stellwerk-task/src/auto/config.rs`; `crates/stellwerk-task/src/auto/queue.rs`; `crates/stellwerk-task/src/auto/ports.rs`.
  Acceptance criteria: `cargo test -p stellwerk-cli --test direct_cli` green; default mode classic; real `--worker-mode direct` without P1-VM/P1-Qwen -> `BLOCKED` before task creation; `--max-active-vms 13` -> `QueueError::Limit`; `--max-sandboxes 0` -> `QueueError::Limit`; fake direct worker via the `FakeDirectWorker` test harness -> `AwaitingHumanValidation`; the real VM `--fake-qwen` path is exercised only in `31g`; direct `--max-active-vms 2` -> `QueueError::Limit`; `AutoDeps.worker` is `DirectWorker`/`FakeDirectWorker`, not a `RunReport`; both types implement `Worker` including `preflight`; `Box<dyn Worker>` compile test passes; queue receives `(max_sandboxes, max_active_vms)`; `--fake-model` in direct mode does not require an HTTP worker URL; no `AutonomyPolicy` field change.
  QA scenarios: happy — fake direct -> `WorkerResult` -> `run_auto` apply/commit; failure — direct without rootfs -> `BLOCKED`.
  Commit: Y | `feat(stellwerk-cli): add explicit direct worker mode`

- [ ] 8. Real VM QA script
  What to do / Must NOT do: Add `scripts/qa-firecracker.sh` (1 file). Resolve `STELLWERK_ROOT` as the absolute Stellwerk checkout root and run the script with CWD at `$STELLWERK_ROOT`; use `git -C "$REPO"` for every repo mutation and `$STELLWERK_ROOT`/`--manifest-path` for every Stellwerk command. Preflight all P1-VM items (Firecracker binary, `/dev/kvm`, kernel, rootfs, rootfs SHA, vsock path) and run `"$STELLWERK_ROOT/scripts/vm-rootfs-check.sh"` for the rootfs; if any P1-VM item is absent, print `BLOCKED` and exit 2. Otherwise create a standalone fixture crate (own `[workspace]`, no dependencies) with `"$REPO/Cargo.toml"`, `"$REPO/src/lib.rs"` (`pub fn answer() -> u32 { 1 }` plus a `#[cfg(test)]` test asserting `answer() == 2`), and `"$REPO/.stellwerk/autonomy.toml"` with the complete valid `[policy]`: `version = 1`, `owner = "qa"`, `base_ref = "main"`, `branch_prefix = "stellwerk/qa"`, `allowed_tools = ["read", "edit", "write"]`, `allowed_paths = ["src/lib.rs"]`, `forbidden_paths = ["tests/**", "Cargo.toml", "Cargo.lock", "tools/**", ".stellwerk/**", ".git/**", "opencode.json", ".opencode/**"]`, `max_files = 1`, `max_diff_lines = 20`, `budget_seconds = 900`, `mutation_threshold = 75.0`, `coverage_threshold = 80.0`, `acceptance_commands = ["cargo test --quiet", "cargo clippy --all-targets -- -D warnings"]`, `model = "opencode/deepseek-v4.1-flash-free"`, and `gate_plugin_sha256` = the literal 64-hex `sha256sum "$STELLWERK_ROOT/crates/stellwerk-task/opencode/stellwerk_gate.ts"` at script runtime (never a placeholder); the fake Qwen patch changes `answer` from `1` to `2`; run `git -C "$REPO" init -b main`, set a local test identity with `git -C "$REPO" config`, commit the baseline with `git -C "$REPO" commit`, and export `STELLWERK_STATE_DIR` outside the repo. If `STELLWERK_PF_BIN` is unset, first require `test -f /home/izyabretatel/gits/polyforge/Cargo.toml`; if absent, print `BLOCKED: missing PolyForge checkout` and exit 2; otherwise build it with `cargo build --manifest-path /home/izyabretatel/gits/polyforge/Cargo.toml -p polyforge-cli` and export `STELLWERK_PF_BIN=/home/izyabretatel/gits/polyforge/target/debug/polyforge-cli`; require `test -x "$STELLWERK_PF_BIN"`, export absolute `PF_LEDGER="$STELLWERK_STATE_DIR/polyforge/ledger.jsonl"` and `PF_EVIDENCE_DIR="$STELLWERK_STATE_DIR/polyforge/evidence"`, and `mkdir -p` both parents. Run `cargo run --manifest-path "$STELLWERK_ROOT/Cargo.toml" -p stellwerk-cli -- policy approve "$REPO" --owner qa` and assert exit 0. Then start one Firecracker VM at a time, invoke the CLI with the exact form `cargo run --manifest-path "$STELLWERK_ROOT/Cargo.toml" -p stellwerk-cli -- auto "$REPO" --goal "<goal>" --worker-mode direct --fake-qwen --fake-model --json` (the subcommand/flags from `31f`, not a Rust API call); in direct mode `--fake-model` selects `FixturePlanner` and does not require an HTTP worker URL. If P1-Qwen is present, require all P1-Qwen names and run `cargo run --manifest-path "$STELLWERK_ROOT/Cargo.toml" -p stellwerk-cli -- auto "$REPO" --goal "<goal>" --worker-mode direct --fake-model --json` (no `--fake-qwen`) sequentially after the fake-Qwen run; both runs use the real vsock seed/apply/export path, assert host `run_auto` reaches `AwaitingHumanValidation`, clean up on every path, and exit 0/1/2. It must not fake a skip or run overlapping VMs. No new Rust dependency.
  Parallelization: Wave 6 | Blocked by: 7 | Blocks: F1-F4
  References: `scripts/vm-rootfs-check.sh`; `auto/firecracker.rs`; `auto/vsock.rs`; `auto/direct/worker.rs`.
  Acceptance criteria: `bash -n scripts/qa-firecracker.sh`; the script uses `git -C "$REPO"` and `$STELLWERK_ROOT` absolute paths; any missing P1-VM item -> exit 2 with `BLOCKED`; fixture has `owner = "qa"`, is `git init`-ed and committed, `policy approve --owner qa` exits 0, and `STELLWERK_PF_BIN`/`PF_LEDGER`/`PF_EVIDENCE_DIR` are absolute and executable/created; fixture with P1-VM and an approved policy -> exit 0; `--fake-model` in direct mode does not require an HTTP worker URL; real Qwen path runs only when P1-Qwen is present; injected failure -> exit 1; never more than one VM at a time; cleanup removes socket/rootfs copy.
  QA scenarios: happy — fake Qwen real VM path; failure — missing rootfs -> 2; host drift -> 1.
  Commit: Y | `test(vm): add the real Firecracker QA script`

## Final verification wave
> F1-F4 must pass. A `BLOCKED` prerequisite is reported as `BLOCKED`, never PASS.

- [ ] F1. Plan compliance
  `python3 scripts/plan-lint.py --self-test`, `python3 scripts/plan-lint.py .sisyphus/plans/M6b`, `git diff --check`; 9 canonical packets under `.sisyphus/plans/M6b/` and a byte-identical `.omo/plans/M6b/` mirror; no forbidden path; no new dependency; no `#[allow]`/`#[ignore]`; C9 cross-milestone deps declared. Evidence: `.omo/evidence/f1-m6b-firecracker-qwen.log`.

- [ ] F2. Code quality
  `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets`. All exit 0. Evidence: `.omo/evidence/f2-m6b-firecracker-qwen.log`.

- [ ] F3. Real QA
  Run `scripts/vm-rootfs-check.sh`; if exit 2, record `BLOCKED` with the missing list. If present, run `scripts/qa-firecracker.sh` (real VM, real vsock, fake Qwen first, then real Qwen if configured), `cargo test -p stellwerk-task --test direct_firecracker`, `cargo test -p stellwerk-task --test direct_vsock`, `cargo test -p stellwerk-task --test direct_vsock_tree`, `cargo test -p stellwerk-task --test direct_worker_vm`, `cargo test -p stellwerk-task --lib auto::qwen`, `cargo test -p stellwerk-task --lib auto::direct::worker`, `cargo test -p stellwerk-cli --test direct_cli`, and the no-`git` sentinel; run each with `-- --list` first and fail on zero selected tests. Evidence: `.omo/evidence/f3-m6b-firecracker-qwen.json`.

- [ ] F4. C2 fidelity
  `python3 scripts/plan-lint.py --budget <packet-merge-commit>` for all 9 packets; ≤3 files, ≤200 src, ≤250 tests; `Cargo.lock` excluded only by ADR-0023; no packet split silently. Evidence: `.omo/evidence/f4-m6b-firecracker-qwen.log`.

## Commit strategy
- One commit per packet, in dependency order; `31a` before `31b`; `31c1` before `31c2`; `31e1` before `31e2`; `31e2` after `31b/31c2/31d`; `31f` last; `31g` after `31f`.
- Before merge: full suite on `main`; `git merge --no-ff`.
- After merge: exact test on `main`, `evidence_append(ModelClaim)`, `evidence_verify(tool_name="cargo test", args=[...])`, `gate_evaluate(required=["Verified"])`, evidence commit with chain tail.
- Never attest a worktree-only revision; never commit tokens, rootfs images, or VM bundles.

## Success criteria
- Direct mode is explicit, default-off, and `BLOCKED` with a complete missing list when the required P1-VM/P1-Qwen set for the selected mode is absent.
- Rootfs is pinned by manifest/hash; no unpinned network install at run time.
- Firecracker starts/stops one VM per run; per-run rootfs copy; no base writable mount.
- Host UDS <-> guest vsock bridge is framed, bounded, and timeout-aware; the sandbox tree is seeded into guest `/work` and the final tree exported back over vsock; no shared filesystem or new dependency.
- Qwen transport is task-local, bounded, and rejects truncation/oversize/timeout.
- `run_auto` performs the authoritative host diffy apply and gix commit; `DirectWorker` returns `WorkerResult` and the guest result is advisory.
- 12 total runs / 1 active VM; no 12-VM path.
- Classic mode remains default; no automatic fallback.
