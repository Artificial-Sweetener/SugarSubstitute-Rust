# Lane report: launcher (C5)

## Unproven

- **Notice window**: `desktop/src/headless/notice.rs` only compiles here (Windows Clippy, all three legs). Native proof needed: it appears about 2 s into a slow launch, takes no focus, follows the theme, and closes on hand-off.
- **Text tests**, which fail under Wine (no DirectWrite) like the existing splash text tests: `notice::tests::a_shown_notice_prepares_one_frame_for_the_reported_window` and `render::starting_tests::starting_status_replaces_the_loading_words_in_every_locale`.
- Windows CI on afbd1291 was still running at stop: [Substitute verification](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43373105133).

## Commits

- 3d6f2f0b fix(launcher): bound every launch in seconds and always exit
- 82831eb2 feat(launcher): show a small "Starting SugarSubstitute…" splash when a launch takes more than two seconds
- afbd1291 merge: take lane/ci so Windows CI builds and runs the tests (CI job limit kept at 240 min)

## What changed

- `LauncherBudget` (contracts `startup::launcher`) replaces the one-hour budget: 120 s per launch; update steps may start only in the first 60 s. After that, cached preparation retires and the accepted version starts.
- Waits on other processes were already kernel-event waits (locks 5 s, takeover, adoption); A1 removed the 100 ms re-poll. New: a route lock timeout starts the accepted version instead of failing.
- Always exits: 5 s after its cleanup cutoff (the stable entry: after its deadline), a worker that never returned is abandoned.
- **Bug fixed**: the stable entry never stopped its executor, so `finish_shutdown` always failed and a normal launch could end in a startup failure report. Regression test: `root::tests::a_root_job_that_returns_settles_without_the_exit_grace`.

## Verification

- Windows tests under Wine 9, with an uncommitted local stub for `committed_dump_parser`: `stable_launcher` 44 pass, plus the font case above; splash `a_notice_without_controls_draws_no_caption_buttons` passes.
- Linux: contracts 317 pass; `cargo xtask governance` passed; fmt clean; `cargo xtask check` stops on `sugarsubstitute-execution` dead-code lints (Linux baseline; lane/linux fixes it). Windows Clippy is clean on all three legs.
- Fingerprints recorded with `cargo xtask fingerprints`; new localization and UI-boundary records for `headless/notice.rs`.

## Decisions made

- Words: "Starting SugarSubstitute…", translated in all five locales, shown in English because the launcher reads no configuration (like its startup report).
- No controls: there's no output to show, and it ends by itself within seconds.
- Shown from 2 s until hand-off. The pump builds its Tao loop only when the notice is due, so a fast launch creates no window.
- 60 s/120 s leave room for the longest admitted wait (adoption 32 s, recovery 15 s, selection 5 s). The cutoff never cancels a running trial; doing that mid-adoption could leave two apps.
- Abandonment forgets the executor, because dropping it joins the stuck worker.

## For others

- Integrator: conflicts with lane/entry in `desktop/src/headless.rs` (both add a surface to the pump; my `HeadlessState::closing` matches entry's text exactly) and with lane/packinstall in the `localization.rs` `Messages` struct.
- Release tooling failed on 3d6f2f0b. This lane doesn't touch release tooling.
