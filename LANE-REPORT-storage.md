# Lane report: storage

## Unfinished or unproven
- **Unproven, native:** the cross-process qualification on a real Windows desktop, where the engine creates the trial app, exits after the slot-held event, and the app (then its custody helper) keeps the run while a second launch is refused. A silent child (antivirus holding the new executable) being ended after 30 s + 2 s, with the accepted release starting, is also unproven natively. Windows tests exist for every step but run only in CI.
- **Unproven here:** none of the Windows tests ran in this container. They type-check under Windows clippy, so CI (`windows-2022`) is the first run. See Verification.
- **Pre-existing, not mine:** on Linux, workspace clippy with `--all-features` fails in `sugarsubstitute-execution/src/admission.rs`, `admission_record.rs` and `runtime_supervision.rs`, none of which I touched; main has the same lines. Main's CI hides this because it stops first on the missing comfy-desktop submodule. It belongs to the `lane/ci` baseline owner.

## Decisions made
- **Unadopted child that exited on its own** counts toward the abandoned-trial streak (`abandon_trial`; three in a row make one strike) instead of `release_trial`. `release_trial` resets that streak, so a release that crashes before taking over would otherwise be retried at every launch forever (the same trap C-46 closed). **A silent child the creator ended** uses `release_trial` (new `Undecided::Unadopted`), as briefed, and never counts against the release.
- **`TRIAL_ADOPTION` = 30 s, grace 2 s.** The child adopts only after its coordinator election. The first start of a new executable on a slow HDD with antivirus scanning it can take many seconds, and ending it too early would retry the update at every launch without ever applying it.
- **The handoff byte is held through a third, overlapped file object** (`ReOpenFile`). A sender can wait for an earlier sender's handoff in the kernel (`file_lock::lock_range`, new), and can release its handoff while keeping its slot. That object closes before the slot's, so a held handoff always means a held slot. The `native-exclusion-slots-v1` offsets are unchanged and both generations' objects interoperate.
- **Trial entry stays protocol v4**, now with three handles. Creator and child are one generation since the thin launcher. The catalog note says so.
- **The child signals right after adopting and matching its snapshot**, before payload verification, so the creator's wait doesn't include hashing.

## Commits (lane/storage)
- `f2139bb4` chore(storage): carry the storage lane's unfinished work to the cloud (the snapshot; force-push to rewrite it was denied, so it stays)
- `7ad3db88` merge: bring main into the storage lane
- `0d997540` fix(updates): keep starting the previous release while an update is in progress or briefly held by antivirus
- `14f62816` fix(updates): stop indexers and antivirus scanners from blocking an update's first start, and end an updated release that never takes over

## Verification
- `cargo xtask governance`: passed, with the comfy-desktop submodule checked out locally.
- `cargo test --locked -p sugarsubstitute-contracts`: 299 passed.
- Windows cross-check (`cargo clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings`): passed for sugarsubstitute-supervision, sugarsubstitute-execution and sugarsubstitute.
- Windows workspace leg (`--workspace --all-targets --all-features`), the `sugarsubstitute` `--no-default-features` leg, `cargo fmt --check` and Windows rustdoc with `-D warnings` (supervision, execution, contracts): all passed.
- The Linux `sugarsubstitute` clippy legs cannot build here (no GTK system libraries).
- CI: [43340890670](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43340890670) for `0d997540` failed on every OS at the known baseline (missing comfy-desktop submodule), before compiling. [43341734823](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43341734823) for `14f62816` runs the same gate. `lane/ci` was still red, with no LANE-REPORT-ci.md, so it isn't merged. **Next:** merge `origin/lane/ci` once it's green and rerun, which is the first real run of the Windows tests.

## For other lanes and the coordinator
- **retire:** `pub fn pointed_generations(installation: &Path, context: &JobContext) -> Result<Vec<GenerationId>, PointerFailure>` in `sugarsubstitute_supervision::setup_storage`. It returns primary then fallback; a missing pointer gives `Ok(vec![])`; any other failure is an error, and the caller then keeps every generation. Run it on an admitted storage worker reserving `POINTER_RESERVATION_BYTES`.
- **trial:** `Undecided::Unadopted` was added to `contracts/installation/verdict.rs`.
- **ui / coordinator:** while the headless creator waits for a silent child (up to 32 s), nothing is on screen. If the trial app adopted earlier, before its election, the wait would shrink. That's outside this lane.
- **lane/ci:** the Linux `--all-features` clippy errors above.
