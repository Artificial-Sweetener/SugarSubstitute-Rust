# Lane report: coord

## Done
- **update_report_export** (item 2): `StartupHost` exports the update report the window shows to a file the user selects (`startup/update_report.rs`). Each export is a main write through `receiving::ReportFile` in its own scope, plus an independent cleanup scope that closes the write's files whatever the outcome. Export is hidden before execution starts, while closing, and in a host composed without export scopes. Shutdown waits for open exports to settle. Copy and Export write the same text, `report::surface::update_attempt_report`.
- **packinstall seams** (item 3): a host step can hold the splash line, leave lines in the caption, and report a failure. See "Seams exposed".
- Commits for the earlier work are listed under "Commits".

## Seams exposed
These are for lane/packinstall and the host contract. Every `StartupBackend` method has a default, so other backends don't change.
- `update_report_export(&mut self) -> Option<Box<dyn report::surface::UpdateExport>>` (desktop host contract, implemented in `startup/window_content.rs`)
  - It exports the update report the window shows to a file the user selects.
  - It returns `None`, so Export stays hidden, before execution starts, while closing, when no attempt has failed, and in a host composed without export scopes.
  - Process composition reserves its scopes with `StartupHost::configure_update_reports(main, cleanup)` (only on `windows-native-host` or in tests).
- `StartupBackend::startup_step(&self, words: &'static Messages) -> Option<&'static str>`
  - This is the splash line while a host step holds the start, such as "Updating SugarCubes…".
  - It shows as `sugarsubstitute_splash::StartupStatus::Step(&'static str)`.
  - It shows only in place of the Loading, LoadingLong and LoadingExtended lines. The application's own "Updating to version X…" line wins over it, and so do the failure, retry and cleanup lines.
- `StartupBackend::take_step_notice(&mut self, words) -> Option<(&'static str, String)>`
  - This is a one-time caption line: a stable localized name and its text.
  - It shows as `sugarsubstitute_desktop::UpdateNotice::Line { label, text }`, with no link, after the application's own start-up notices.
- `StartupBackend::step_failure(&self) -> Option<UpdateReport>`
  - This is a failed host step's report.
  - It is reported through the update button and the update report, Copy and Export included. It shows only while the application's own updater has no failure and no attempt running.
- **Page-failure event:** this lane did not add one. packinstall's `SettleAttempt` (lane/packinstall 64491633) already carries the page failure to the coordinator. A second path would duplicate it.

## Decisions made
- The export's report text is fixed when Export is requested, from the coordinator's update-status snapshot, so the file holds exactly what the user saw and copied.
- A failed host step is shown through the application's existing update failure surface rather than a new one. The application's own failure or running attempt takes precedence.
- Host-step copy comes from the backend in the caller's language (`words`). The window and splash introduce no literals.
- A waiting restart survives a dropped socket. The last known queue stands until 4 connections in a row fail, or until the backend's origin or lease ends.
- The status shows Applying(X) while this run is an unaccepted trial, and Updated(X) once `update_source()` admits it. "Repairing" comes from `UpdateSelection::repairing` and replaces the rollback line.
- Release notes come from an optional https `{version}` template in the distribution config. It is additive, so the schema stays 2.
- Updates leave 256 MB free. A shortage runs cache maintenance, then measures again. A full disk never resumes within an attempt; C1 backoff is unchanged.
- I merged ui's 0c68a2c7 (C-44) and `origin/lane/ci` early, so CI gets past governance.

## Commits
- 7dab3344 feat(updates): let a host step hold the splash line, leave window lines and report its failure
- 7773cc65 feat(updates): export the update report the window shows to a file the user selects
- 64b4c067 docs(coord): report the coord lane's outcome, decisions, verification and handoffs
- 0c10a026 fix(updates): check free disk space before an update downloads or stages, and report a full disk plainly
- 8bfda98b fix(updates): pass clippy and build update report export only where a host composes it
- 0e32c6ae chore(governance): review the update report export tests' storage access and resync the splash fingerprint
- Earlier: the move into the monorepo layout, lane/ui's start-up messages, the fingerprint resync, and the waiting-restart fix.

## Verification (Windows, MSVC, through the w-coord build wrapper)
- `cargo test --lib startup` (`sugarsubstitute`, at 0e32c6ae): 349 passed, 0 failed. This includes the 5 `update_report::tests` export tests and the 5 host-step `platform_tests::update_notices` tests.
- `cargo test --lib updates` (`sugarsubstitute-desktop`, at 0e32c6ae): 13 passed, 0 failed (12 + 1), including the 10 `main_updates_tests`.
- `cargo test -p` (update crate): 41 passed, 0 failed (35 unit, 6 integration).
- `cargo test` (entry_runner): 26 passed, 0 failed.
- `cargo xtask check` passes at 0e32c6ae: governance, fmt, workspace clippy, and `sugarsubstitute` clippy with and without default features.
- 7dab3344 is an intermediate commit. Its localization fingerprints were recomputed for its own contents, but it was not built on its own.

## Unproven
- Native proofs still needed:
  - Export writing a real file from the update report;
  - the host-step splash line and caption line in the real app;
  - the "Updating to version X…" splash during a trial;
  - the caption notices;
  - the queue watch reconnecting after ComfyUI restarts, managed and attached;
  - a real full disk.
- The C7 tests that use a real cache have not run.

## Next
- **lane/packinstall:** implement `startup_step`, `take_step_notice` and `step_failure` on the production backend from its SugarCubes install state.
- **lane/host:** the child's update-status source is `CoordinatorUpdates` (`production/child/channel/update_status.rs`).
  - It offers `status()`, `workflow()` and `take_notice()`, fed by revision through kind 22.
  - Map restart to `request_restart_to_apply`.
- **retire (C6):** pass old-version retirement as a `setup_storage::Reclaim` to `Download::open_with`.
- **lane/ci and the integrator:**
  - build `committed_dump_parser` on the Windows runner;
  - install `libdbus-1-dev` on ubuntu;
  - fix the Linux build of `sugarsubstitute-execution`.
