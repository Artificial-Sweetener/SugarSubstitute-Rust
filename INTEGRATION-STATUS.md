# Integration status

Branch `integration`, created from `origin/main` 0cd6362f. Updated by each cloud integration pass.

## Pass 5 (2026-11-13)

Tools that worked: git fetch/push over the proxy; rustup 1.93.1 with clippy, rustfmt and the
`x86_64-pc-windows-gnu` target; apt for mingw-w64, `libgtk-3-dev`, `libdbus-1-dev` and
`pkg-config`; Node 22 with `npm ci` in `Sugar/`; uv for a Python 3.12 venv (`pip install -e
".[dev]"`); the public Actions REST API through curl; the GitHub MCP `get_job_logs`. Not working:
`gh` (token rejected).

Commit identity: passes 1–4 committed as the container's preset "Claude" identity, against
CLOUD-RULES' maintainer rule. From this pass on, integration commits are authored and committed as
the maintainer. The coordinator's authorship rewrite needs to cover integration's earlier commits
too.

History rewrite: while this pass ran, integration, main and every lane branch were rewritten
under the maintainer identity (integration 2c55674d became ccccccc5 with the same tree). I rebuilt
this pass's merges on the rewritten history. Their trees are identical to the ones checked below,
apart from the host and entry reports, which now cite rewritten hashes. Hashes in this section are
the rewritten ones; CI run links point at runs on the pre-rewrite commits with the same trees.
Earlier passes' hashes are pre-rewrite, and their merge commits now have new hashes (timeouts merge
a0e1b56b is d6311f04). The "Commit identity" note below is resolved by that rewrite.

### Integration tip and CI

- New merges: host at a83420b7 (f22025cd), entry at bb026c95 (a3d80408), packinstall at 62a5bae3
  (8b542f46). No product fixes were needed.
- Merge resolution: only governance records conflicted. I kept both sides' new records (storage:
  owned_retirement and pack_tree/pack_install fixtures; configuration: fingerprints/update-repository
  and shared_vectors_tests) and both `durable_storage` modules (`owned_retirement`, `pack_tree`).
  I re-recorded source fingerprints with `cargo xtask fingerprints`. That tool doesn't cover
  `[[dependencies]]` reviews, so I took their digests from `cargo xtask storage-inventory`, the
  gate's own inventory: entry adds `Win32_UI_WindowsAndMessaging` to execution's `windows`
  features (for the responsiveness probe), and packinstall's `Cargo.lock` changes every manifest
  digest. All 18 reviewed dependencies still match 18 actual ones, with none added or removed.
- Local checks on the merged tip: `cargo xtask governance` passed (submodule checked out);
  `cargo fmt --check` passed; Windows-target Clippy (`-D warnings`) passed all three legs;
  `cargo test --locked -p sugarsubstitute-contracts -p sugarsubstitute-application -p
  sugarsubstitute-xtask` passed (host's new schema-one contract test included; xtask 200).
  `Sugar/` changed (packinstall's shared install vectors, `digest_manifest.py`, `tracking.py`).
  `npm run check:staged`, run with those changes staged on the previous tip, stops at the known
  mypy baseline ("Found 22 errors in 11 files"). The 158 tests in `tests/pack_install` and
  `tests/self_update` pass, and ruff and black pass on the changed areas. CI runs the full suite.
- Previous tip 2c55674d, now ccccccc5: [Substitute verification](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43363849901) is baseline only (`download.ts`
  on every OS).
- CI for the new tip starts with this push:
  [integration runs](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions?query=branch%3Aintegration).
- Wiring: retire to storage was done in pass 1. Host `sugarcubes.updates` to coord is **still
  pending, because coord isn't merged**. packinstall's splash line, one plain line and report
  (`PackUpdateStatus`, `PackUpdateReport`) are also waiting on coord's kind-22 channel; both lanes
  moved the lifecycle wire to 14, so the combined wire must become 15 when coord merges. Until
  then integration has **two placeholders that must not ship**: host's idle update sources, and a
  splash-time SugarCubes update of up to 300 s with nothing on screen (HANDOFF §7).

### Lanes

Moved since pass 4: ci, release, host, entry and packinstall. None is stalled: every tip has moved
from its starting snapshot. coord, storage, trust, retire, timeouts and fixer haven't moved.

Branches outside this pass's lane list, not reviewed or merged: `lane/linux` (492fb7b8;
[run 43367808883](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43367808883): ubuntu and macOS **green**, Windows fails), `lane/ci-next`
(00bacaf9, lane/ci plus lane/linux; [run 43367994736](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43367994736): ubuntu and macOS **green**,
Windows fails Clippy `-p sugarsubstitute --no-default-features` because `crate::job_deadline` is
gated behind `native-composition`), and `lane/launcher` (3d6f2f0b; [run 43368450801](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43368450801)
fails on every OS). They need the coordinator to list them first.

| Lane | Tip | State | Last reviewed | Merged | Stalled | Blocking reasons / unproven |
|---|---|---|---|---|---|---|
| ci | 5de2f01b | working | 5de2f01b | no | no | Report still doesn't say both workflows are green. [Substitute verification](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43365354840) on 7ddbc338, now f7d74163 (the parser link): Windows tests now **link and run**, and `sugarsubstitute` lib gives 1768 passed and 6 failed: two composition GPU tests (need a hardware GPU runner, **the maintainer's call**), three `setup::worker::records` tests ("Record parent must use its resolved local location") and `input_documents::tests::storage::bases::only_originals_no_kept_reference_names_are_selected_for_retirement`. Nobody has explained or assigned those four non-GPU failures yet. Ubuntu and macOS stop at the lints, which lane/linux / ci-next address. |
| host (+ host-d2w-draft) | a83420b7 | items done except E6 | a83420b7 | **yes** (f22025cd) | no | CI: [43363594548](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43363594548) baseline only (`download.ts`). Review passed: the panel test now sends the presentation watch at schema 1, which the contract fixes (`presentation.rs`: "presentation schema one"). That corrects the test, it doesn't weaken it, and a new contract test refuses schemas 0, 2, 3 and 1003. Unfinished: E6. Unproven: every Windows test of the lane; native Updates entry. |
| entry | bb026c95 | items done | bb026c95 | **yes** (a3d80408) | no | CI: [43363698336](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43363698336) baseline only. **Review now passes**: b61736d2 follows C-6. Every wait for another copy shows a surface at 2 s on the execution clock, and the launch keeps forwarding and re-electing behind it. "Close it and start" comes only after a hung owned probe or an Unresponsive answer, and ends only the verified copy. Re-election forgets the old copy, and a failed probe is retried. Seven manual-clock tests cover these, with no sleeps or real-clock reads. The two deleted `instance_conflict` tests belong to the removed end-the-pump-and-relaunch path; their behaviour is covered by `a_closing_copy_is_waited_for_then_the_launch_continues` and the probe tests. Words are plain ("Checking the previous SugarSubstitute window…", "Finishing closing the previous window…"). Open: a claim holder with no handshake gets the closing line and Cancel, never the question, until supervision supplies `ClaimHolder`. Unproven: real-Windows tests (Wine only), frozen-UI qualification, foreground from behind another window. Commits before b61736d2 carry the Claude identity. |
| packinstall | 62a5bae3 | items done; display seam waits on coord | 62a5bae3 | **yes** (8b542f46) | no | CI: [43363809191](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43363809191) baseline only. **Review now passes** for the lane's own scope. Gap 1 is fixed: a SugarCubes page failure (wire 14) is a trial verdict, the retry is held behind a restore of the previous version, and a restored version isn't reinstalled. Tests: `a_failed_page_restores_the_previous_version_before_the_next_attempt` and `a_failed_page_holds_the_retry_behind_a_restore…`. Gap 2 (the splash line and the one plain line) is now a typed seam that coord must display (see Wiring). It blocks shipping, not integration. No sleeps or real-clock tests; the removed asserts are wire 13 → 14 updates. Unproven: headless real run with ComfyUI; junction vectors; WinHTTP loopback; every `sugarsubstitute` crate test on Windows. |
| release | ee1ddcb3 | items done | ee1ddcb3 | content **yes** (b4ea9dc4) | no | The branch was rewritten for authorship: 3e05348e is no longer an ancestor, and its tree differs from 3e05348e only in `.github/release/LANE-REPORT.md` (new hashes). The code is already on integration, so I didn't merge the duplicate history. CI: [Release tooling](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43363221863) **success**; [Substitute](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43363221861) and [Sugar quality](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43363221913) are baseline only (mypy "22 errors in 11 files"). |
| coord | 64b4c067 | items done except `update_report_export` | 64b4c067 | no | no | Unchanged. It contains non-green lane/ci (80c37dd3), and item 2 is unfinished. Now also holds back packinstall's display. |
| timeouts | 483627df | items done | 483627df | **yes** (d6311f04) | no | Unchanged. |
| canvas-flakes | b88bc3fa | finished | b88bc3fa | **yes** (e27622c5) | no | none |
| ui | 0c68a2c7 | finished, converted | 0c68a2c7 | **yes** (d0ab3289) | no | Native: splash and caption images. |
| retire | 6634fd32 | item done | 6634fd32 | **yes** (e8a83a7d) | no | Unchanged. |
| fixer | f79c1f50 | items done | f79c1f50 | **yes** | no | Only its report moved (rewritten hashes). Integration doesn't carry `LANE-REPORT-fixer.md`, so I didn't merge the report-only commit. |
| storage | 89a29380 | items done | 89a29380 | **yes** (d37b4821) | no | Unchanged. |
| trust | 1d6adcd8 | items done | 1d6adcd8 | **yes** (138d1308) | no | Unchanged. |

### Native Windows proofs (unproven here)

Everything listed in earlier passes, plus entry's frozen-UI and foreground qualification, and
packinstall's headless ComfyUI run. lane/ci now links Windows tests, but integration doesn't have
that commit, so no Windows test of integration has run yet.

## Pass 4 (2026-11-13)

Tools that worked: git fetch/push over the proxy; rustup 1.93.1 with clippy, rustfmt and the
`x86_64-pc-windows-gnu` target; apt for mingw-w64, `libgtk-3-dev`, `libdbus-1-dev` and
`pkg-config`; the public Actions REST API through curl; the GitHub MCP `get_job_logs`. Not
working: `gh` (token rejected). Python 3.11 with no pytest; I ran the updater tool tests with
`unittest`.

### Integration tip and CI

- New tip a0e1b56b: merge of lane/timeouts at e9572e0b. No integration fixes were needed.
- CI on integration for a0e1b56b starts with this push. Runs:
  [integration runs](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions?query=branch%3Aintegration).
  Expect the baseline only (submodule on every OS for Substitute; ubuntu mypy and windows-latest
  VS2022 for Sugar quality). The previous tip's runs are listed under pass 3.
- Local checks on a0e1b56b:
  - `cargo fmt --check` passed. `cargo xtask governance` passed with the submodule checked out,
    including timeouts' new wall-clock gate (gate 1) across the whole merged tree.
  - `cargo test --locked -p sugarsubstitute-xtask`: 192 passed.
  - Windows-target Clippy (`-D warnings`) passed all three legs: workspace all-targets
    all-features, `-p sugarsubstitute`, and `--no-default-features`.
  - `python -m unittest tools.test_updater_feed_fixture`: passed. `tools.test_updater_qualification`
    can't import on Linux (it needs `native_windows`); CI on Windows runs it. **Unproven here.**
  - `sugarsubstitute-execution` still doesn't build on Linux (baseline), so its new
    `qualification_time` tests didn't run here.
  - `Sugar/` didn't change, so no Sugar gate was needed.
- Wiring: retire to storage was done in pass 1. Host `sugarcubes.updates` to coord is **still
  pending, because coord isn't merged**. Host's idle placeholders must not ship.

### Lanes

Only ci and timeouts moved since pass 3. A new branch, `lane/linux` (3e439aa2, "lint the
workspace clean on Linux and macOS without allowances", built on lane/ci), appeared. It isn't one
of the lanes this pass is given, it has no lane report, and its Substitute verification
([run 43361831925](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43361831925))
was still running. I didn't review or merge it. It looks like the portability lane lane/ci's report
recommended; it will need the coordinator to list it before it merges.

| Lane | Tip | State | Last reviewed | Merged | Stalled | Blocking reasons / unproven |
|---|---|---|---|---|---|---|
| ci | 11311c2f | working | 11311c2f | no | no | [Sugar quality](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43357914629) on 11311c2f is **green** on every leg, the first time. The report hasn't changed and still doesn't say both workflows are green: Substitute on Windows can't link `committed_dump_parser.lib` (**needs the maintainer or the crash owner**), and Linux/macOS lints remain (lane/linux may address those). No Substitute verification run on 11311c2f. |
| timeouts | e9572e0b | items done | e9572e0b | **yes** (a0e1b56b) | no | CI on 15cb35e9 (the code tip; later commits are report-only) is baseline only: [Substitute verification](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43361365145) stops at "resolve third_party/comfy-desktop/src/main/lib/download.ts" on ubuntu (windows and macOS fail in the same step); [Sugar quality](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43361365413) ubuntu fails mypy "Found 22 errors in 11 files" and windows-latest was still finishing; [Release tooling](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43361365196) **success**. Review passed. The branch already contained integration 47d2ade4, and its 13 combined-tree hits are converted, not allowlisted. `ProgressGuard` fails only after 10 minutes with no progress at all, as a hang backstop, so elapsed time never decides a pass. Its 15 s patrol only re-checks progress. The strike tests now check withholding at the strike's own recorded date, not at a later real-clock "now". That's the same claim without the real clock. `count` still decides withholding. The lost-event message test moved into `qualification_time`'s stall test (the stall still returns the lock and fails the check). No sleeps added and no attribution trailers. Commit 69b848f1 ("Merge remote-tracking branch…") doesn't follow the commit format (minor). Unproven: setup_storage and crate tests on Windows (test link blocker); native qualification campaigns under the no-progress guard; `test_updater_qualification` on Windows. Its 240-minute CI job limit must win over lane/ci's 60 when lane/ci merges. |
| canvas-flakes | 4a709007 | finished | 4a709007 | **yes** (e27622c5) | no | none |
| ui | 0c68a2c7 | finished, converted | 0c68a2c7 | **yes** (d0ab3289) | no | Native: splash and caption images. |
| retire | 13ff72df | item done | 13ff72df | **yes** (e8a83a7d) | no | Unchanged. |
| fixer | a2a51446 | items done | a2a51446 | **yes** (00d6be71) | no | Unchanged. |
| storage | e0fb0c7b | items done | e0fb0c7b | **yes** (d37b4821) | no | Unchanged. |
| trust | b1609462 | items done | b1609462 | **yes** (138d1308) | no | Unchanged. |
| host (+ host-d2w-draft) | 94ef3807 | items done except E6 | 94ef3807 | **yes** (04811a38) | no | Unchanged. Wiring to coord is pending. |
| release | 3e05348e | items done | 3e05348e | **yes** (b4ea9dc4) | no | Unchanged. |
| entry | 640e8f55 | items reported done | 640e8f55 | no | no | Unchanged; **still fails review** for the pass 1/2 reasons (C-6 departure; silent waits of up to 15 s or 3600 s). |
| coord | 19d3f15b | items done except `update_report_export` | 19d3f15b | no | no | Unchanged. It contains non-green lane/ci (80c37dd3), and item 2 is unfinished. |
| packinstall | d18856b6 | items reported done, with open gaps | d18856b6 | no | no | Unchanged; **still fails review** for the pass 3 reasons (no `PageFailed` verdict, so a broken pack is never restored and later updates block; the splash can wait up to 300 s with nothing on screen). |

### Native Windows proofs (unproven here)

Unchanged from pass 2, plus timeouts' native qualification campaigns under the no-progress guard.
Every Windows-only test on integration stays unproven until `committed_dump_parser.lib` builds in
CI.

## Pass 3 (2026-11-13)

Tools that worked: git fetch/push over the proxy; the public Actions REST API through curl (runs,
jobs, check-run annotations); the GitHub MCP `get_job_logs` with inline content. Not working: `gh`
(token rejected) and direct download of job-log blobs (the proxy answers 403). I didn't install
toolchains this pass: nothing merged, so integration is unchanged and had nothing new to check.

### Integration tip and CI

- Tip unchanged at ce549b3e (code tip 70129539). No merges this pass.
- CI for 70129539 is baseline only:
  - [Substitute verification](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43352365202)
    failed on ubuntu, windows and macOS. I read the ubuntu log: it stops at "resolve
    third_party/comfy-desktop/src/main/lib/download.ts". The windows and macOS failures are
    in the same step.
  - [Sugar quality](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43352365234):
    ubuntu fails mypy with "Found 22 errors in 11 files"; windows-latest fails with "Select
    exactly one installation of the declared VS2022 product".
  - [Release tooling](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43352365179):
    **success**.
- Wiring: retire to storage was done in pass 1. Host `sugarcubes.updates` to coord is **still
  pending, because coord isn't merged**. Host's idle placeholders must not ship.

### Lanes

Only ci and packinstall moved since pass 2. Every other lane's row from pass 2 still stands, with
the same tip and the same reasons. No lane is stalled: each tip has moved from its starting
snapshot.

| Lane | Tip | State | Last reviewed | Merged | Stalled | Blocking reasons / unproven |
|---|---|---|---|---|---|---|
| ci | 13cc1c06 | working | 13cc1c06 | no | no | New commit: the caret proof's editor is now sized from the measured first paragraph (it targets the ubuntu `prompt_empty_line_caret` failures). Its [Sugar quality](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43355708262) was still running. The report hasn't changed and doesn't say both workflows are green: Windows tests still can't link `committed_dump_parser.lib` (**needs the maintainer or the crash owner**), and about 250 Linux/macOS lints remain. |
| canvas-flakes | 4a709007 | finished | 4a709007 | **yes** (e27622c5) | no | none |
| ui | 0c68a2c7 | finished, converted | 0c68a2c7 | **yes** (d0ab3289) | no | Native: splash and caption images. |
| retire | 13ff72df | item done | 13ff72df | **yes** (e8a83a7d) | no | Unchanged. |
| fixer | a2a51446 | items done | a2a51446 | **yes** (00d6be71) | no | Unchanged. |
| storage | e0fb0c7b | items done | e0fb0c7b | **yes** (d37b4821) | no | Unchanged. |
| trust | b1609462 | items done | b1609462 | **yes** (138d1308) | no | Unchanged. |
| host (+ host-d2w-draft) | 94ef3807 | items done except E6 | 94ef3807 | **yes** (04811a38) | no | Unchanged. Wiring to coord is pending. |
| release | 3e05348e | items done | 3e05348e | **yes** (b4ea9dc4) | no | Unchanged. |
| entry | 640e8f55 | items reported done | 640e8f55 | no | no | Unchanged; **still fails review** for the pass 1/2 reasons (C-6 departure; silent waits of up to 15 s or 3600 s). |
| timeouts | c79a5ab8 | gate added, 18 hits unresolved | c79a5ab8 | no | no | Unchanged; merging it would break governance on integration. |
| coord | 19d3f15b | items done except `update_report_export` | 19d3f15b | no | no | Unchanged. It contains non-green lane/ci (80c37dd3), and item 2 is unfinished. |
| packinstall | d18856b6 | items reported done, with open gaps | d18856b6 | no | no | CI is baseline only: [Substitute verification](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43350654846) (submodule); [Sugar quality](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43350654936) (mypy "22 errors in 11 files", VS2022). d18856b6 is a report-only commit after c255de41. **Fails review against F3w and HANDOFF §7:** (1) Nothing sends `TrialEvent::PageFailed`, so a new pack whose page fails stays in `Started`/`Unobserved`. `TrackedReplacement::settle` returns without a verdict for those phases (`pack_install/trial.rs`), so the broken pack is never restored and every later pack update stays blocked. F3w requires a restore when "the page does not establish its session", and §7 requires that a release is running within two launches or the user is told why. (2) The splash can wait up to `PACK_UPDATE_LIMIT` (300 s) for the pack update with no "Updating SugarCubes…" line. A failed check never shows its "one line", and `report.json` never reaches the window, which breaks F3w's "show one line" and §7's rule that every wait over two seconds has something on screen. The lane assigns both fixes to coord2/ui3. They need a page-failure path from the page to the coordinator and a splash line. The lane is fine otherwise: no sleeps or wall-clock tests in the diff, no attribution trailers, and the 300 s/30 s limits come from the injected client clock. Unproven: a headless real run with ComfyUI, the junction vectors, the registry-install setup test and WinHTTP loopback on Windows. |

### Native Windows proofs (unproven here)

Unchanged from pass 2. Every lane's native proofs and every Windows-only test on integration stay
unproven until `committed_dump_parser.lib` builds in CI.

## Pass 2 (2026-11-13)

Tools that worked: git fetch/push over the proxy (the clone was shallow, so I ran `git fetch --unshallow`;
release tooling tests need the full history); rustup 1.93.1 with clippy and rustfmt, plus the
`x86_64-pc-windows-gnu` target and mingw-w64 for Windows-target Clippy; apt; Node 22 with `npm ci` in
`Sugar/` and `.github/release/`; uv for a Python 3.12 venv (`pip install -e ".[dev]"`; the system
Python is 3.11 and the pack needs >=3.12); the public Actions REST API through curl; the GitHub MCP for
job logs. Not working: `gh` (its token is rejected).

### Integration tip and CI

- Tip 70129539: pass 1 plus host (04811a38), release (b4ea9dc4) and an integration fix (70129539).
- CI for 70129539, queued when this was written:
  [Substitute verification](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43352365202),
  [Sugar quality](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43352365234),
  [Release tooling](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43352365179).
  Until lane/ci lands, Substitute and Sugar quality will stop at the baseline.
- Pass 1's tip 138d1308: [Substitute verification](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43346701866) failed on every OS at the
  `download.ts` submodule baseline only. [Sugar quality](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43346701876) failed with ubuntu mypy
  "22 errors in 11 files" (baseline) and the windows-latest VS2022 selection (baseline).
- Local checks on the tip:
  - `cargo xtask governance` and `cargo fmt --check` passed. `cargo xtask fingerprints` reported
    nothing stale after both merges.
  - Windows-target Clippy (`-D warnings`) passed all three legs: workspace all-targets
    all-features, `-p sugarsubstitute`, and `--no-default-features`.
  - Tests passed for contracts, application, xtask (186), update and update-repository, plus the
    92 Node panel-bridge tests. Release tooling `npm test` passed 36/36 and `format:check` passed.
  - Sugar `npm run check:staged` stops at the known mypy baseline. Lint and `build:check` pass.
    `--tests-only` ran 1990 pytest passes; the 10 failures in `tests/gimp/unit/test_viewer_runtime.py`
    (`subprocess.CREATE_NO_WINDOW` on Linux) fail the same way on origin/main, so they are baseline.
    Jest didn't run after that pytest failure. No Jest code touches what release changed, and CI
    runs the full suite.
- Integration fix 70129539, `fix(quality)`: the release lane deletes `Sugar/.releaserc.json`, and
  the staged gate refused it ("runtime change is not allowed to select zero tests"). A file that
  is gone can't be given an owner. Now a removed runtime file that no area owns selects every proof
  group, and an unowned file that still exists is refused as before. It comes with two regression
  tests (selection, and a staged `--plan` on a fixture repository); both fail before the fix and
  pass after.
- Merge conflicts were fingerprint and record conflicts in `governance/infrastructure/
  localization.toml`, `governance/ui/policy.toml` and `configuration.toml`. I resolved them by
  keeping both sides' records and reviewing the merged sources (no new label literals; the UI
  boundary now covers host's panel files), then recorded the fingerprints with
  `cargo xtask fingerprints`, the tool the gates name.
- Wiring:
  - retire to storage: done in pass 1 (654f7cea).
  - host `sugarcubes.updates` to coord's update-status source: **pending, because coord isn't
    merged.** Until then, host's `IdleUpdateStatus` and `IdleWorkflowActivity` placeholders tell
    the page "up to date", "check unavailable" and "restart not ready". That is fine on
    integration, but it must not ship like that.

### Lanes

| Lane | Tip | State | Last reviewed | Merged | Stalled | Blocking reasons / unproven |
|---|---|---|---|---|---|---|
| ci | 09ffb330 | working | 09ffb330 | no | no | Its report still doesn't say both workflows are green. [Sugar quality](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43347133142) on 5761bd3c: windows-2022 **green**; ubuntu fails 7 `prompt_empty_line_caret` font-wrapping tests, which 09ffb330 targets. Substitute: Windows tests can't link `committed_dump_parser.lib` (**needs the maintainer or the crash owner**). Linux and macOS have about 250 `unreachable_pub`/`dead_code` lints. |
| canvas-flakes | 4a709007 | finished | 4a709007 | **yes** (e27622c5) | no | none |
| ui | 0c68a2c7 | finished, converted | 0c68a2c7 | **yes** (d0ab3289) | no | Native: splash and caption images. |
| retire | 13ff72df | item done | 13ff72df | **yes** (e8a83a7d) | no | Unchanged since pass 1. Windows tests not run; native proofs as in pass 1. |
| fixer | a2a51446 | items done | a2a51446 | **yes** (00d6be71) | no | Unchanged. Items 1–3 haven't run on Windows. |
| storage | e0fb0c7b | items done | e0fb0c7b | **yes** (d37b4821) | no | Unchanged. Native: cross-process run handoff; silent child. Open: nothing on screen during the headless creator's wait of up to 32 s. |
| trust | b1609462 | items done | b1609462 | **yes** (138d1308) | no | Unchanged. Native: expired installer offline and online; prepared update launched with expired cached metadata. |
| host (+ host-d2w-draft) | 94ef3807 | items done except E6 | 94ef3807 | **yes** (04811a38) | no | CI: [43344721034](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43344721034) is baseline only. Review passed: governance edits are fingerprints and new records only; the `cargo xtask fingerprints` tool refuses unresolved sources and doesn't change any gate; the bridge coalescing fix is tested. Unfinished: E6 Substitute profile (needs Windows tests to link). Unproven: every Windows test in this lane; native Updates entry; description reaching the real page. Minor: its report sits at `Substitute/LANE-REPORT-host.md`, not the repository root. |
| release | 3e05348e | items done | 3e05348e | **yes** (b4ea9dc4) | no | Review passed. The deleted `release_config.test.ts` is superseded by `.github/release/tests/release.test.mjs` (a breaking change bumps the minor version; notes list breaking changes), which asserts behaviour rather than config shape. Publishing stays gated by dispatch plus `RELEASES_ENABLED`. Its root `AGENTS.md` section is release rules only. `release_source.json` now names this monorepo (recorded decision). Unproven: real publication, `.ssupdate` packaging, `release.yml` dispatch (works only from main), the Python registry tests (they need pinned comfy-cli; CI runs them). The maintainer must choose the SugarSubstitute baseline and create keys and root. |
| entry | 640e8f55 | items reported done | 640e8f55 | no | no | **Still fails review**; only a docs commit since pass 1. Decision 1 departs from settled C-6: a copy that answers its message queue but not the pipe gets no surface. A closing copy with no handshake re-elects silently for up to 15 s (3600 s if hung), which breaks HANDOFF §7 ("every wait longer than two seconds has something on screen"). Needs a coordinator ruling or a surface for those waits. |
| timeouts | c79a5ab8 | gate added, 18 hits unresolved | c79a5ab8 | no | no | Unchanged. Its wall-clock gate fails governance on 18 setup_storage test files, so merging would break integration. Fixer's `StrikeClock` is on integration for the conversion. |
| coord | 19d3f15b | items done except `update_report_export` | 19d3f15b | no | no | Unchanged. It contains `origin/lane/ci` (80c37dd3), which isn't green, so merging coord would bring lane/ci in early. Item 2 (report export) unfinished. This also holds back the host-to-coord wiring. |
| packinstall | 53de5c93 | report landed this pass | 53de5c93 (preliminary) | no | **no** (moved this pass: 9b62b528 → 53de5c93) | Its [Sugar quality](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43350654936) was still running, so the full review is next pass. [Substitute verification](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43350654846) is baseline only. Concerns its own report raises, both breaking HANDOFF §7: (1) a SugarCubes page failure isn't a trial verdict, so the update stays "being judged" and **blocks later updates**; (2) nothing on the splash during the update wait, and the result (`report.json`) never reaches the window. Both need coord2/ui3 work. Commit 7231f5a5 ("Merge remote-tracking branch…") and the WIP snapshot 9b62b528 don't follow the commit format. Unproven: headless real run with ComfyUI; junction vectors and WinHTTP loopback on Windows. |

### Native Windows proofs (unproven here)

All lanes' native proofs above, plus every Windows-only test on integration. None can run until
`committed_dump_parser.lib` builds in CI.

## Pass 1 (2026-11-13)

Tools that worked: git fetch and push over the proxy; rustup 1.93.1 with clippy and rustfmt, plus the
`x86_64-pc-windows-gnu` target and mingw-w64 for Windows-target Clippy; apt for `libgtk-3-dev`,
`libdbus-1-dev` and `pkg-config`; the public Actions REST API through curl; the GitHub MCP for job logs.
Not working: `gh` (its token is rejected). Node and npm are present but weren't needed: `Sugar/` on
integration is identical to main.

### Integration tip and CI

- Tip 138d1308, which holds canvas-flakes, ui (converted), retire, fixer, storage, trust and the
  retire/storage wiring 654f7cea.
- Substitute verification: [run 43346701866](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43346701866)
  (queued when this was written). Until lane/ci lands, expect it to stop at main's submodule baseline.
- Local, on the tip:
  - `cargo xtask governance` passed (submodule checked out); `cargo fmt --check` passed.
  - Windows-target Clippy (`x86_64-pc-windows-gnu`, `-D warnings`) passed all three legs: workspace
    all-targets all-features, `-p sugarsubstitute`, and `-p sugarsubstitute --no-default-features`.
  - `cargo test -p sugarsubstitute-update -p sugarsubstitute-contracts -p sugarsubstitute-xtask`:
    all passed (299 contracts, 53 update, 179 xtask).
  - Linux `cargo xtask check` stops in workspace Clippy on the baseline (`sugarsubstitute-execution`
    doesn't compile off Windows; lane/ci 22ed884d fixes the hard errors).
- Wiring done: retire's `LaunchPointerGenerations` now delegates to storage's
  `setup_storage::pointed_generations` (654f7cea), with a test that reads a real published pointer.
  It passes Windows Clippy, but the test is Windows-only and hasn't run.
- Wiring pending: host `sugarcubes.updates` to coord's update-status source. Neither lane is merged yet.

### Baseline (lane/ci not merged yet)

These are main's three known failures, plus what lane/ci's report found behind them:
- Windows test binaries can't link `committed_dump_parser.lib`, because nothing in the repository
  builds it. **Only the maintainer or the crash owner can fix this.** Until it's fixed, no Windows
  test runs anywhere.
- Ubuntu and macOS: about 250 `unreachable_pub`/`dead_code` lints remain after lane/ci's compile fixes.

### Lanes

| Lane | Tip | State | Last reviewed | Merged | Stalled | Blocking reasons / unproven |
|---|---|---|---|---|---|---|
| ci | 37f603c5 | working | 37f603c5 (report) | no | no | Its report doesn't say both workflows are green: Windows tests can't link, and Linux/macOS lints remain. The rule is to merge only once green. |
| canvas-flakes | 4a709007 | finished | 4a709007 | **yes** (e27622c5) | no | none |
| ui | 0c68a2c7 | finished, converted locally (ba81ef40 + fingerprints 63d198b7) | 0c68a2c7 | **yes** (d0ab3289) | no | Native: splash and caption images on a real desktop. |
| retire | 13ff72df | item done | 13ff72df | **yes** (e8a83a7d) | no | Windows tests not run. Native: a rename fails while a file inside is open; a real update cycle reclaiming disk. Staging copies the payload before the inventory (trust's `candidate.rs`). |
| fixer | a2a51446 | items done | a2a51446 | **yes** (00d6be71) | no | Items 1–3 haven't run on Windows. The cause of items 2–3 is inferred, not reproduced. |
| storage | e0fb0c7b | items done | e0fb0c7b | **yes** (d37b4821) | no | Windows tests not run. Native: the cross-process run handoff; a silent child ended at 30 s + 2 s. Open: nothing is on screen while the headless creator waits up to 32 s for a silent child. |
| trust | b1609462 | items done | b1609462 | **yes** (138d1308) | no | Windows tests not run. Native: an expired installer offline and online; a prepared update launched with expired cached metadata. |
| entry | 72609888 | items reported done | 54091044 | no | no | **Fails review.** Decision 1 knowingly departs from settled C-6: a copy that answers its message queue but not the pipe gets no surface. With a closing copy that has no handshake, this launch re-elects silently for up to 15 s, or 3600 s if the copy hangs. Both break HANDOFF §7 ("every wait longer than two seconds has something on screen"). Needs a coordinator ruling or a surface for those waits. Windows tests not run. |
| timeouts | c79a5ab8 | gate added, 18 hits unresolved | c79a5ab8 | no | no | Its own wall-clock gate fails governance on 18 setup_storage test files, so merging it would break integration. Fixer's `StrikeClock` (now on integration) lets those files convert. Re-check once timeouts merges integration's clock or converts the files. |
| coord | 19d3f15b | items done except `update_report_export` | 19d3f15b (report) | no | no | It merged `origin/lane/ci` (80c37dd3), and lane/ci isn't green, so merging coord would bring lane/ci in early. Item 2 (report export) is unfinished. Native: updating splash during a trial, caption notices, queue-watch reconnect, a real full disk. |
| host (+ host-d2w-draft) | 85d7b09b | report present | not reviewed this pass | no | no | Review next pass. E6 Substitute profile not recorded (needs Windows tests to link). Wiring to coord pending. |
| release | 3e05348e | report present | not fully reviewed | no | no | Review next pass. It changes `Sugar/` (release config, `release_source.json`, deletes `release_config.test.ts`), so it needs Sugar gates and a check that the deleted test is superseded, not weakened. It also edits root `AGENTS.md`. |
| packinstall | 9b62b528 | no report | — | no | **pass 1 of 2 unmoved** (= starting snapshot 9b62b528) | No work since the snapshot. |

### Native Windows proofs (unproven here)

All lanes' native proofs listed above, plus every Windows-only test on integration. None can run
until `committed_dump_parser.lib` builds in CI.
