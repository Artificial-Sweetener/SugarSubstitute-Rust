# Lane report: coord

## Unfinished
- **update_report_export** (item 2) is not built. It needs `report_export_admission` plus `submit_report_export` with a paired cleanup job, writing through `receiving::ReportFile`, plus native proof. Until then Export stays hidden and Copy works, which is the contract's default.

## Unproven
- Tests that have not run anywhere: desktop, `sugarsubstitute`, and the C7 tests that use a real cache. Wine can't run them (security descriptors, MSVC-only library), and CI can't link the test binaries.
- Native proofs still needed:
  - the "Updating to version X…" splash during a trial;
  - the caption notices;
  - the queue watch reconnecting after ComfyUI restarts, managed and attached;
  - a real full disk.

## Decisions made
- A waiting restart now survives a dropped socket. The last known queue stands until 4 connections in a row fail, or until the backend's origin or lease ends.
- The status shows Applying(X) while this run is an unaccepted trial, and Updated(X) once `update_source()` admits it. "Repairing" comes from `UpdateSelection::repairing` and replaces the rollback line.
- Release notes come from an optional https `{version}` template in the distribution config. It is additive, so the schema stays 2.
- Updates leave 256 MB free. A shortage runs cache maintenance, then measures again. A full disk never resumes within an attempt; C1 backoff is unchanged.
- I merged ui's 0c68a2c7 (C-44) and `origin/lane/ci` early, so CI gets past governance.

## Commits
- 455ad5f3 merge: move lane/coord into main's monorepo layout
- f33d3298 merge: bring in lane/ui's start-up update messages
- 7f3cfa37 chore(governance): resync fingerprints after the move
- 07029e40 fix(updates): keep a waiting restart through a short loss of the queue connection
- 8eed7548 feat(updates): window/splash messages for updated, repairing, previous version
- 1896c9d1 fix(updates): check free disk space before download and staging
- 80c37dd3 merge: bring in lane/ci's baseline CI repair

The snapshot 1c775ed6 stays as it is.

## Verification
- `cargo xtask governance` and `cargo fmt --check` pass. On Linux the contracts and update suites pass; `sugarsubstitute-execution` doesn't build on Linux, on main either.
- Under Wine:
  - websocket 9/9
  - queue_watch 3/3
  - contracts update and localization 40/40
  - free-space units 5/5
- CI [run 43341820345](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43341820345) at 80c37dd3: windows passes governance, fmt and all three Clippy legs, then `cargo test` can't link `committed_dump_parser.lib` (baseline). ubuntu is missing `libdbus-1-dev` (baseline).

## Next
- **lane/host:** the child's update-status source is `CoordinatorUpdates` (`production/child/channel/update_status.rs`).
  - It offers `status()`, `workflow()` and `take_notice()`, fed by revision through kind 22.
  - Map restart to `request_restart_to_apply`, which already waits for the workflow. Add `revision()` when you need it.
- **retire (C6):** pass old-version retirement as a `setup_storage::Reclaim` to `Download::open_with`.
- **Small edits outside my files:** a 3-line splash hook in `startup.rs`, a trait method and its fake in `startup/backend.rs` and `platform_tests.rs`, `UpdateNotice::Repairing` in the desktop, and a `release_notes` field in the update distribution.
- **lane/ci and the integrator:** build `committed_dump_parser` on the Windows runner, install `libdbus-1-dev` on ubuntu, and fix the Linux build of `sugarsubstitute-execution`. Then rerun this lane's tests.
