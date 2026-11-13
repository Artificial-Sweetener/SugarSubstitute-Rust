# Lane report: trust (A2, C3, C4)

## Unfinished or unproven
- **Windows tests have not run in CI.** On every OS, "Substitute verification" stops at the known submodule baseline before any test (run 43340243980). `lane/ci` was not green, so it is not merged. Every Windows-only change passed a type-check, `clippy --all-targets --all-features -D warnings` and Rustdoc with `-D warnings`, all for `x86_64-pc-windows-msvc` in the container. The new tests in `signed_installer/tests/staged.rs`, `update_preparation/tests/receipt.rs` and `signed_installer/distribution/tests/expiry.rs` have not been executed.
- **Native proofs needed:** an expired installer run offline (retry sentence, then install) and online; a prepared update launched with expired cached metadata.
- Linux workspace Clippy fails in `sugarsubstitute-execution` (unused `mut`). That is pre-existing and outside this lane.

## Decisions made
- A2: the first hold also advances the release floor durably, so journaled intent never comes before it. The final hold authorises it again (idempotent).
- C3: a retried commit replaces the receipt atomically. No receipt is written for an included-release install.
- C3 revocation: the protected targets floor shows whether newer metadata was seen. It is inspected through the retained cached targets bytes, used only when their digest matches that floor. A changed descriptor means superseded; a missing descriptor or package means revoked. Newer metadata that cannot be inspected also refuses.
- C3: the `Reopening` reservation now includes the offer lookup. No new `Progress` variant.
- C4: the included release is verified as of its earliest role expiry, in an isolated trust copy. It needs valid signatures under the current root and passes rollback history; durable floors don't change.
- C4 online: if refreshed metadata names a newer release, the included release installs and the updater brings it current. Withdrawn or lower releases are refused.
- C4 offline: the explicit choice is Retry after one plain sentence, translated in all installer locales. The retry tries online once more first.
- `MetadataSet` is now `Copy`.
- Force-push was refused, so the branch stays fast-forward: a layout merge with main's tree, then clean commits.

## Commits
- fa9db781 merge: move lane/trust into main's monorepo layout under Substitute
- 9aafce0e fix(updates): keep update trust readable while an update is staged, and refuse it if signing authority changes meanwhile (A2)
- dfc84781 feat(updates): start a prepared update offline once its signed metadata has expired (C3)
- 83279d05 feat(setup): let a downloaded installer install even after its built-in update information expires (C4)
- a001adda test(updates): judge offline receipt activation by the started trial, not by a later run-lock probe

## Verification
- `cargo xtask governance` passes; fmt is clean.
- `cargo test -p sugarsubstitute-update`: 53 passed (12 receipt conditions, 5 pinned). Contracts localization tests pass.
- Windows-target check, Clippy and Rustdoc pass for supervision, update, contracts and sugarsubstitute.
- CI: https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43340243980 failed on the baseline only.

## Next
- Merge `origin/lane/ci` once it is green, rerun CI and fix any failures in the new Windows tests.
- G release tooling: serve `N.root.json` with a clean 404 after the newest root.
- UI owner (optional): replace the retry sentence with a two-button page that sends `InstallerMetadataChoice::Included`.
