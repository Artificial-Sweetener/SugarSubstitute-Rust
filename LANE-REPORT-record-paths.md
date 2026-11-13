# Lane report: record-paths

## Unfinished and handed over

1. **`cargo xtask check` stops at a failure outside this lane.** Governance, formatting and every supervision leg pass. The check then stops at `cargo clippy -p sugarsubstitute --no-default-features`: `native_canvas/session/tests.rs` names `crate::job_deadline`, and `5f435b79` gates that module behind `native-composition`. The fix belongs to the owner of `5f435b79`.

## Decisions made

- **Locations are compared by identity, not spelling.** A location is the drive letter plus the native object (volume and 128-bit file ID) at every pinned component. Record parents (`6bc94bed`) and shell entries (`c4cc1d06`) both use it. 8.3 short names, letter case and the verbatim `\\?\` prefix are one location. A drive alias or another folder is a different location and is still refused.
- **`Directory::same_location` accepts identical spellings without opening anything.** An absent folder still matches its own spelling, as before. Differently spelled folders must both exist before they can be compared.
- **`entrypoint::selected` keeps the caller's spelling.** It returns the generation as `<root>\releases\<name>`, after proving it is the same location as the folder the shell entry targets. That replaced a string comparison of two `canonicalize` results.
- **Regression tests fail without the fix on every Windows machine.** Each one also uses an uppercase spelling and a `\\?\` spelling, which differ from the resolved spelling on every volume. The 8.3 spelling is an extra case wherever the volume generates short names. I checked this on X:, which has none.
- **Not changed:** `qualification_artifact/native.rs` compares a final path with the caller's spelling. Its three tests failed on GitHub but pass here under the short `TEMP`, so I left it alone. Seven `signed_installer` tests failed once locally with `Error: Missing`, then passed in every later run under the same `TEMP`.
- **The temporary proof commit `e54c62a8` and its workflow are dropped.** `e88cdf5d`'s drive-alias test change is carried in `1316b114`. The branch is based on `6bc94bed` and nothing is pushed.

## Commits

- `6bc94bed` fix(supervision): open record folders reached through 8.3 short names (cloud)
- `1316b114` test(supervision): prove every record-parent spelling opens at its resolved location
- `c4cc1d06` fix(supervision): recognise shell entries reached through 8.3 short names

## Verification

All runs were local, with `TEMP` and `TMP` set to `C:\Users\imkno\AppData\Local\Temp\claude\A87AA5~1\DFE83C~1\SCRATC~1\LONGNA~1`. Logs are in `D:\SugarSubstitute-Rust\update-system\w-record-paths\`.

- **Before the shell-entry fix:** 223 of 1466 supervision tests failed. 215 of them failed with the shell-entry working-directory refusal (`sup-all-short-before.*`).
- **After the fix:** all 1467 supervision tests pass (`final-sup-short.*`).
- **`sugarsubstitute` app:** all 41 `input_documents` and `setup::worker` tests pass, including the four that failed on GitHub (`final-app-short.*`).
- **Without the product changes, under the default `TEMP` on X::**
  - the records spelling test fails with "Record parent must use its resolved local location" (`sup-beforefix-x.*`);
  - the shell-entry spelling test fails with the working-directory refusal (`native-beforefix.*`).
- **Clippy:** supervision passes with `-D warnings`, all targets and all features (`final-clippy-sup.*`).
- **`cargo xtask check`:** passes up to the item 1 failure (`xtask-check.*`).

## Next

- Merge `1316b114` and `c4cc1d06` after `6bc94bed`, and skip `e54c62a8`, `e88cdf5d` and `e32a81f6`.
