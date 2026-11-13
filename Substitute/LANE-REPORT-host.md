# Lane report: host

## Unfinished

- **E6, the Substitute profile, is not recorded.** The profile must be the real Rust host's answers, composed by the startup and production-child owners in `sugarsubstitute`. That crate builds only for Windows, where tests cannot link yet (lane/ci item 1). `PENDING_HOST_PROFILES.substitute` and its `test.failing` therefore stay as they are; removing them without a recording would fabricate one. Once Windows tests link, a Windows test feeds the kit's page messages through those owners and saves the transcript as `profiles/substitute/0.1.0.json`. Checking against the kit found one real defect, fixed in `4a63d3a5`.

## Unproven

- No Windows test of this lane has run: startup updates link, desktop ingress, delivery and messages, production panel and deliveries. All Windows Clippy legs pass locally.
- Native: the SugarCubes Updates entry, the description reaching the real page, and discovery before readiness.

## Decisions made

- The three host facts use the released `sugarcubes.host` v1, not a new schema. `preferencePolicy: saved` means host styles are not declared. `outputSurfacePreference` becomes `output.host-surface`, declared only to composition-hosted browsers. `memoryBudget` becomes the always-declared `sugarcubes.memory`. The page half already read these (`1f558cb5`).
- The description and configure travel as `deliver(a)&&deliver(b)`, so a refused description fails the delivery.
- The bridge coalesced control messages by type alone, which would have dropped a queued update check. It now coalesces by protocol and type, never coalesces named check or restart requests, and caps that backlog at 16.
- The draft's `UpdateControls` became an `UpdateStatusPort` plus a `WorkflowActivitySource`. The channel comes from the subscription in the snapshot.
- Until coord2's sources land, the idle sources answer up to date, no workflow, check *unavailable*, restart *not ready*. The version shown is the workspace version.
- No fingerprint tool existed; the user chose a reviewed `cargo xtask fingerprints`.
- I reverted my submodule and execution `cfg` fixes because the baseline belongs to lane/ci.

## Taken from `lane/host-d2w-draft`

- `host_updates.rs`, `host_updates/wire.rs` and `host_updates/tests.rs`: kept, plus schema-binding tests.
- `updates_panel.rs` and its tests: reshaped as above.
- `main_updates_contract.rs`: kept; `revision()` is now public.

## Commits

- `2f050f5b` merge: bring main into the host lane
- `7d333ffb` build(xtask): record reviewed source fingerprints with `cargo xtask fingerprints`
- `7ed92b24` feat(host): tell SugarCubes what Substitute serves in one host description with every configure
- `93685a01` feat(updates): answer the page's update status, check and restart requests
- `4a63d3a5` fix(host): tell a composition-hosted page its output surface is unavailable, not unsupported, before the native surface can answer

## Verification

- Substitute verification [43343727996](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43343727996) failed on all three OSes at the known baseline only (`download.ts` missing). There are no new failures.
- Local: `cargo xtask governance` passes. Windows-target Clippy passes all three legs. Linux tests pass: contracts, application and xtask (186). The 92 Node bridge tests pass.

## Next

- coord2: replace the idle sources in `StartupHost::new` with the real update status, restart-to-apply and queue activity. Feed the caption from the same sources.
- lane/ci: once green, this lane merges it and records E6.
