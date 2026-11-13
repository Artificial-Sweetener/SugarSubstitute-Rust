# Lane report: storage

## Unfinished or unproven
- **Unproven, native:** the cross-process run handoff: the engine exits after the slot-held event, while the app and then its custody helper keep the run and a second launch is refused. Also unproven natively: a silent child ended after 30 s + 2 s, with the accepted release starting.
- **Unproven, CI:** no Windows test has run. They pass Windows clippy here, but CI stops at the known submodule baseline (see Verification).
- **Pre-existing, not mine:** Linux workspace clippy with `--all-features` fails in untouched `sugarsubstitute-execution` files (`admission.rs`, `admission_record.rs`, `runtime_supervision.rs`). This is for `lane/ci`.

## Decisions made
- **A child that exits before adopting** counts toward the abandoned-trial streak (`abandon_trial`), so a release that never starts isn't retried forever (as in C-46). **A silent child its creator ended** uses `release_trial` with the new `Undecided::Unadopted`, as briefed, and never counts against the release.
- **`TRIAL_ADOPTION` is 30 s, with a 2 s grace.** The child adopts only after its election, and a cold first start on a slow HDD under antivirus can take many seconds. Ending it early would block the update forever.
- **The handoff byte is held through its own overlapped file object** (`ReOpenFile`), which closes before the slot's. Senders wait for it with the new `file_lock::lock_range`. The frozen slot offsets are unchanged.
- **Trial entry stays v4 with three handles**, since creator and child are one generation. The child signals after adopting and matching its snapshot, before hashing payloads.

## Commits
- `f2139bb4` chore(storage): carry the storage lane's unfinished work to the cloud (a force-push to rewrite it was denied)
- `7ad3db88` merge: bring main into the storage lane
- `0d997540` fix(updates): keep starting the previous release while an update is in progress or briefly held by antivirus
- `14f62816` fix(updates): stop indexers and antivirus scanners from blocking an update's first start, and end an updated release that never takes over

## Verification
- Local: `cargo xtask governance` passed (with the submodule checked out). `cargo test -p sugarsubstitute-contracts`: 299 passed. `cargo fmt --check` passed.
- Windows cross-check (`--target x86_64-pc-windows-msvc`), all passing: clippy `--workspace --all-targets --all-features`, clippy `-p sugarsubstitute --no-default-features`, and rustdoc `-D warnings` for supervision, execution and contracts. The Linux `sugarsubstitute` legs need GTK, which this container lacks.
- CI: [43340890670](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43340890670) and [43341734823](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43341734823) stop at the known submodule baseline on every OS. `lane/ci` is still red, so it isn't merged.

## Next
- **This lane or the integrator:** merge `origin/lane/ci` once it's green and rerun, which is the first real Windows test run.
- **retire:** `pub fn pointed_generations(installation: &Path, context: &JobContext) -> Result<Vec<GenerationId>, PointerFailure>` in `setup_storage`. It returns primary then fallback; a missing pointer gives `Ok(vec![])`; any other error means keep every generation. Reserve `POINTER_RESERVATION_BYTES`.
- **trial:** note the new `Undecided::Unadopted`.
- **ui / coordinator:** nothing is on screen while the headless creator waits for a silent child (up to 32 s). Adopting before the election would shorten the wait.
