# Lane report: timeouts

## Unfinished
- **Setup-storage and crate tests haven't run anywhere.** They need real Windows files. lane/ci reports that Windows test binaries can't link yet (`committed_dump_parser.lib`), and that Ubuntu and macOS stop on lint. Linux can't build `sugarsubstitute-execution` here either (`runtime_supervision.rs:192`).
- **lane/ci isn't merged.** Its report doesn't yet show both workflows green, which CLOUD-RULES requires. When it is merged, keep this lane's 240-minute hang guard over lane/ci's 60 minutes.

## Unproven
Native qualification under the no-progress guard:
- the process, held, pipe, output, descendant, claim and capture campaigns;
- both setup qualifications.

## Decisions made
- **History.** Force-pushing was denied, so `946b22e2` merges the snapshot with a linear rebuild on main, with the same tree.
- **The 13 combined-tree hits** (origin/integration, with fixer's StrikeClock):
  - Tests that pass a time now use a fixed one.
  - The fixtures that own their stores take `StrikeClock::Fixed` and assert the strike's date.
  - Where the product opens its own stores on the system clock (updater staging, the first-install rescue), the test checks withholding at the strike's recorded date. A strike's `count` still decides whether it withholds.
  - The test-only `unix_now()` and its re-exports are gone.
  - The close-save `ManualClock` maps instants from the frozen anchor.
  - Nothing is allowlisted.
- **CI timeout.** `timeout-minutes: 240` is set in `substitute-verify.yml`.

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
- 69b848f1 merge of origin/integration
- 2c35b75b test(workspace): map the close-save fixture's native instants from the frozen anchor
- 7954b3e6 test(setup-storage): date every setup-storage test from a fixed clock

## Verification
On the combined tree:
- **Governance:** `cargo xtask governance` passes, gate 1 included. Fingerprints were recorded with `cargo xtask fingerprints`.
- **fmt:** passes.
- **xtask tests:** 192 pass.
- **Windows-target clippy `-D warnings`, all three legs:** clean. This type-checks every test target; `ring` was built against stub C headers.
- **Before the merge:** Windows Rustdoc clean; FamilySettlement Python tests pass.
- **CI:** combined-tree [run 43361365145](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43361365145) stops on all OSes at the submodule baseline, which integration still lacks. Nothing new.

## Next
- **Integrator:** merge lane/timeouts. Gate 1 passes on integration plus this branch.
- **lane/ci or a portability lane:** the Windows test link and the non-Windows build.
- **Then:** run the setup_storage and crate suites on Windows CI.
