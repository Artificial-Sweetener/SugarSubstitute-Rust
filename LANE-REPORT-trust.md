# Lane report: trust (A2, C3, C4)

## Unfinished or unproven
- **Windows tests have not run in CI.** On every OS, "Substitute verification" stops at the known baseline (`third_party/comfy-desktop` submodule not checked out; run 43340243980), before any test. `lane/ci` was not green yet, so I did not merge it. Every Windows-only change was type-checked and passed `clippy --all-targets --all-features -D warnings` and `RUSTDOCFLAGS=-D warnings cargo doc` for `x86_64-pc-windows-msvc` in the container. The new Windows tests in `signed_installer/tests/staged.rs`, `update_preparation/tests/receipt.rs` and `signed_installer/distribution/tests/expiry.rs` have not been executed.
- **Native proofs needed:** a real installer run, offline and online, past metadata expiry (the retry sentence on screen, then install), and a real launch of a prepared update with expired cached metadata.
- **Linux workspace Clippy** fails in `sugarsubstitute-execution` (`runtime_supervision.rs`, `admission*.rs`, unused `mut`). That is pre-existing; I don't touch that crate.

## Decisions made
- A2: the first hold also durably advances the release floor, so journaled intent never comes before it. The final hold authorises it again (idempotent).
- C3: a retried commit replaces the receipt atomically, so the newest admitted facts win. The receipt is not written for an included-release install.
- C3 "newer metadata omits or supersedes": the protected targets floor (version and digest) tells me whether newer metadata was seen. I read it from the retained cached targets bytes, but only when their digest matches that floor. A different descriptor means "superseded" and a missing descriptor or package means "revoked". Newer metadata I cannot inspect (absent, mismatched, or announced but never fetched) also refuses.
- C3: the `Reopening` reservation now includes `OFFER_LOOKUP_RESERVATION_BYTES`. I did not add a new `Progress` variant.
- C4: I verify the included release as of its earliest role expiry, in an isolated trust copy. Signatures must still hold under the current root, rollback history is still enforced, and durable floors don't change.
- C4, online: if refreshed metadata names a newer release, the exact included release installs and the updater brings it current. Withdrawn or lower releases are refused.
- C4, offline: the explicit choice is the existing Retry after one plain sentence, translated in every installer locale. The retry first tries online once more. A dedicated two-button page would belong to the UI owner; the typed `IncludedVersionOffered` and `IncludedInstallChoice` are ready for it.
- `MetadataSet` is now `Copy`.
- Force-push was refused, so the branch stays fast-forward: a layout merge with main's tree, then clean commits. The snapshot's content lives only in those commits.

## Commits
- fa9db781 merge: move lane/trust into main's monorepo layout under Substitute
- 9aafce0e fix(updates): keep update trust readable while an update is staged, and refuse it if signing authority changes meanwhile (A2)
- dfc84781 feat(updates): start a prepared update offline once its signed metadata has expired (C3)
- 83279d05 feat(setup): let a downloaded installer install even after its built-in update information expires (C4)
- a001adda test(updates): judge offline receipt activation by the started trial, not by a later run-lock probe

## Verification
- `cargo xtask governance` passes; `cargo fmt --check` is clean.
- `cargo test --locked -p sugarsubstitute-update`: 53 passed, including 12 receipt-condition tests and 5 pinned-verification tests.
- `cargo test -p sugarsubstitute-contracts localization` passed.
- Windows-target check, Clippy and Rustdoc pass for supervision, update, contracts and sugarsubstitute.
- CI run https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43340243980 failed on the baseline only.

## Next
- Merge `origin/lane/ci` once it is green, rerun CI and fix any failures in the new Windows tests.
- G (release tooling): keep serving `N.root.json` with a clean 404 after the newest root. Installer refresh and discovery rely on it.
- UI owner (optional): replace the retry sentence with a two-button page that sends `InstallerMetadataChoice::Included`.
