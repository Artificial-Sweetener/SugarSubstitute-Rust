# Integration status

Branch `integration`, created from `origin/main` 0cd6362f. Updated by each cloud integration pass.

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
