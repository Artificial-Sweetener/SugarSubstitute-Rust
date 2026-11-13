# Lane report: entry (A6, C-6, M-1)

## Unfinished and unproven
- **Windows tests blocked.** No lane test has run. Even with `lane/ci`, Windows test binaries can't link, because `committed_dump_parser.lib` isn't built anywhere (`lane/ci` item 1).
- **Native qualification, unproven:** frozen UI thread plus a second launch; foreground from behind another window; the real probe; surface images.
- **Closing copy with no handshake:** if its endpoint is gone but it still holds its claim, there is no verified pid. The launch re-elects silently until that copy exits (15 s budget, or the 3600 s backstop if it hangs). Needs the claim holder's identity (Restart Manager on `coordinators\<ns>.claim`).

## Decisions made
1. **A copy that answers its message queue but not the pipe gets no surface (departs from C-6's wording).** At 2 s an owned probe decides: hung means the question, exited means re-election, responsive means keep forwarding and probe again. A window shown then can't be dismissed by a later Accepted, so it would strand over a copy still starting.
2. **No splash in forwarding processes** (FINDINGS-host Q5). The D1w Fluent window shows "Starting SugarSubstitute", "Finishing closing the previous window…" and Cancel.
3. **Continuing is a fresh launch** through supervision's shared `Relaunch`, once the copy has exited.
4. **Probe targets:** the coordinator probes its desktop; the forwarding launch probes the coordinator's pump windows. Answers are bound to the identity probed.
5. **No 15 s Force step (M-1).** "Close it and start" ends the verified coordinator at once. A copy that exits during the question continues the launch.
6. **`lane/ci` not merged:** not green, and it wouldn't run these tests. The merged tree was checked locally.
7. **Snapshot kept in history** (force-push refused).

## Commits
- 758d6d2c chore(entry): carry the entry lane's unfinished work to the cloud
- b509038f merge: move lane/entry into main's monorepo layout under Substitute
- 890db2c6 refactor(supervision): let any conflict resolution start one fresh launch with the original arguments
- 968a350a feat(entry): a second launch brings a responsive copy forward, waits for a closing one and asks only about one that has stopped responding
- d6fb6c18 fix(entry): probe each running copy afresh and let a launch continue when an unresponsive copy exits by itself
- 1162d758 test(entry): build the lost-coordinator fixture without Windows-only error types

## Verification
- Branch: governance and fmt pass. Windows-target workspace clippy (all targets and features, `-D warnings`) and rustdoc pass.
- Merged with `origin/lane/ci` (local only): clean merge, no stale fingerprints, governance and Windows clippy pass. Linux check with lints capped has no hard errors, and none of `lane/ci`'s remaining Linux lints are in this lane's files.
- Linux tests: application and contracts pass.
- CI runs 43338293318 and 43340366209: failed on all OSes at the submodule baseline only.

## Next
- Integrator: merge `lane/ci` here when green. Once Windows tests link, fix any failure.
- `lane/ci`: Linux clippy still fails in `sugarsubstitute-execution` on main.
- Coordinator: rule on decision 1; assign claim-holder identity; run the native proofs.
