# Lane report: timeouts

## Unfinished
- **Gate 1 fails on 18 fixer-owned files.** Their allowlist entries stay empty, as the brief asked. All are real stored-versus-fresh shapes (e.g. `finish_trial` stamps `unix_now()` at `selection/run/settlement.rs:90`; the test asserts `withheld(pkg, unix_now())`), so setup_storage needs an injected wall clock. Under `crates/sugarsubstitute-supervision/src/setup_storage/`:
  - `user_data/tests.rs`, `selection/tests.rs`, `selection/tests/locks.rs`, `selection/run/{tests,fallback/tests,settlement/tests}.rs`
  - `trust/tests.rs`, `trust/updater/recovery/tests.rs`, `updater_package/stage/trial/tests.rs`
  - `signed_installer/tests/fresh/installed/rescue.rs`, `test_package/maintenance.rs`
  - `packed/tests/downloaded/authorization/{candidates,launch}.rs` and `launch/{inheritance,maintenance,recovery,reservation,running/image}.rs`
- **The crate test suites have not run:** CI stops at main's governance baseline, and main doesn't build on Linux (see "Found").

## Unproven
Native qualification under the no-progress guard:
- the process, held, pipe, output, descendant, claim and capture campaigns;
- both setup qualifications.

## Decisions made
- **History.** Force-pushing was denied, so `946b22e2` merges the snapshot with a linear rebuild on main (same tree). Each rebuilt commit passes governance on its own, except the gate commit's 18 hits.
- **Conflicts.** The three governance files come from a 3-way apply of the lane's delta, with fingerprints resynced by xtask's algorithm.
- **CI.** `timeout-minutes: 240` moved to `substitute-verify.yml`; old `verify.yml` dropped.

## Commits
- 9f3e5c7c test(execution): give tests one frozen time anchor and a shared no-progress guard
- 785476f4 test: wait for observed events under the shared no-progress guard instead of fixed timeouts
- 7455da76 test: derive test instants from the frozen anchor instead of the real clock
- 33497147 test(qualification): end native fixture campaigns on lost progress instead of a 60-second deadline
- 26fc3fcd test(setup): qualify the real installer by its outcome under the no-progress guard
- cc2a81e5 test(tools): settle updater qualification on progress and stop the feed only by its control file
- 0e7a1ef0 ci: let a cold three-OS Substitute verification finish
- 7d653758 build(xtask): reject wall-clock pass/fail in tests
- 946b22e2 merge: move lane/timeouts into main's monorepo layout under Substitute
- 83ed0c46 docs(execution): let qualification_time's module docs resolve their own links

## Verification
- **xtask tests:** 185 pass, including the 6 gate fixtures. These reject the stored-versus-fresh shape for both `unix_now()` and `SystemTime::now()`.
- **Governance:** passes with the 18 registered by an uncommitted probe, so there are no other hits or unused registrations.
- **fmt:** passes.
- **Windows-target type checks** (`ring` built against stub C headers):
  - clippy `-D warnings`, all three legs: clean.
  - Rustdoc `-D warnings`: clean after 83ed0c46.
- **Python:** the FamilySettlement tests pass with the Windows imports stubbed.
- **CI:** [run 43340327438](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43340327438) fails on all three OSes at main's baseline (`resolve third_party/comfy-desktop/...download.ts`). Nothing new.
  - The "Sugar quality" run was still running; `Sugar/` is identical to main.
  - No load-only failures could be observed: no tests ran.

## Found (not this lane)
Main doesn't build on Linux or macOS:
- `sugarsubstitute-execution` reads a `cfg(any(windows, test))` field in `runtime_supervision.rs:192`.
- Under `test`, its Windows modules need the `windows` crate.
- `sugarsubstitute-desktop` uses the Windows-only `crate::main_console`.

## Next
- **fixer:** inject a clock into setup_storage and convert the 18 files.
- **lane/ci:** fix the build break above.
- **This lane:** merge `origin/lane/ci` once it is green, then get CI green.
