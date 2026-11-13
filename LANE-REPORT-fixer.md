# Lane report: fixer

## Unfinished and unproven
- **Items 1–4 have not run anywhere yet.** The tests are Windows-only. The Linux container can't build `sugarsubstitute` or `sugarsubstitute-execution`, because main's non-Windows build is broken (see "Next"). Wine can't run storage code (`security_descriptor_shape`). CI is red at the known baseline. Proof needed: Windows "Substitute verification" once `lane/ci` is green and merged here, plus repeated full-suite runs for items 1–3.
- **Items 2–3: the root cause is inferred, not reproduced on Windows.** The close record is written by one job bounded by the close's settle cutoff, 15 s on the host's real `MonotonicClock`, and execution has 2 blocking workers. Under load the cutoff passes and the record is dropped (submit or run → `DeadlineExpired`), giving `"Notice"`. The new test reproduces this by moving the clock. If either test still fails on the manual clock, the cause is something else.
- No native proofs needed.

## Decisions made
- **Item 4:** the clock is owned by the store (`StrikeClock`, `SelectionStore`/`TrustStore`, system clock in production) instead of a `now` argument per call. Strike time belongs to the store, and a per-call argument would have touched about 100 call sites in other lanes' files. `Fixed` and `with_strike_clock` exist only under `cfg(test)`.
- `unix_now()` is now `cfg(test)`, kept for test sites outside my brief (listed under "Next").
- **Items 2–3:** `StartupHost` takes `Arc<dyn HostClock>` (`Clock` + `instant_for`). Production callers still pass `MonotonicClock`. Only the close-save `Fixture` (also used by open/admission tests) runs on `ManualClock`.
- **Item 1:** the fix is in the shared capture fixture, not production. I kept the snapshot's reuse of the incident-host capture.
- I kept the WIP snapshot `07e3e051`. It sits under the layout-conversion merge, so rewriting it would mean redoing that merge.
- I pushed to `lane/fixer`, as the brief says.
- I didn't commit the local one-line non-Windows build fix for execution; that file belongs to another owner.

## Commits
- `5e56965b` fix(model-library): report an offline preview refresh as one plain warning
- `0d60d9b5` test(diagnostics): keep captured log lines when another thread registers a callsite first
- `84c43d2f` refactor(setup-storage): date strikes and withholding from one injectable store clock
- `f601ff5f` test(workspace): run the close-save fixture on a clock only the test moves

## Verification
- `cargo xtask governance`: passed. Fingerprints were recomputed with the gate's own algorithm. `cargo fmt --check`: clean.
- Windows target `x86_64-pc-windows-gnu` (mingw): `cargo clippy -D warnings` passed for `sugarsubstitute` (all-features, default and no-default) and for `sugarsubstitute-supervision` (all-targets, all-features). `cargo doc --no-deps` with `RUSTDOCFLAGS=-D warnings`: clean.
- Item 1 mechanism (standalone crate, same tracing versions, 3 runs): the old capture loses the line after an unsubscribed interest rebuild; the shared capture keeps it.
- Sugar: the new tests fail before and pass after. `tests/model_library` passed (585). The selected proofs of `check:staged` passed (Jest 353). black, ruff and eslint passed. mypy shows only main's Linux baseline (22).
- CI [43339605267](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43339605267) and [43339605210](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43339605210): only the baseline failures (submodule, VS2022, mypy).

## Next
- **All lanes:** merge `origin/lane/ci` when it is green, then rerun Windows CI here.
- **Execution/desktop owners:** the non-Windows build is broken on main.
  - `sugarsubstitute-execution/src/runtime_supervision.rs:192` reads the `cfg(any(windows, test))` field `native_deadlines` unconditionally.
  - `sugarsubstitute-desktop/src/main_readiness.rs:273` uses `main_console`, which is cfg'd out.
- **lane/timeouts:** real-clock test sites remain in `setup_storage`: `selection/tests.rs`, `selection/tests/locks.rs`, `selection/run/tests.rs`, `selection/run/fallback/tests.rs`, `trust/tests.rs`, `trust/updater/recovery/tests.rs`, `user_data/tests.rs`, `signed_installer/tests/fresh/installed/rescue.rs`. They can use `with_strike_clock(StrikeClock::Fixed(..))`.
- **sugarsubstitute test owners:** 7 other test files build their own tracing capture. They have the lone-dispatcher hazard and should use `incident_host/tests/diagnostics.rs::Output`.
