# Lane report: host

## Unfinished

- **E6, the Substitute profile, is not recorded.** It must be the real host's answers, from Windows-only owners in `sugarsubstitute`, and Windows tests cannot link yet (lane/ci item 1). `PENDING_HOST_PROFILES.substitute` and its `test.failing` stay; removing them without a recording would fabricate one. Checking against the kit found one real defect, fixed in `4a63d3a5`.

## Unproven

- The integration Windows run failed `a_composition_hosted_page_hears_its_surface_is_unavailable_until_it_can_answer`: the test sent presentation schema 3, which the contract fixes at 1. The test is fixed; a new contract test pins schema 1 and passes on Linux. Unproven on Windows until lane/ci lands the parser build.
- No Windows test of this lane has run: startup updates link, desktop ingress, delivery and messages, production panel and deliveries. All Windows Clippy legs pass locally.
- Native: the SugarCubes Updates entry, the description reaching the real page, and discovery before readiness.

## Decisions made

- The three host facts use the released `sugarcubes.host` v1, not a new schema. `preferencePolicy: saved` means host styles are not declared. `outputSurfacePreference` becomes `output.host-surface`, declared only to composition-hosted browsers. `memoryBudget` becomes the always-declared `sugarcubes.memory`. The page half already read these (`1f558cb5`).
- The description and configure travel as `deliver(a)&&deliver(b)`, so a refused description fails the delivery.
- The bridge coalesced control messages by type alone, which would have dropped a queued update check. It now coalesces by protocol and type, never coalesces named check or restart requests, and caps that backlog at 16.
- The draft's `UpdateControls` became an `UpdateStatusPort` plus a `WorkflowActivitySource`.
- Until coord2's sources land, the idle sources answer up to date, no workflow, check *unavailable*, restart *not ready*. The version shown is the workspace version.
- No fingerprint tool existed; the user chose a reviewed `cargo xtask fingerprints`.
- Baseline fixes (submodule, execution `cfg`) were reverted; lane/ci owns them.

## Taken from `lane/host-d2w-draft`

The `host_updates` contract and `main_updates_contract.rs` were kept (plus schema-binding tests and a public `revision()`); `updates_panel` was reshaped as above.

## Commits

- `2f050f5b` merge: bring main into the host lane
- `7d333ffb` build(xtask): `cargo xtask fingerprints`
- `7ed92b24` feat(host): one host description with every configure
- `93685a01` feat(updates): answer update status, check and restart
- `4a63d3a5` fix(host): unavailable, not unsupported, before the native surface answers
- next: test(host): send the presentation watch at its fixed schema one

Commits up to `94ef3807` were wrongly authored as Claude (the container's preset identity); the coordinator rewrites them. Later commits are under the maintainer's name.

## Verification

- Substitute verification [43343727996](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43343727996) and [43344721034](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43344721034): every OS stops at the known baseline (`download.ts` missing).
- Sugar quality [43343727876](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43343727876): the known baseline only (ubuntu mypy, windows VS2022). No failure is new.
- Local: governance, Windows-target Clippy (three legs), Linux tests (contracts, application, xtask) and 92 Node bridge tests pass.

## Next

- coord2: replace the idle sources in `StartupHost::new` with the real update status, restart-to-apply and queue activity. Feed the caption from the same sources.
- lane/ci: once green, this lane merges it and records E6.
