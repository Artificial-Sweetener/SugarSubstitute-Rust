# Lane report: packinstall

## Unfinished or unproven
- **Unproven: the headless real run with ComfyUI.** Not possible in the cloud. Needed: setup fresh install, a splash update (registry and GitHub), and the three verdicts with rollback.
- **Unproven on real Windows:** the junction vectors (`ownership_vectors`, `update_refusal_vectors`), `fresh_setup_installs_sugarcubes_as_a_registry_install` and the WinHTTP loopback tests. Wine lacks junctions, security descriptors and one WinHTTP option. CI can't run them until main's baseline is repaired.
- **Page failure isn't a verdict yet.** `SettleAttempt` carries no cause, so a SugarCubes page failure looks like a user close. The update then stays "being judged" and blocks later updates.
- **Nothing shows the one line or the report yet.** Each run writes `state/pack-updates/SugarCubes/report.json`, and `StartupOutcome::notice`/`report_lines` render the words. Neither reaches the window. The splash also has no "Updating SugarCubes…" line during the wait (bounded at 5 minutes).

## Decisions made
- **Package format:** packages carry the exact registry archive at `sugarcubes/SugarCubes.zip`. Setup extracts it as a registry install does. The `sugarcubes/` folder form is no longer accepted (unreleased, so no migration).
- **One accessor per suite:** both suites read vectors only through `drive("<group>")`. The xtask rejects any driver that bypasses it.
- **Shared lock:** the host takes SugarCubes' own lock (`.sugarcubes/updates/.lock`) with a whole-file `LockFileEx`, which conflicts with Python's byte-0 lock.
- **Coordinator gate:** the update runs in the coordinator backend before source inspection, and it gates launch.
- **Readiness verdict:** backend ready counts as "start unobservable", because readiness can't see node registration. The main reveal counts as the page loading with SugarCubes ready (readiness.rs:130).
- **No repeated failures:** a restored version is never reinstalled. Only a newer one is.
- **Attached ComfyUI:** gets a read-only check that reports "available" and writes nothing.

## Commits
- 9b62b528 chore(packinstall): the local lane's snapshot. It was not reworded, because force-pushing the rewrite was refused here.
- 7231f5a5 merge of origin/main
- ae72d98c test(pack_install): require both suites to read every vector through one accessor
- a6edab9d feat(setup): install SugarCubes from its registry archive, exactly as a registry install
- 6041e576 feat(updates): update SugarCubes during the splash, before the managed backend loads it

## Verification
- `cargo xtask governance` and fmt pass.
- Windows-target clippy `-D warnings` is clean on contracts, update, supervision and sugarsubstitute, in all three feature legs. It ran as a cross check with C objects stubbed.
- Linux clippy fails on main's own `sugarsubstitute-execution` (E0609), before reaching my code.
- Under Wine, `pack_install` gives 28 passed and 3 failed, all Wine gaps.
- Linux focused tests pass: update `http::`, contracts `localization`, xtask `shared_vectors`.
- pytest `tests/pack_install tests/self_update` gives 158 passed.
- `npm run check:staged` stops at the known mypy baseline. black, ruff and mypy are clean on the changed files.
- CI: see below.

## Next steps for other lanes
- **release:** `application-package.mjs` must ship `SugarCubes-<version>.zip` as `sugarcubes/SugarCubes.zip`.
- **coord2:** carry the page's `SugarcubesUnavailable`/`SugarcubesFailed` failure to the coordinator, then queue `TrialEvent::PageFailed` the way `pack_page_loaded` does.
- **coord2 / ui3:** show `report.json` through `notice` and the update report, and add a splash line.
- **integrator:** merge the baseline fix, then get CI green.
