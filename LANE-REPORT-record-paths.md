# Lane report: record-paths

Stopped on the coordinator's instruction; a local agent takes over.

## Unfinished and unproven

1. **The four named tests have not yet run green in CI.** Verification run [43372415444](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43372415444) stopped at the Windows `--no-default-features` Clippy leg (`5f435b79` on `lane/ci-next`, as the coordinator reported), before any test ran. They fail through the owner fixed here, and the owner's own tests now pass on Windows (see Verification).
2. **Rebase onto `lane/ci` is done locally but not pushed.** The coordinator asked for a normal push, so this branch stays on `lane/ci-next`. The local branch `rebased-on-ci` in this container holds the same fix on `origin/lane/ci`, and governance passes there. Before merging, cherry-pick `6bc94bed` and `e88cdf5d` onto `lane/ci` and skip `e54c62a8`.
3. **The same short-name cause breaks two more owners, outside this lane's files.** I didn't touch them:
   - `setup_storage/entrypoint/native.rs` `target()`: about 215 supervision tests fail with "The existing shell entry has a different working directory". The shell normalises the stored shortcut target to its long spelling, but keeps the working directory as the literal short string, so `target.parent() != working directory`.
   - `qualification_artifact/native.rs`: 3 tests fail with `preview_artifact_resolved_namespace`. An expected path built from the short `TEMP` is compared as a string with `GetFinalPathNameByHandleW`'s long result.
4. **Unrelated failures:** 7 `signed_installer` tests fail with `Error: Missing`, and one `qualification_artifact` cancellation assertion fails. Cause unknown.

## Decisions made

- **Locations are compared by identity, not spelling.** `selected_parent` now requires the same drive letter and the same native object (128-bit file ID) at every pinned component. A `subst` drive alias still fails. 8.3 names, letter case and the verbatim `\\?\` prefix are accepted, and custody keeps the resolved long spelling. Reparse points were already refused by pinning.
- **The regression test needs a volume that generates 8.3 names.** Windows system volumes and GitHub runners do. No unprivileged API can create a short name, so the test asserts that the alias exists rather than skipping.
- **A real `DefineDosDeviceW` drive alias proves the check is not loosened.** It is a per-session alias on an unused letter, removed explicitly afterwards.
- The commit email is the one in CLOUD-RULES (`@`); the brief spelled it with a full-width `＠`.

## Commits

- `6bc94bed` fix(supervision): open record folders reached through 8.3 short names
- `e54c62a8` ci(record-paths): run the supervision suite on Windows before and after the fix (temporary proof; removed again in `e88cdf5d`)
- `e88cdf5d` test(supervision): refuse drive-alias record parents below the alias root only

## Verification

- Proof run [43372415609](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43372415609) on windows-2022:
  - **TEMP:** prints `TEMP=C:\Users\RUNNER~1\AppData\Local\Temp`, which resolves to `runneradmin`. This confirms the hypothesis.
  - **Without the fix:** every `durable_storage::records` test fails with "Record parent must use its resolved local location", `short_name_parents_open_at_their_resolved_long_location` included.
  - **With the fix:** no record-parent refusal remains anywhere in the supervision crate, and the short-name test passes.
  - **Drive-alias test:** failed in this run because of the drive-root case, which `e88cdf5d` removes. Not re-run since.
- Local: `cargo xtask check` passes on this branch. Windows-target Clippy (`x86_64-pc-windows-gnu`, `-D warnings`) is clean for the supervision crate.

## Next

- Local agent: run `cargo test -p sugarsubstitute-supervision durable_storage::records` and the four named `sugarsubstitute` tests on Windows, or dispatch `lane/ci`'s test inventory on the rebased branch.
- Assign the shell-entry and qualification-artifact spelling comparisons (item 3) to their owners. They need the same idea: compare identity or the resolved spelling, never the caller's spelling.
