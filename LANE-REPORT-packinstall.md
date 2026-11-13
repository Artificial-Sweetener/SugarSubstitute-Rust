# Lane report: packinstall

## Unfinished or unproven
- **Unproven: the headless real run with ComfyUI.** Cloud cannot run it. It should cover a fresh setup, a splash update (registry and GitHub), every verdict including a page failure, and the rollback.
- **Unproven on real Windows:** the junction vectors, `fresh_setup_installs_sugarcubes_as_a_registry_install`, the WinHTTP loopback tests, and every `sugarsubstitute` crate test, which nothing here can link (the child, parent and coordinator wiring).
- **Not shown yet (seam only):** the splash line, the one plain line and the report. They need coord's kind-22 channel; see "Next".

## Decisions made
- **Package:** `sugarcubes/SugarCubes.zip` carries the exact registry archive. The folder form is gone (unreleased).
- **Accessor:** both suites read vectors only through `drive()`, and the xtask enforces it.
- **Lock:** the host takes SugarCubes' own `.sugarcubes/updates/.lock`.
- **Gate:** the update holds source inspection and launch.
- **Readiness:** backend ready counts as an unobservable start. A main reveal counts as the page loading.
- **Page failure:**
  - `SettleAttempt` carries "the page reported SugarCubes failing" (wire 14). Only `SugarcubesUnavailable`/`SugarcubesFailed` count; `TimedOut`/`FrontendUnavailable` give no verdict.
  - The retry is held behind a restore. With no retry, the next start restores.
  - A restored version is never reinstalled.
- **Attached ComfyUI:** read-only check, never written.
- **Schedule:** the coordinator's decisions live in the pure `PackSchedule` (supervision), which Wine can test.

## Commits
Snapshot 9b62b528, merge 7231f5a5. Then ae72d98c (vectors), a6edab9d (setup), 6041e576 (splash update), and 90ae2c3e (page-failure restore and the status seam). Commits before 90ae2c3e were authored as Claude by mistake; the coordinator rewrites them.

## Verification
- Governance and fmt pass. Windows-target clippy `-D warnings` is clean on contracts, application, update, supervision and sugarsubstitute, in all three legs.
- Under Wine: `pack_install` 33 passed and 3 failed (Wine gaps), `lifecycle_protocol` 50/50.
- Linux: contracts 221/221 and application 233 pass. pytest gives 158 passed.
- CI on c255de41 shows only main's baseline failures.

## Next (coord, ui)
- **Coord, splash line:**
  - Add `CoordinatorBackend::pack_update_status()`, returning `self.pack.schedule.status()` (status plus revision).
  - Carry it in kind 22.
  - In the splash, show `PackUpdateStatus::splash_line(messages.pack_updates)` while it's `Updating`.
- **Coord, after the update:** when it's `Finished(report)`, show `report.notice(..)` as the one plain line, and add `report.lines(..)` to the window's update report.
- **Coord, at window start:** `PackUpdateReport::parse` reads `state/pack-updates/SugarCubes/report.json`.
- **Coord, wire version:** both lanes moved the lifecycle wire to 14. The integrator takes 15 for the combined version.
- **release:** ship `SugarCubes-<version>.zip` as `sugarcubes/SugarCubes.zip`.
