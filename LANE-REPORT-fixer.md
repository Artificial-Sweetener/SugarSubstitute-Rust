# Lane report: fixer

## Unfinished and unproven
- **Supervision suite hangs on this tree in two tests (not this lane's code).** `update_preparation::tests::startup::cached_trial_channel_and_running_engine_guard_share_one_admission_ledger` and `...::online_reopened_trial_retires_metadata_buffers_before_launch_admission` stall every time, even run alone (four tests, two threads, still running after 120 s; the full probe ran for over 90 minutes). Suspect `e645ccc3` (storage/launcher: the creator waits for the child's slot-held event on the job clock). The fixture's `TestClock` never moves, so the 30 s handoff bound never elapses. Owner: the storage/launcher lane.
- Items 2–3: the cause is still inferred, not reproduced, but the tests are now proven stable (see Verification).

## Decisions made
- **Item 4:** the stores own a `StrikeClock` (system clock in production) instead of taking `now` per call. Strike time belongs to the store, and per-call `now` would touch about 100 call sites in other lanes' files. `Fixed` and `with_strike_clock` are `cfg(test)`.
- **Items 2–3:** `StartupHost` takes `Arc<dyn HostClock>` (`Clock` + `instant_for`). Production callers still pass `MonotonicClock`. Only the close-save `Fixture` (also used by open/admission tests) runs on `ManualClock`.
- **Item 1:** the fix is in the shared test capture, not production.
- I kept the WIP snapshot `07e3e051`. It sits under the layout-conversion merge, so rewriting it would mean redoing that merge.
- I pushed to `lane/fixer`, as the brief says.
- I didn't commit a local non-Windows build fix for execution (another owner's file).

## Commits
- `fd449242` test(canvas): observe input autosaves without opening an archive a save is replacing
- `5986e2eb` test(setup-storage): prove a recovered strike's second is part of its record
- `dce65944` fix(model-library): report an offline preview refresh as one plain warning
- `14c5477f` test(diagnostics): keep captured log lines when another thread registers a callsite first
- `01819cc8` refactor(setup-storage): date strikes and withholding from one injectable store clock
- `72020c32` test(workspace): run the close-save fixture on a clock only the test moves

## Verification
- **Windows, full `sugarsubstitute` lib suite under load:** 50 of 50 runs clean (1778 tests each). This covers items 1–3.
- **Item 4 on Windows:** `abandoned_recovery_waits_for_the_trial_owner_and_not_for_witness_duplicates` passes in the supervision probe.
- **Autosave flake:** the hard-kill test read the archive while the save replaced it. On Windows that fails with access denied (0x80070005), and the retry after the quiet period was never reached on the fixture clock. Reproduced with tracing in round 2 of a 10-worker loop. After the fix, 370 recorded parallel runs of `surface::inputs` passed. The new regression proves an open reader fails the save, the save keeps the previous archive and retries one quiet period later.
- `cargo xtask governance`, `cargo fmt --check`, and clippy `-D warnings` (sugarsubstitute + supervision, all targets and features) pass on Windows.
- `cargo xtask governance` and `cargo fmt --check`: pass. Fingerprints recomputed with the gate's own algorithm.
- `x86_64-pc-windows-gnu`: clippy `-D warnings` passes for all three `sugarsubstitute` gates and supervision (all-targets, all-features). Rustdoc with `-D warnings` is clean.
- Item 1 (standalone crate, same tracing versions, 3 runs): the old capture loses the line, the new one keeps it.
- Sugar: new tests fail before and pass after. `tests/model_library` (585) and the `check:staged` proofs (Jest 353) pass, as do black, ruff and eslint. mypy shows only the baseline.
- CI [43339605267](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43339605267) and [43339605210](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43339605210): only the baseline failures (submodule, VS2022, mypy).

## Next
- **All lanes:** merge `origin/lane/ci` when it is green, then rerun Windows CI here.
- **Execution/desktop owners:** the non-Windows build is broken on main. `runtime_supervision.rs:192` reads the Windows-only `native_deadlines`, and `main_readiness.rs:273` uses the cfg'd-out `main_console`.
- **lane/timeouts:** real-clock test sites remain in `setup_storage`: `selection/tests.rs`, `selection/tests/locks.rs`, `selection/run/tests.rs`, `selection/run/fallback/tests.rs`, `trust/tests.rs`, `trust/updater/recovery/tests.rs`, `user_data/tests.rs`, `signed_installer/tests/fresh/installed/rescue.rs`. They can use `StrikeClock::Fixed`.
- **sugarsubstitute test owners:** 7 other test files build their own tracing capture and should use the shared `Output`.
