# Lane report: ci

## Unfinished and handed over

1. **Substitute, Windows test link: done by the maintainer's local agent** (`7ddbc338` links the recorded parser library).
2. **Substitute, Ubuntu and macOS: handed to the Linux lane.** `22ed884d` fixes every hard compile error. About 250 `unreachable_pub` and `dead_code` findings remain in execution and desktop; the app crate isn't linted yet.
3. **Windows composition GPU tests need a real GPU; a software Vulkan ICD cannot pass them.** The two `native_canvas::session::tests::source_io::retained::composition` tests use only adapters that are `compatible` and `shared_textures`:
   - `compatible()` in `sugarsubstitute-image-gpu/src/adapter.rs` rejects CPU-type adapters, which lavapipe and SwiftShader both are.
   - Shared textures need a Vulkan device LUID that matches a hardware DXGI adapter. `hardware_adapters()` in `sugarsubstitute-desktop/src/canvas/graphics.rs` drops `DXGI_ADAPTER_FLAG_SOFTWARE`, and the runner has only WARP.
   - Composition imports D3D11 NT textures through `VK_KHR_external_memory_win32`.
   Passing them would mean changing that product policy, so I added no ICD. Needs a GPU runner (GitHub GPU larger runner or a self-hosted runner), the maintainer's call.

## How lanes name their tests (decision M-4)

A push to `lane/**` runs a focused Windows job: `cargo xtask check`, then the lane's tests. Full `xtask verify` on three platforms runs for pushes to `main` and `integration`, pull requests to `main`, and dispatches with `mode=full`.
- To name tests, commit `LANE-TESTS.txt` at the repository root, one entry per line: `-p <package> [<filter>]`. A `cargo test --locked` prefix is optional, and blank lines and `#` comments are skipped. Each entry runs `cargo test --locked -p <package> --all-features [<filter>]`. The package must be a workspace member, and the filter a plain test path like `coordinator_entry::tests`. Any other argument fails the job.
- Without that file, the job tests every package the push changed. Changes to `Sugar/integrations/contracts` select its consumer packages.
- To run the same job on demand, dispatch Substitute verification with `mode=focused`, a `package` and an optional `filter`.

## Decisions made

- Sugar's Windows leg runs on `windows-2022`, because `windows-latest` now carries only VS 2026.
- Substitute's job timeout is 60 minutes, because Windows Clippy alone takes about 11.
- Windows-only Python uses `sys.platform` guards, so strict mypy passes in every platform view.
- ui's `Active` enum gets a reasoned non-Windows `expect(clippy::large_enum_variant)`, following the repo's allocation convention.
- The caret proof keeps the recorded 400px width and falls back to a measured width under other fonts.
- All commits now carry the maintainer identity, following the coordinator's history rewrite.

## Commits

See `git log origin/main..origin/lane/ci`. Each commit's message states its cause and fix.

## Verification

- Sugar quality is green on ubuntu-latest and windows-2022 at `11311c2f` ([run 43357914629](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43357914629)).
- Substitute verification at `22ed884d` ([run 43343357866](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43343357866)): governance passes on all OSes. On Windows, formatting and the three Clippy legs pass, and tests stop at item 1. Ubuntu and macOS stop at item 2. The coordinator reports a later Windows run passing 1768 of 1774 `sugarsubstitute` lib tests.

## Next

- Merge `lane/ci`. Until items 1 and 2 land, judge Substitute by Windows governance, formatting and Clippy.
