# Lane report: ci

## Unfinished and handed over

1. **Substitute, Windows tests: handed to the maintainer's local agent.** `sugarsubstitute-crash` links `committed_dump_parser.lib`. No file in the repository or its history defines `SugarParseCommittedDump`, and `native/crashpad/BUILD.gn` has no `committed_dump_parser` target, so its source must be committed first. Findings for that agent:
   - The crashpad, mini_chromium and googletest pins equal `git archive --prefix=<name>-<rev>/ | gzip -n` of the pinned commits.
   - The zlib and buildtools pins are Gitiles downloads with request-time mtimes, so they are unreproducible. Same trees, reproducible as `git archive | gzip -n`.
   - CIPD `gn/gn/windows-amd64@git_revision:5e19d2fb…` and `infra/3pp/tools/ninja/windows-amd64@version:2@1.8.2.chromium.3` match `build_native.py`'s pins.
2. **Substitute, Ubuntu and macOS: handed to the Linux lane.** `22ed884d` fixes every hard compile error. About 250 `unreachable_pub` and `dead_code` findings remain in execution and desktop; the app crate isn't linted yet.

## Decisions made

- Sugar's Windows leg runs on `windows-2022`, because `windows-latest` now carries only VS 2026.
- Substitute's job timeout is 60 minutes, because Windows Clippy alone takes about 11.
- Windows-only Python uses `sys.platform` guards, so strict mypy passes in every platform view.
- ui's `Active` enum gets a reasoned non-Windows `expect(clippy::large_enum_variant)`, following the repo's allocation convention.
- I recomputed review fingerprints with the gate's algorithm, checked first against each unmodified file.
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
- Substitute verification at `22ed884d` ([run 43343357866](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43343357866)): governance passes on all OSes. On Windows, formatting and the three Clippy legs pass, and tests stop at item 1. Ubuntu and macOS stop at item 2.
- Local: strict mypy in three platform views; all 863 Jest suites on Linux; `cargo xtask governance`; `cargo fmt --check`.

## Next

- Merge `lane/ci`. Until items 1 and 2 land, judge Substitute by Windows governance, formatting and Clippy.
- `prompt_browser.test.ts` still writes failure screenshots under `D:/SugarCubes`.
