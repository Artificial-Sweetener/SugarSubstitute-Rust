# Lane report: linux

## Unfinished
- **Production host on Linux.** The splash host runs on GTK. Setup storage, launcher/updater, ComfyUI supervision, the main webview, native canvas, crash capture and pipe IPC remain Windows-only; their entries declare `native_adapter_unavailable` (tested). Each needs its own Linux boundary (wry/WebKitGTK, `flock`, Unix sockets).
- **GTK visibility.** A deferred read-back on the next loop turn would be stronger.

## Owned by lane/ci
- windows-2022 `cargo test --workspace --all-features`: `sugarsubstitute-crash` (lib test, `portable_history`) fails with LNK1181 `committed_dump_parser.lib` ([run](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43372999638/job/131712910795)). That's the Windows parser build; this lane didn't change it.

## Unproven
- Windows-only behaviour can't run here; CI covers its build, lints and tests.

## Decisions made
- **Branch and identity.** `lane/linux` starts from `origin/lane/ci`. Every commit is authored as Artificial Sweetener, after the coordinator's history rewrite.
- **Merge resolution.** `lane/ci`'s compile fixes won in shared files; lint fixes reapplied on top. `lane/ci`'s `expect(large_enum_variant)` is replaced by boxing the portable glyph scaler.
- **System counter.** One counter-deadline owner: QPC on Windows, `CLOCK_MONOTONIC` (libc adapter, reviewed as a test fixture) in Unix tests, typed "unsupported" elsewhere.
- **GTK behaviour.** tao applies visibility on the next GTK iteration, so the synchronous read-back stays Windows-only; centring uses the inner size because GTK's outer size is unset at creation.
- **Ownership.** Claims and loans only Windows reads back stay owned on every platform.
- **Visibility.** Portable test builds mount Windows adapter modules publicly, fixing `unreachable_pub` without allowances.

## Commits
- 5f435b79 build(substitute): lint the workspace clean on Linux and macOS without allowances
- 4f211ae3 build(execution): drop the duplicated Windows gate on the captured admission test
- c53d3f72 test(execution): qualify capture cleanup on Linux through the system monotonic counter
- ab23e2fd build(substitute): drop duplicated Windows gates left by the platform merge
- c0a908b1 test(supervision): run artifact publication tests from a symlink-free temporary root
- eeb2760d build(image-gpu): keep the shipped application build warning-free off Windows
- d223761e fix(desktop): show and centre the splash on Linux, checked by an Xvfb launch in CI
- 23b1971f fix(sugarsubstitute): declare the canvas viewer unavailable off Windows instead of exiting silently
- 90f4320a build(sugarsubstitute): compile the open-ended job deadline for every Windows feature set

## Verification
- **Builds:** whole workspace on Linux, all targets, all and default features, warning-free.
- **Tests on Linux:** every crate's suite, 61 test binaries with no failures, including all 668 execution tests. Windows-only tests stay Windows-only.
- **Runs:** the splash host, launched under Xvfb in CI; the step requires the first presented frame.
- **Local:** `cargo xtask verify` passes in the container. Clippy `-D warnings` for `x86_64-pc-windows-gnu` (mingw-w64) is clean in all three `xtask verify` legs at `492fb7b8`: workspace all-features, `-p sugarsubstitute`, and `-p sugarsubstitute --no-default-features`.
- **CI:** [43372999638](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43372999638) on `492fb7b8`: ubuntu-24.04 green, including the Xvfb launch; macOS-15 green. windows-2022 passed governance, fmt and all three clippy legs, then failed at test link time on a lane/ci item (below).

## Next
- Merge `lane/linux` after `lane/ci`. They overlap only in the files resolved above.
