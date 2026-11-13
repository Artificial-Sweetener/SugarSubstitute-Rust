# Lane report: linux

## Unfinished
- **Production host on Linux.** `sugarsubstitute` runs its native splash host on GTK under Xvfb. Setup storage, the launcher and updater, ComfyUI supervision, the main webview, the native canvas, crash capture and pipe IPC remain Windows-only. The launcher, installer, updater and viewer declare `native_adapter_unavailable`, and that is tested. Each mechanism needs its own platform boundary: wry on WebKitGTK, `flock`, Unix sockets, and declared-unsupported crash capture.
- **GTK visibility read-back.** A deferred check on the next loop turn would be stronger than accepting the submitted transition.

## Unproven
- Windows-only behaviour can't run here; CI covers its build, lints and tests.

## Decisions made
- **Branch and identity.** `lane/linux` starts from `origin/lane/ci`. Every commit is authored as Artificial Sweetener, after the coordinator's history rewrite.
- **Merge resolution.** Where both lanes changed a file, `lane/ci`'s compile fixes won and my lint fixes were reapplied. I removed `lane/ci`'s `expect(large_enum_variant)`; the portable glyph cursor boxes its scaler.
- **System counter.** Counter deadlines use one platform-neutral owner: QPC on Windows, `CLOCK_MONOTONIC` in Unix test builds through a narrow libc adapter, and a typed "unsupported" error elsewhere. The Unix adapter is a reviewed test fixture because no Linux production code consumes it yet.
- **GTK behaviour.** tao applies visibility on the next GTK iteration, so the synchronous read-back stays Windows-only. Splash centring uses the inner size, because tao's GTK outer size is unset until the first configure event.
- **Retained ownership.** Values only Windows reads back but that must stay owned (a memory claim, a desktop loan) remain owned on every platform.
- **Visibility.** Portable test builds mount the Windows adapter modules publicly, which fixes `unreachable_pub` at the visibility level instead of allowing it.

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
- **Builds:** the whole workspace on Linux, all targets, with all features and with default features, warning-free.
- **Tests on Linux:** every crate's suite, 61 test binaries with no failures, including all 668 execution tests. Windows-only tests stay Windows-only.
- **Runs:** the splash host, launched under Xvfb in CI; the step requires the first presented frame.
- **Local:** `cargo xtask verify` passes in the container. Clippy `-D warnings` for `x86_64-pc-windows-gnu` (mingw-w64) is clean in all three `xtask verify` legs at `492fb7b8`: workspace all-features, `-p sugarsubstitute`, and `-p sugarsubstitute --no-default-features`.
- **CI:** [43367808883](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43367808883) ran on `23b1971f`: ubuntu-24.04 green, including the Xvfb launch, and macOS-15 green. windows-2022 failed only on the minimal-host `job_deadline` gate, which `90f4320a` fixes. Dispatched run [43372999638](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43372999638) verifies the rewritten head.

## Next
- Merge `lane/linux` after `lane/ci`. They overlap only in the files resolved above.
