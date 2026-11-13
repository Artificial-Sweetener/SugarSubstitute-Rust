# Integration status

Branch `integration`, created from `origin/main` 0cd6362f. Updated by each cloud integration pass.

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
