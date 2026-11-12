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
