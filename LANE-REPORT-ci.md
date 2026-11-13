# Lane report: ci

## Unfinished and handed over

1. **Substitute, Windows test link: done by the maintainer's local agent** (`7ddbc338` links the recorded parser library).
2. **Substitute, Ubuntu and macOS: handed to the Linux lane.** `22ed884d` fixes every hard compile error. About 250 `unreachable_pub` and `dead_code` findings remain in execution and desktop; the app crate isn't linted yet.
3. **Windows composition GPU tests need a real GPU; a software Vulkan ICD cannot pass them.** The two `native_canvas::session::tests::source_io::retained::composition` tests use only adapters that are `compatible` and `shared_textures`:
   - `compatible()` in `sugarsubstitute-image-gpu/src/adapter.rs` rejects CPU-type adapters, which lavapipe and SwiftShader both are.
   - Shared textures need a Vulkan device LUID that matches a hardware DXGI adapter. `hardware_adapters()` in `sugarsubstitute-desktop/src/canvas/graphics.rs` drops `DXGI_ADAPTER_FLAG_SOFTWARE`, and the runner has only WARP.
   - Composition imports D3D11 NT textures through `VK_KHR_external_memory_win32`.
   Passing them would mean changing that product policy, so I added no ICD. Needs a GPU runner (GitHub GPU larger runner or a self-hosted runner), the maintainer's call.

## Decisions made

- Sugar's Windows leg runs on `windows-2022`, because `windows-latest` now carries only VS 2026.
- Substitute's job timeout is 60 minutes, because Windows Clippy alone takes about 11.
- Windows-only Python uses `sys.platform` guards, so strict mypy passes in every platform view.
- ui's `Active` enum gets a reasoned non-Windows `expect(clippy::large_enum_variant)`, following the repo's allocation convention.
- The caret proof keeps the recorded 400px width and falls back to a measured width under other fonts.
- Commits before `11311c2f` carry the container's default "Claude" identity, which I failed to check. Later commits use the maintainer identity; the coordinator rewrites the earlier ones.

## Commits

- `0427630d` ci(substitute): check out the pinned Comfy Desktop submodule that governance verifies
- `2bb8870d` refactor(platform): type-check Windows-only adapters and GIMP companion launch on every host
- `97206aef` ci(gimp): select the declared VS2022 installation on a runner image that ships it
- `11f77ea9` ci(substitute): give verification the Linux windowing headers and room to finish
- `4109716f` test(sugar): keep governance and canvas browser proofs off the maintainer's drive
- `22ed884d` build(substitute): resolve non-Windows compile errors in platform-gated modules
- `048029fa` build(gimp): judge the D-drive package output rule by Windows path syntax on every host
- `09ffb330`, `13cc1c06`, `11311c2f` test(prompt-editor): find the caret proof's wrapping width under each platform's font

## Verification

- Sugar quality is green on ubuntu-latest and windows-2022 at `11311c2f` ([run 43357914629](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43357914629)).
- Substitute verification at `22ed884d` ([run 43343357866](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43343357866)): governance passes on all OSes. On Windows, formatting and the three Clippy legs pass, and tests stop at item 1. Ubuntu and macOS stop at item 2. The coordinator reports a later Windows run passing 1768 of 1774 `sugarsubstitute` lib tests.
- Local: strict mypy in three platform views; all 863 Jest suites on Linux; `cargo xtask governance`; `cargo fmt --check`.

## Next

- Merge `lane/ci`. Until items 1 and 2 land, judge Substitute by Windows governance, formatting and Clippy.
