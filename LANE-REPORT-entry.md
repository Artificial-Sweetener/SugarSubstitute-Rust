# Lane report: entry (A6, C-6, M-1)

## Unfinished and unproven
- **Real-Windows CI hasn't run the tests.** Main's CI stops at the submodule step. With `lane/ci`, Windows test binaries still can't link `committed_dump_parser.lib` (`lane/ci` item 1). They ran locally under Wine (below).
- **Native qualification, unproven:** frozen UI thread plus a second launch; foreground from behind another window; the real probe; the hosted waiting surface and its images.
- **Claim holder without a handshake:** the claims owner can't yet name the coordinator's process, so that wait shows the closing line with Cancel but never the question. `ForwardWait` takes a `ClaimHolder` port; supervision needs to supply it.

## Decisions made
1. **C-6 followed (ruling):** every wait for another copy shows a surface at 2 s on the execution clock.
2. **The headless pump hosts the surface** as an unowned form on its own loop. The launch keeps forwarding and re-electing behind it. Accepted or winning election dismisses it. The end-the-pump-and-relaunch path is removed.
3. **Words:** "Checking the previous SugarSubstitute window…", or "Finishing closing the previous window…" for a closing copy, with Cancel only. Cancel ends the launch. The question appears only after a hung probe or an Unresponsive answer. "Close it and start" ends exactly that verified copy, with no 15 s Force step (M-1).
4. **No splash in forwarding processes** (FINDINGS-host Q5); the surface is the D1w Fluent window.
5. **Probe answers are bound to their identity.** Re-election to another copy forgets the old one. A failed probe is retried and never ends the launch.
6. **`lane/ci` not merged** (not green). Merged tree checked locally.
7. **Snapshot kept in history** (force-push refused). Authors were rewritten by the coordinator.

## Commits
- 758d6d2c chore(entry): carry the entry lane's unfinished work to the cloud
- 349a348b merge: move lane/entry into main's monorepo layout under Substitute
- 5d3c75da refactor(supervision): let any conflict resolution start one fresh launch with the original arguments
- a3c333e5 feat(entry): a second launch brings a responsive copy forward, waits for a closing one and asks only about one that has stopped responding
- 3158e962 fix(entry): probe each running copy afresh and let a launch continue when an unresponsive copy exits by itself
- 1bc25f24 test(entry): build the lost-coordinator fixture without Windows-only error types
- b61736d2 feat(entry): show a launch waiting on another copy within two seconds while it keeps forwarding

## Verification
- Governance and fmt pass. Windows-target workspace clippy (all targets and features, `-D warnings`) and rustdoc pass.
- Under Wine (`x86_64-pc-windows-gnu`, local stub parser archive, not committed): 359 entry runner, activation, parent lifecycle and conflict tests pass, including all 7 manual-clock wait tests. 156/157 execution coordinator-entry tests pass; the one failure also fails on `origin/main` under Wine. Desktop rendering tests hit Wine's E_NOTIMPL.
- Merged with `origin/lane/ci` on Linux: no stale fingerprints; desktop report, contracts and application tests pass.

## Next
- Integrator: merge `lane/ci` when green; re-run on real Windows.
- Supervision: provide `ClaimHolder`.
- Coordinator: native proofs.
