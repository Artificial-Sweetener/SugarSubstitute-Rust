# Lane report: linux

## Unfinished
- **Production host on Linux.** `sugarsubstitute` runs its native splash host on GTK under Xvfb. Setup storage, the launcher and updater, ComfyUI supervision, the main webview, the native canvas, crash capture and pipe IPC remain Windows-only. The launcher, installer, updater and viewer declare `native_adapter_unavailable`, and that is tested. Each mechanism needs its own platform boundary: wry on WebKitGTK, `flock`, Unix sockets, and declared-unsupported crash capture.
- **GTK visibility read-back.** A deferred check on the next loop turn would be stronger than accepting the submitted transition.

## Unproven
- Windows-only behaviour can't run here; CI covers its build, lints and tests.

## Decisions made
- **Branch and identity.** `lane/linux` starts from `origin/lane/ci`. Claude-authored commits (`3e439aa2` and earlier) are left for the coordinator to rewrite.
- **Merge resolution.** Where both lanes changed a file, `lane/ci`'s compile fixes won and my lint fixes were reapplied. I removed `lane/ci`'s `expect(large_enum_variant)`; the portable glyph cursor boxes its scaler.
- **System counter.** Counter deadlines use one platform-neutral owner: QPC on Windows, `CLOCK_MONOTONIC` in Unix test builds through a narrow libc adapter, and a typed "unsupported" error elsewhere. The Unix adapter is a reviewed test fixture because no Linux production code consumes it yet.
- **GTK behaviour.** tao applies visibility on the next GTK iteration, so the synchronous read-back stays Windows-only. Splash centring uses the inner size, because tao's GTK outer size is unset until the first configure event.
- **Retained ownership.** Values only Windows reads back but that must stay owned (a memory claim, a desktop loan) remain owned on every platform.
- **Visibility.** Portable test builds mount the Windows adapter modules publicly, which fixes `unreachable_pub` at the visibility level instead of allowing it.

## Commits
- 3e439aa2 build(substitute): lint the workspace clean on Linux and macOS without allowances
- bc3409fe build(execution): drop the duplicated Windows gate on the captured admission test
- 4dd19391 test(execution): qualify capture cleanup on Linux through the system monotonic counter
- 8d69df7a build(substitute): drop duplicated Windows gates left by the platform merge
- 2444ff02 test(supervision): run artifact publication tests from a symlink-free temporary root
- a8b24279 build(image-gpu): keep the shipped application build warning-free off Windows
- c12914f8 fix(desktop): show and centre the splash on Linux, checked by an Xvfb launch in CI
- da245a44 fix(sugarsubstitute): declare the canvas viewer unavailable off Windows instead of exiting silently
- de467a43 build(sugarsubstitute): compile the open-ended job deadline for every Windows feature set

## Verification
- **Builds:** the whole workspace on Linux, all targets, with all features and with default features, warning-free.
- **Tests on Linux:** every crate's suite, 61 test binaries with no failures, including all 668 execution tests. Windows-only tests stay Windows-only.
- **Runs:** the splash host, launched under Xvfb in CI; the step requires the first presented frame.
- **Local:** `cargo xtask verify` passes in the container.
- **CI:** [43367808883](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43367808883) ran on `da245a44`: ubuntu-24.04 green, including the Xvfb launch, and macOS-15 green. windows-2022 failed only on the minimal-host `job_deadline` gate, which `de467a43` fixes; its run is pending.

## Next
- Merge `lane/linux` after `lane/ci`. They overlap only in the files resolved above.
