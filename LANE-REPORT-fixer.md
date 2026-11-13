# Lane report: fixer

## Unfinished and unproven
- **Items 1–4 have not run yet.** The tests are Windows-only, main's non-Windows build is broken (see "Next"), Wine can't run storage code, and CI is red at the baseline. Needed: Windows CI after merging a green `lane/ci`, with repeated full-suite runs for items 1–3.
- **Items 2–3: root cause inferred, not reproduced on Windows.** One job writes the close record, bounded by the settle cutoff (15 s on the real clock), and execution has 2 blocking workers. Under load the cutoff passes and the record is dropped, giving `"Notice"`. A new test reproduces this by moving the clock. If either test still fails on the manual clock, the cause is something else.
- No native proofs needed.

## Decisions made
- **Item 4:** the stores own a `StrikeClock` (system clock in production) instead of taking `now` per call. Strike time belongs to the store, and per-call `now` would touch about 100 call sites in other lanes' files. `Fixed` and `with_strike_clock` are `cfg(test)`.
- **Items 2–3:** `StartupHost` takes `Arc<dyn HostClock>` (`Clock` + `instant_for`). Production callers still pass `MonotonicClock`. Only the close-save `Fixture` (also used by open/admission tests) runs on `ManualClock`.
- **Item 1:** the fix is in the shared test capture, not production.
- I kept the WIP snapshot `07e3e051`. It sits under the layout-conversion merge, so rewriting it would mean redoing that merge.
- I pushed to `lane/fixer`, as the brief says.
- I didn't commit a local non-Windows build fix for execution (another owner's file).

## Commits
- `dce65944` fix(model-library): report an offline preview refresh as one plain warning
- `14c5477f` test(diagnostics): keep captured log lines when another thread registers a callsite first
- `01819cc8` refactor(setup-storage): date strikes and withholding from one injectable store clock
- `72020c32` test(workspace): run the close-save fixture on a clock only the test moves

## Verification
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
