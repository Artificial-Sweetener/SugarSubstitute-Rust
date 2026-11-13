# Lane report: host

## Unfinished

- **E6, handed over unfinished.** A Windows recorder (`production/child/panel/conformance_tests.rs`) checks the production owners' answer to each kit message against `conformance_answers.json`. `crates/sugarsubstitute-desktop/tests/host_conformance.mjs` runs the real kit under Node over the real bridge, answering from that file, and recorded `profiles/substitute/0.1.0.json`; SugarCubes' pending entry and `test.failing` are retired, and its 15 profile tests pass. Still to do: storage-inventory records for the two new `.mjs` files (governance fails on that now); a Windows run to confirm the answers, computed on Linux from the same owner logic; CI wiring for the Node harness.

## Unproven

- The integration Windows run failed `a_composition_hosted_page_hears_its_surface_is_unavailable_until_it_can_answer`: the test sent presentation schema 3, which the contract fixes at 1. The test is fixed; a new contract test pins schema 1 and passes on Linux. Unproven on Windows until lane/ci lands the parser build.
- No Windows test of this lane has run: startup updates link, desktop ingress, delivery and messages, production panel and deliveries. All Windows Clippy legs pass locally.
- Native: the SugarCubes Updates entry, the description reaching the real page, and discovery before readiness.

## Decisions made

- The three host facts use the released `sugarcubes.host` v1, not a new schema. `preferencePolicy: saved` means host styles are not declared. `outputSurfacePreference` becomes `output.host-surface`, declared only to composition-hosted browsers. `memoryBudget` becomes the always-declared `sugarcubes.memory`. The page half already read these (`f1677c40`).
- The description and configure travel as `deliver(a)&&deliver(b)`, so a refused description fails the delivery.
- The bridge coalesced control messages by type alone, which would have dropped a queued update check. It now coalesces by protocol and type, never coalesces named check or restart requests, and caps that backlog at 16.
- The draft's `UpdateControls` became an `UpdateStatusPort` plus a `WorkflowActivitySource`.
- Until coord2's sources land, the idle sources answer up to date, no workflow, check *unavailable*, restart *not ready*. The version shown is the workspace version.
- No fingerprint tool existed; the user chose a reviewed `cargo xtask fingerprints`.
- Baseline fixes (submodule, execution `cfg`) were reverted; lane/ci owns them.

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

## Next

- coord2: replace the idle sources in `StartupHost::new` with the real update status, restart-to-apply and queue activity. Feed the caption from the same sources.
- lane/ci: once green, this lane merges it and records E6.
