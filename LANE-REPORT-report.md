# Lane report: report (D4w)

## Unfinished
- **Export writer (coord).** Export stays hidden until `StartupHost` implements `update_report_export()` in `startup/window_content.rs`. The trait changed: `UpdateExport::begin(destination, report: &str)`. The writer stores `report` as UTF-8 unchanged, through `ReportFile` under export admission. Its tests should check that the written bytes equal the text it was given.
- **SugarCubes report (coord + packinstall).** In `window_content.rs`, implement `fn pack_update_failure(&self) -> Option<&PackUpdateFailure>` from a field filled at window start from `PackUpdateReport::parse(state/pack-updates/SugarCubes/report.json)` or `PackUpdateStatus::Finished`. `w` is `messages.pack_updates`:
  - `Failed`: `new(to, Some(w.steps[step]), lines, [code, reason])`
  - `Restored`: `new(from, None, lines, [reason])`
  - `CouldNotCheck`: `new(None, Some(w.steps[0]), lines, [])`
  - anything else: `None`

  `lines` is `report.lines(w)` minus the step line, which the summary already shows. `new` redacts and bounds everything it is given.

## Unproven
- The desktop report-lifetime tests can't run under Wine: DirectWrite returns `E_NOTIMPL`, and integration's untouched report tests fail the same way. That covers 3 of my tests: Copy equals Export, nothing private in what is copied or exported, and Export hidden without a writer. Windows CI can't link test binaries yet (baseline).
- Native capture of the combined report over the main window.

## Decisions made
- One report covers both failures, the application first. The caption warning also shows for a SugarCubes failure. A ready update still outranks either failure.
- Export takes the text from the surface, so it matches Copy byte for byte and is in the user's language. I removed the English `UpdateReport::to_text`, which nothing used.
- Redaction lives in `attempt/redact.rs` and is a fixed point, so the attempt channel's `sanitize(cause) == cause` check holds. It's stricter now (paths with spaces, relative, home and `%VAR%` paths, host:port, IP and e-mail, credential values, tokens, control characters). An older helper's cause could be refused, but none has shipped, so I didn't bump the version. After release, a change to `sanitize` needs a protocol bump.
- The surface redacts every cause again before showing it.
- SugarCubes wording stays with its owner (`PackUpdateMessages`). I added only the heading and explanation: `pack_update_title` and `pack_update_message`, in all 5 locales.
- **Hazard:** `host_contract.rs` is now at exactly 500 of its 500 allowed code lines. The next addition must extract code first.

## Commits
- 5a5e8ebf fix(updates): keep paths with spaces, addresses and credentials out of update reports
- 08178c01 feat(updates): show a failed SugarCubes update in the update report and export exactly what it shows

## Verification
- `cargo xtask check` on Linux: governance and fmt pass. It then stops in `sugarsubstitute-execution` (Linux baseline). Fingerprints were resynced with `cargo xtask fingerprints`.
- Windows-target Clippy (`-D warnings`), all three legs: pass.
- `cargo test -p sugarsubstitute-contracts`: 324 passed, including 9 new redaction tests and 2 `PackUpdateFailure` tests.
- Desktop tests under Wine: all 9 `main_updates` tests pass, including the new SugarCubes warning test. 5 of the 8 update report surface tests pass; the other 3 hit the Wine gap above.
- CI: [Substitute verification run 43369278225](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43369278225). Expect only the baseline failures.

## Next
- **coord:** the two items under Unfinished. Expect a small conflict in `main_updates.rs` (my change is only in `UpdateFeed::read`) and a fingerprint resync in `catalog.toml`.
- **Integrator:** rerun the 3 desktop report tests once Windows test linking works.
