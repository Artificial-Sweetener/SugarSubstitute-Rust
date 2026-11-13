# Rules for cloud lanes

You are one lane of a coordinated project on the SugarSubstitute monorepo. The coordinator is a local Claude session that is offline while you work, and you cannot message it. Everything you need is in your brief and on this branch (`coordination`):

- `HANDOFF.md`: the authoritative outcome, settled decisions D1–D22, work items and done criteria. Read sections 2 and 4 plus every item your brief names.
- `DECISIONS.md`: the coordinator's decisions C-1… and the maintainer's M-1…. These are settled; follow them.
- `FINDINGS-storage.md`, `FINDINGS-host.md`, `TIMEOUTS-INVENTORY.md`: investigation results your brief may point to.

Paths in these documents that start with `D:\`, `E:\` or `X:\` refer to the maintainer's PC, which is unreachable. Ignore instructions about local wrappers, build slots, sccache, hidden desktops, proof lanes, the live board and `notes.md`; this file replaces them.

## Never stop to ask
Nobody is watching your session: the maintainer is away and the coordinator is offline. Never end a turn with a question, a request for confirmation or a "should I…?".
- Settled answers: HANDOFF.md, DECISIONS.md, this file and your brief.
- Anything they don't settle: choose the most reliable professional design for ordinary home Windows PCs, record the choice and your reasons in your lane report under "Decisions made", and keep going.
- An item genuinely blocked (it needs another lane's code, a native proof, or something only the maintainer can provide): do everything around it, record it under "Unfinished", and move to your next item.
- Your session ends only when every item is done or recorded as blocked, and your lane report is committed and pushed.

## Design
- Decisions D1–D22, C-n and M-n are settled. Do not reopen, swap or simplify them. If your work cannot meet one, stop that item and explain why in your lane report.
- Build the most reliable professional design, not the minimum fix: remove root causes, prefer event-driven waits over polling, use typed errors, handle cancellation and teardown.
- Users are regular consumers on ordinary home Windows 11 PCs (NTFS, non-admin). Nothing may depend on enterprise or server Windows, admin rights, special file systems or anything a home user would have to set up.
- Installation is self-contained: `<setup drive>\SugarSubstitute`, `SugarSubstitute.exe` alone in the root, user data in `user\`, nothing in AppData or Programs.
- User-facing words are plain: "ready", "up to date", "couldn't check", "previous version is running". Never profile, lock, lease, instance, election, trial, candidate or quarantine in user-facing text. All Substitute product text goes through the localization owner.
- Substitute is unreleased: no compatibility layers or migrations for Substitute-side formats (where a gate requires a registered migration, do the minimum and say so). SugarCubes has users: saved workflows, ComfyUI settings and frozen names must keep working.
- The old Python app is not available to you. Don't model locks, leases, updating or shutdown on it.

## Repository
- Read `AGENTS.md`, `Substitute/AGENTS.md` and `Sugar/agents.md` (plus nested AGENTS.md files in areas you touch) before changing anything.
- Work only on your lane branch. Push only to it. Never push to `main`, never force-push another lane's branch, never create tags or releases, never set the `RELEASES_ENABLED` variable.
- Two lanes never write the same files. Stay inside the files your brief names; if the robust design needs another lane's files, do the minimum interface on your side (a trait plus a fake) and record what the other side needs in your lane report.

## Old-layout branches
`main` has `Substitute/` (the Rust workspace) and `Sugar/` (the node pack). If your branch still has the Rust workspace at the root (a root `Cargo.toml`, no `Substitute/`), never merge main normally and never keep a `-X subtree` merge as the result. Convert it:

```sh
git merge --no-edit -X subtree=Substitute origin/main   # resolve any conflicts, then commit the merge
merged=$(git rev-parse HEAD); lane=$(git rev-parse HEAD^1); main=$(git rev-parse HEAD^2)
export GIT_INDEX_FILE="$(mktemp -u)"
git read-tree "$main" && git rm -r -q --cached Substitute && git read-tree --prefix=Substitute/ "$merged^{tree}"
tree=$(git write-tree); unset GIT_INDEX_FILE
c=$(git commit-tree "$tree" -p "$lane" -p "$main" -m "merge: move $(git branch --show-current) into main's monorepo layout under Substitute")
git reset -q --hard "$c"
git diff --quiet "$main" HEAD -- Sugar && echo "Sugar/ matches main"
```
Then run `cargo xtask check` from `Substitute/` and resync any governance fingerprint it reports, using the tooling the gate names. Never hand-edit fingerprints to pass.

## Commits
- `type(scope): changelog-ready outcome`; `feat`/`fix`/`perf` for user-facing behaviour, `refactor`/`test`/`build`/`ci`/`chore`/`docs` otherwise. The message ends with its body: no Co-Authored-By or other attribution trailer.
- Never `--no-verify`, never change git config globally, never install hooks, never `git stash`.
- Commit coherent outcomes with their tests. The first commit on your branch may be a work-in-progress snapshot; rewrite it into real commits where practical (rewriting your own lane branch and force-pushing it is allowed).
- Keep LF line endings. The AGPL licence change belongs to a different lane.

## Verification
- In your container: `cargo xtask check` from `Substitute/` plus focused `cargo test --locked -p <crate> <filter>` for the area you changed; `npm run check:staged` from `Sugar/` for pack changes. When you change doc comments, run `cargo doc --locked -p <crate> --no-deps` with `RUSTDOCFLAGS=-D warnings`.
- Windows-only code and tests cannot run in your container. GitHub Actions is the verifier: push, then start "Substitute verification" (and "Sugar quality" for pack changes) on your branch with `workflow_dispatch` (`gh workflow run ... --ref <branch>`) if push did not trigger it, and read the results with `gh run view`. Fix every failure at its root.
- Never weaken a test, gate, budget, waiver limit or assertion. Every fix lands with a regression test that fails before and passes after.
- No wall-clock pass/fail: a test must never pass or fail because a fixed number of seconds elapsed. Use injected or fake clocks, or wait for the observed outcome. Product network operations still have bounded timeouts, tested with fake clocks.
- **Main's CI is red before any lane change.** Runs 43327781938 and 43327786110 on main failed for reasons that aren't yours:
  - Substitute verification on every OS: xtask cannot resolve `third_party/comfy-desktop/src/main/lib/download.ts` (the submodule isn't checked out in CI).
  - Sugar quality on ubuntu: mypy flags Windows-only APIs in platform-guarded modules.
  - Sugar quality on windows: "Select exactly one installation of the declared VS2022 product".
  Don't fix these in a lane branch. A dedicated cloud session repairs the baseline on branch `lane/ci`. The integrator merges `lane/ci` into `integration` first, as soon as its LANE-REPORT-ci.md says both workflows are green; until then it treats these three failures as the known baseline. When `lane/ci` is green, every lane merges `origin/lane/ci` into its branch. Until then, judge your work by your own local `cargo xtask check` and focused tests, plus any CI failure that is new compared with main's baseline. Once the baseline fix exists, merge it into your branch and get fully green.
- Native proofs on a hidden Windows desktop cannot run in the cloud. List each one your items need under "Unproven" in your report.

## Lane report
Before you finish, commit `LANE-REPORT-<lane>.md` at the repository root of your branch, under 500 words, in this order: unfinished or unproven items first; commits (hash and subject); verification run (CI run links and results, local commands); what other lanes or the coordinator must do next.
