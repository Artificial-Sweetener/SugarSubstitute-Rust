# Lane report: release (G2, G3)

## Unfinished or unproven

- "Substitute verification" and "Sugar quality" are red here only at main's known baseline
  failures, which stop both gates before this lane's code (see Verification). Neither gate has run
  this lane's Rust or Sugar tests on GitHub yet; merge `lane/ci` once it is green.
- On main, `sugarsubstitute-execution` doesn't compile on Linux (unchanged here), so workspace
  clippy fails in this container. The schema-3 fixture edits in `crates/sugarsubstitute` tests only
  get compiled on Windows CI.
- `release.yml` can only be dispatched once it is on main; tests execute its decision script.
- Real publication has never run, by design; `RELEASES_ENABLED` is unset.
- SugarSubstitute declares no baseline and does not release; its first version is the maintainer's
  call (`lib/components.mjs`).
- No CI builds `.ssupdate` packages or descriptors, and `release.yml` doesn't call the
  update-repository workflow yet. Not created yet: `.github/update-repository/root.json`, the
  `substitute-updates` release, the `update-signing` environment, the `UPDATE_*_KEY` secrets.
- Builds must embed `release_notes` = `…/releases/tag/substitute-v{version}` of this repository; no
  pipeline here produces the distribution file yet.
- HANDOFF: C3, C4 and F10 must land before the D5 lifetimes can be relied on.

## Decisions made

- `release_source.json` names this monorepo: tags are pushed here and releases use `--verify-tag`,
  so no other repository could get the asset. A SugarCubes split repository is a later decision.
- SugarCubes continues from a declared baseline, 0.15.3 at `a839f593`, checked against its
  `pyproject.toml`. The first real release tags it.
- Dry runs use a disposable clone with no remote or credentials, because semantic-release demands
  push access even in dry-run mode.
- Notes use conventionalcommits preset 9.3.1, so `type!:` is listed as breaking (major 10 needs a
  newer writer).
- The update-repository workflow is also gated by `RELEASES_ENABLED`. Keys are hex Ed25519 seeds; the
  trusted root is committed as public data; root expiry warns at 60 days.
- Main already contains the WIP's base, so the history is rebuilt directly on main and the WIP
  snapshot is gone.

## Commits

- `42b92e09` fix(updates): look for SugarCubes releases in the repository that tags them
- `7e161bf6` build(updates): require each build's update distribution to name where its release
  notes are published
- `fec56b4d` build(update-repository): sign the update repository with its scheduled lifetimes and
  prove the result with the app's own verifier
- `ae3f48b0` build(release): release SugarCubes and SugarSubstitute separately from their own commits
- `6cf2419f` ci(release): release only when dispatched and switched on, and prove the release and
  signing tooling on every change

## Verification

- [Release tooling](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43338487449):
  green. 36/36 Node tests, 5/5 Python tests with comfy-cli 1.26.0, and a dry run that reached
  SugarCubes 0.16.0, as planned. Its first run failed because the harness leaked the runner's CI
  identity into synthetic releases; that is fixed in `ae3f48b0`.
- [Substitute verification](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43338487380):
  all three OSes stop at `resolve third_party/comfy-desktop/…/download.ts`, as main does.
- [Sugar quality](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/actions/runs/43337971962):
  ubuntu fails with "22 errors in 11 files" from mypy, and windows with "Select exactly one
  installation of the declared VS2022 product". Both are main's exact failures.
- Local (Linux): `cargo test --locked -p sugarsubstitute-update -p sugarsubstitute-update-repository`
  gives 42 + 11 passed, including a 400-day schedule that `RepositoryTrust` accepts every day.
  `cargo xtask governance` passes (submodule checked out). Clippy and Rustdoc are clean with
  `-D warnings` for the changed crates. Sugar: 251 pytest (self_update, pack_install,
  governance) and the release-versions Jest test pass after `npm ci` from the pruned lockfile.

## Next

- Integrator: merge `lane/ci` into this branch when it is green, then expect all three workflows
  green. The Sugar link above is for the pre-rebase SHA `ba1a71f3`, whose Sugar tree is identical.
- Maintainer: choose the SugarSubstitute baseline, then generate keys offline, run `init`, publish
  the result and commit the root.
