# Lane report: host

## Unfinished

- No CI job runs the Node harnesses (`host_conformance.mjs`, `panel_bridge.mjs`); that wiring belongs to lane/ci. SugarCubes CI replays the recorded profile.

## E6: the Substitute conformance profile

`production/child/panel/conformance_tests.rs` checks the production owners' answer to each kit message against `conformance_answers.json`. `crates/sugarsubstitute-desktop/tests/host_conformance.mjs` runs the real kit under Node over the real bridge, answering from that file, and recorded `profiles/substitute/0.1.0.json`. SugarCubes no longer lists Substitute as pending, and its profile test is a normal test. Both harness scripts have storage and cache records, and `sugarcubes-conformance` declares the kit files the harness consumes.

## Unproven

- Native: the SugarCubes Updates entry, the description reaching the real page, and discovery before readiness.

## Decisions made

- The three host facts use the released `sugarcubes.host` v1, not a new schema. `preferencePolicy: saved` means host styles are not declared. `outputSurfacePreference` becomes `output.host-surface`, declared only to composition-hosted browsers. `memoryBudget` becomes the always-declared `sugarcubes.memory`. The page half already read these (`f1677c40`).
- The description and configure travel as `deliver(a)&&deliver(b)`, so a refused description fails the delivery.
- The bridge coalesced control messages by type alone, which would have dropped a queued update check. It now coalesces by protocol and type, never coalesces named check or restart requests, and caps that backlog at 16.
- The draft's `UpdateControls` became an `UpdateStatusPort` plus a `WorkflowActivitySource`.
- Until coord2's sources land, the idle sources answer up to date, no workflow, check *unavailable*, restart *not ready*. The version shown is the workspace version.
- No fingerprint tool existed; the user chose a reviewed `cargo xtask fingerprints`.
- Baseline fixes (submodule, execution `cfg`) were reverted; lane/ci owns them.
- The conformance recorder never compiled before its Windows run (it is Windows-only); it was missing the `SurfaceReply` import.
- The harness reaches SugarCubes' kit through a new shared component, `sugarcubes-conformance`, rather than widening `sugarcubes-contracts`. The recorder's doc names the kit by its pack path so only the harness consumes that component.

## Taken from `lane/host-d2w-draft`

The `host_updates` contract and `main_updates_contract.rs` were kept (plus schema-binding tests and a public `revision()`); `updates_panel` was reshaped as above.

## Commits

- `3590a29d` merge: bring main into the host lane
- `1e785eae` build(xtask): `cargo xtask fingerprints`
- `7d4467eb` feat(host): one host description with every configure
- `9a891957` feat(updates): answer update status, check and restart
- `138b247e` fix(host): unavailable, not unsupported, before the native surface answers
- `f67ee7df` test(host): send the presentation watch at its fixed schema one

The coordinator rewrote this lane's early commits, which had been authored as Claude, to the maintainer's name.

## Verification

- Substitute verification [43343727996](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43343727996) and [43344721034](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43344721034): every OS stops at the known baseline (`download.ts` missing).
- Sugar quality [43343727876](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43343727876): the known baseline only (ubuntu mypy, windows VS2022). No failure is new.
- Local: governance, Windows-target Clippy (three legs), Linux tests (contracts, application, xtask) and 92 Node bridge tests pass.
- Windows: `sugarsubstitute` `production::child::panel` 32/32 (the presentation schema test and the conformance recorder included); `startup::updates_page`, `startup::routing`, `startup::memory`, `panel_host` 18/18; `sugarsubstitute-desktop` `main_actor::panel`, `main_updates_contract`, `main_panel`, `main_native_delivery`, `main_memory_contract` 54/54; `sugarsubstitute-contracts` all pass. Node 22: `host_conformance.mjs` 1/1, `panel_bridge.mjs` 92/92. SugarCubes `recorded_host_profiles` 7/7.

## Next

- coord2: replace the idle sources in `StartupHost::new` with the real update status, restart-to-apply and queue activity. Feed the caption from the same sources.
- lane/ci: once green, this lane merges it and records E6.
