# Lane report: entry (A6, C-6, M-1)

## Unfinished and unproven
- **Windows tests not yet run** (CI stops at main's baseline; `lane/ci` isn't green): re-election, Closing/Unresponsive delivery, foreground before Launch, v3 refused, hung probe, silent server at 2 s, closing copy then continue.
- **Native qualification, unproven:** frozen UI thread plus a second launch; foreground from behind another window; the real probe; surface images; "Close it and start" ending the copy's children.
- **Closing copy with no handshake:** if its endpoint is gone but it still holds its claim, there is no verified pid. This launch re-elects silently until that copy exits: its 15 s shutdown budget, or the 3600 s backstop if it hangs. Needs the claim holder's identity (Restart Manager on `coordinators\<ns>.claim`).

## Decisions made
1. **A copy that answers its message queue but not the pipe gets no surface (departs from C-6's wording).** At 2 s, an owned probe decides: hung means the question, exited means re-election, responsive means keep forwarding and probe again after 2 s. A report window can't run behind an ongoing forward, so it would strand a "Checking…" window over a copy that is still starting.
2. **No splash in forwarding processes** (FINDINGS-host Q5). The wait uses the D1w Fluent window: "Starting SugarSubstitute", "Finishing closing the previous window…", Cancel.
3. **Continuing is a fresh launch** through supervision's shared `Relaunch` once the copy has exited.
4. **Probe targets:** the coordinator probes its desktop; the forwarding launch probes the coordinator's pump windows.
5. **No 15 s Force step (M-1).** "Close it and start" ends the verified coordinator at once; its kill-on-close job ends the desktop and backend.
6. **Snapshot kept in history** (force-push refused).

## Commits
- 758d6d2c chore(entry): carry the entry lane's unfinished work to the cloud
- b509038f merge: move lane/entry into main's monorepo layout under Substitute
- 890db2c6 refactor(supervision): let any conflict resolution start one fresh launch with the original arguments
- 968a350a feat(entry): a second launch brings a responsive copy forward, waits for a closing one and asks only about one that has stopped responding
- d6fb6c18 fix(entry): probe each running copy afresh and let a launch continue when an unresponsive copy exits by itself

## Verification
- `cargo xtask governance` and `cargo fmt --check`: passed.
- Windows target (`x86_64-pc-windows-gnu`): `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed. `cargo doc --no-deps` with `-D warnings` passed for the six touched crates.
- Linux: application `instance_conflict` 5 passed; contracts all passed. Linux `cargo xtask check` and the `sugarsubstitute` tests stop on `sugarsubstitute-execution` clippy errors (`admission.rs`, `runtime_supervision.rs`) that `origin/main` also has: baseline.
- CI run 43338293318: failed on all three OSes at the baseline only (`third_party/comfy-desktop` not checked out). No lane test ran.

## Next
- Merge `origin/lane/ci` once it's green, re-run CI, and fix any test above that fails.
- `lane/ci`: Linux runners lack dbus/GTK dev packages; then Linux clippy fails in `sugarsubstitute-execution`.
- Coordinator: rule on decision 1; assign claim-holder identity; run the native proofs.
