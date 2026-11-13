# Lane report: ci

## Unfinished and unproven

1. **Substitute, Windows: tests cannot link.** `sugarsubstitute-crash` statically links `committed_dump_parser.lib` (`src/native/committed_parser.rs`), found only through `SUGAR_COMMITTED_PARSER_LIB`. Nothing in the repository builds it (`native/crashpad/BUILD.gn` has no such target), and the Crashpad build script is pinned to the maintainer's PC. Governance, formatting and all three Clippy legs pass on Windows. Needs the maintainer or the crash owner to add a reproducible build (or a reviewed vendored prebuilt) plus CI wiring.
2. **Substitute, Ubuntu and macOS: the workspace never compiled off Windows.** `22ed884d` fixes every hard compile error. About 250 deny-level `unreachable_pub` and `dead_code` findings remain (execution about 207, desktop 41) on items only Windows code reaches. The app crate isn't linted yet, and test results there are unknown. These fixes fall in other lanes' crates, so I stopped (two lanes never write the same files). Recommend one portability lane.

## Decisions made

- Sugar's Windows leg runs on `windows-2022`. `windows-latest` now carries only VS 2026, so the declared VS2022 product is absent there.
- Substitute's job timeout goes from 20 to 60 minutes. Windows Clippy alone takes about 11 minutes, so every run was cancelled before its tests.
- Windows-only Python uses `sys.platform` guards mypy understands, so strict mypy passes for linux, darwin and win32 alike.
- ui's `Active` enum gets a reasoned non-Windows `expect(clippy::large_enum_variant)`, following the repo's no-unadmitted-allocation convention.
- No resync tool exists, so I recomputed edited sources' review fingerprints with the gate's algorithm, checked against each unmodified file's recorded value. The edits are `cfg` additions.

## Commits

- `0427630d` ci(substitute): check out the pinned Comfy Desktop submodule that governance verifies
- `2bb8870d` refactor(platform): type-check Windows-only adapters and GIMP companion launch on every host
- `97206aef` ci(gimp): select the declared VS2022 installation on a runner image that ships it
- `11f77ea9` ci(substitute): give verification the Linux windowing headers and room to finish
- `4109716f` test(sugar): keep governance and canvas browser proofs off the maintainer's drive
- `22ed884d` build(substitute): resolve non-Windows compile errors in platform-gated modules
- `048029fa` build(gimp): judge the D-drive package output rule by Windows path syntax on every host

## Verification

- `22ed884d`: [Sugar quality](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43343357886) is green on windows-2022 (full gate, no tracked-file mutation). Ubuntu passed typing, lint and the first 528 Jest suites, then failed `gimp/contract/package_layout`, which `048029fa` fixes; the rerun is pending. [Substitute verification](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43343357866): governance passes on all OSes; on Windows, formatting and the three Clippy legs pass and tests stop at blocker 1; Ubuntu and macOS stop at blocker 2's lint findings.
- Local: strict mypy in three platform views; changed Jest, pytest, ESLint and Prettier; `cargo xtask governance`; `cargo fmt --check`; Linux `cargo check` without hard errors.

## Next

- Merge `lane/ci`. Until items 1 and 2 land, judge Substitute by Windows governance, formatting and Clippy, and treat the test-link and Linux/macOS failures as baseline.
- `prompt_browser.test.ts` still writes failure screenshots under `D:/SugarCubes`.
