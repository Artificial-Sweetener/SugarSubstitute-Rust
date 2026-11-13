# Lane report: coord

## Unfinished
- None of my items is open. The packinstall wiring is the integrator's step (see Next).

## Unproven
- No Windows test of `sugarsubstitute`, the desktop, the splash or the real-cache C7 tests has run anywhere. Wine can't run them (DirectWrite, security descriptors, the MSVC-only `committed_dump_parser`), and CI can't link them yet.
  - These include the new export tests (a real file round trip, a confirmed overwrite, refusals).
- Native proofs: the "Updating to version X…" and "Updating SugarCubes…" splash lines, the caption notices, exporting through the shell Save dialog, the queue watch reconnecting after ComfyUI restarts, and a real full disk.

## Decisions made
- **Branch**: rebuilt on origin/integration without lane/ci. I dropped my optional release-notes field; integration's required `ReleaseNotes` owns the "What's new" link.
- **Export**: each report gets its own execution owner on the report window's wake, using the report-export admission plus `receiving::ReportFile`. It writes the report's `to_text()`, the same text Copy uses.
- **Page failure**: new lifecycle kind 24 `PageFailed` (Unavailable or SetupFailed), sent once per start. Lifecycle stays v14, which is unreleased. The child's control queue grows from 5 to 6.
- **SugarCubes status**: `PackActivity` travels in the update status (kind 22). Its line arrives already localized by the coordinator, because packinstall owns those words. "Updating SugarCubes…" is new copy in `updates.rs`.
- **Identity**: my earlier commits went out as `Claude <noreply@anthropic.com>`, the container's preset, which I never checked. The checkout now commits as Artificial Sweetener. I recreated my two unpushed commits; pushed history waits for the coordinator's rewrite.

## Commits since the rebuild
- 7c578eaf merge: bring the integration branch into the coord lane
- a09e0ab1 feat(updates): export the update report to the file the user chooses
- 670c49f7 test(updates): build the panel's recorded update state from its defaults
- 5c280ad6 feat(updates): tell the coordinator when the SugarCubes page fails, and say on the splash that SugarCubes is updating

Earlier: 455ad5f3, f33d3298, 7f3cfa37, 07029e40, 8eed7548, 1896c9d1 (layout, queue watch, notices, C7).

## Verification
- `cargo xtask governance` and `cargo fmt --check` pass.
- Strict Clippy for the Windows MSVC target passes on all three legs, as a cross-check without linking.
- Linux: contracts 319/319 and the application's updates panel 10/10 pass.
- Wine: lifecycle protocol 53/53, websocket 9/9, queue watch 3/3, free-space checks 5/5.
- CI [run 43370543732](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43370543732) at fcf54146: only main's baseline. Without lane/ci, the comfy-desktop submodule isn't checked out, so governance stops on all three OSes before compiling. Nothing new from this lane.

## Next
- **Integrator, packinstall (F3w)**:
  - Feed `TrialEvent::PageFailed` where `entry_runner/updates.rs` logs `sugarcubes_page_failed`. That reads `ParentLifecycle::take_page_failure`.
  - Add `StatusSource::observe_pack` mirroring `observe_launch`.
  - Call it with `PackActivity::Updating` before the pack update and `Finished { line: outcome.notice(words), report }` after.
  - Opening `UpdateStatus::pack_report` from the caption is ui work.
- **lane/host**: the `updates_panel` already reads `UpdateStatus`. Its new facts are `repairing`, `pack` and `launch`.
- **lane/ci and the integrator**: link `committed_dump_parser` on Windows CI, then run this lane's tests.
