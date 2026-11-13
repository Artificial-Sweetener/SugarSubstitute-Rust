# AGENTS.md

## Repository layout

This repository holds two independently released components.

- `Sugar/` is the SugarCubes ComfyUI node pack. Its `__init__.py` and `pyproject.toml` sit at
  the folder root, which is the exact shape ComfyUI loads and the Comfy Registry publishes. The
  pack installs as `custom_nodes/SugarCubes`. `Sugar/agents.md` and the nested `AGENTS.md`
  files under `Sugar/integrations/` govern everything in this folder.
- `Substitute/` is the SugarSubstitute Rust workspace: the desktop host and the native canvas.
  `Substitute/AGENTS.md` governs everything in this folder.

The repository root stays thin: CI workflows, `.gitmodules`, pre-commit configuration and these
instructions. Never add a `Cargo.toml`, `package.json` or `pyproject.toml` at the root; Cargo,
npm and Python tooling discover their projects by searching parent directories, and each
component (including the separate `Sugar/integrations/gimp/companion` Cargo workspace) relies on
finding none above it.

## Working in a component

- Run Substitute commands from `Substitute/` (`cargo xtask check`, `cargo xtask verify`).
- Run SugarCubes commands from `Sugar/` (`npm run check:changed`, `npm run check:staged`,
  `npm run check`).
- Substitute consumes the SugarCubes host contract directly from `Sugar/integrations/contracts`,
  declared in `Substitute/governance/monorepo/components.toml`. A change there changes both
  components: run both components' gates.
- Commits follow the component's own commit rules. Scope commits to one component unless the
  outcome is a single cross-component change.

## Releases

- Each component releases on its own: tags `sugarcubes-v<version>` and `substitute-v<version>`,
  its own `CHANGELOG.md`, and its own version files. Features and breaking changes bump the
  minor version; fixes and performance changes bump the patch version.
- A commit counts toward a component when it changes that component's folder, or when it is part
  of that component's imported standalone history. A change elsewhere releases nothing.
- A component's first release continues the baseline it declares in
  `.github/release/lib/components.mjs` (SugarCubes: 0.15.3); one with neither a release tag nor
  a declared baseline does not release.
- `.github/release/` owns the release tooling (semantic-release, commit scoping, GitHub
  publication and the SugarCubes registry archive); `.github/workflows/release.yml` runs it,
  only when dispatched. It publishes only when the dispatch asks to and the repository
  variable `RELEASES_ENABLED` is `true`; otherwise it is a dry run in a disposable clone with no
  credentials. Test the tooling with `npm test` and `npm run format:check` from that folder, and
  `python -m unittest discover -s tests -p "test_*.py"` with SugarCubes and the pinned
  comfy-cli installed. Never publish from a local checkout.
