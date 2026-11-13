# Lane report: entry (A6, C-6, M-1)

## Unfinished and unproven
- **Real-Windows CI hasn't run the tests.** Main's CI stops at the submodule step. With `lane/ci`, Windows test binaries still can't link `committed_dump_parser.lib` (`lane/ci` item 1). They ran locally under Wine (below).
- **Native qualification, unproven:** frozen UI thread plus a second launch; foreground from behind another window; the real probe; the hosted waiting surface and its images.

## Decisions made
1. **C-6 followed (ruling):** every wait for another copy shows a surface at 2 s on the execution clock.
2. **The headless pump hosts the surface** as an unowned form on its own loop. The launch keeps forwarding and re-electing behind it. Accepted or winning election dismisses it. The end-the-pump-and-relaunch path is removed.
3. **Words:** "Checking the previous SugarSubstitute window…", or "Finishing closing the previous window…" for a closing copy, with Cancel only. Cancel ends the launch. The question appears only after a hung probe or an Unresponsive answer. "Close it and start" ends exactly that verified copy, with no 15 s Force step (M-1).
4. **No splash in forwarding processes** (FINDINGS-host Q5); the surface is the D1w Fluent window.
5. **Probe answers are bound to their identity.** Re-election to another copy forgets the old one. A failed probe is retried and never ends the launch.
6. **`lane/ci` not merged** (not green). Merged tree checked locally.
7. **Snapshot kept in history** (force-push refused). Authors were rewritten by the coordinator.
8. **Review fix 1, a copy that answers its windows but not its pipe (C-6, M-1; corrected by the coordinator):** a copy verified on its pipe, by a connection or a discovery, becomes the copy whose windows are probed. If its pipe stays silent, the surface still opens at 2 s with the waiting line and Cancel, and the launch keeps waiting for its answer. The question appears only when the probe reports the copy hung, or the copy itself answers Unresponsive. No timer ever turns into the question. The interim commit 8504f08e made 2 s of pipe silence ask the question, which departed from M-1. d8a39bb5 removes that.
9. **Review fix 2, the pump opens its loop for the surface:** a forwarding launch never activates its own server, so its headless pump stayed in preparation and never presented the surface. Every wait ended silently when the copy exited (15 s) or at the 3600 s backstop. `prepare_native_runtime` now reports Ready once the surface is due (2 s, or at once for an unresponsive copy) and the launch is neither closing nor already forwarded.
10. **The coordinator record names its holder:** a coordinator record's unused generation and process slots now carry the publishing coordinator's pid and creation time, kept through release and checked on release. Election reports the holder in `EntrySnapshot::holder`. A launch that never reached the copy probes that process, and gets the question when it is hung. The holder grants no authority, and every native use re-verifies the creation time. The `ClaimHolder` port is removed. Substitute is unreleased, so there is no record migration: a record with one slot set and the other zero is rejected as invalid.

## Commits
- d8a39bb5 fix(entry): ask about another copy only when the probe finds it hung, never after a silent pipe
- 3dccbbfd docs(entry): record the A6 review fixes, their decisions and verification
- 8504f08e fix(entry): ask about a copy that stops answering its pipe and show every wait for another copy at two seconds
- 758d6d2c chore(entry): carry the entry lane's unfinished work to the cloud
- 349a348b merge: move lane/entry into main's monorepo layout under Substitute
- 5d3c75da refactor(supervision): let any conflict resolution start one fresh launch with the original arguments
- a3c333e5 feat(entry): a second launch brings a responsive copy forward, waits for a closing one and asks only about one that has stopped responding
- 3158e962 fix(entry): probe each running copy afresh and let a launch continue when an unresponsive copy exits by itself
- 1bc25f24 test(entry): build the lost-coordinator fixture without Windows-only error types
- b61736d2 feat(entry): show a launch waiting on another copy within two seconds while it keeps forwarding

## Verification
- Review fixes (Windows, local): `cargo xtask check` passes on the staged contents. Supervision `native_claims::backend_recovery` and `coordinator_entry` tests: 65 passed. `sugarsubstitute --lib entry_runner`: 36 passed. These include the 9 manual-clock wait tests, among them "pipe silent, windows answer: waiting line, no question" and "pipe silent, probe hung: question", and the 2 pump-admission tests. Rustdoc with `-D warnings` passes for both crates. The new tests use the new wait API, so the old code can't compile them; their assertions are exactly the behaviour the review found missing.
- The persisted claim-record catalog fingerprint is refreshed. The layout and version are unchanged, and older records with zero identity slots decode as naming no holder.
- Governance and fmt pass. Windows-target workspace clippy (all targets and features, `-D warnings`) and rustdoc pass.
- Under Wine (`x86_64-pc-windows-gnu`, local stub parser archive, not committed): 359 entry runner, activation, parent lifecycle and conflict tests pass, including all 7 manual-clock wait tests. 156/157 execution coordinator-entry tests pass; the one failure also fails on `origin/main` under Wine. Desktop rendering tests hit Wine's E_NOTIMPL.
- Merged with `origin/lane/ci` on Linux: no stale fingerprints; desktop report, contracts and application tests pass.

## Next
- Integrator: merge `lane/ci` when green; re-run on real Windows.
- Coordinator: native proofs.
