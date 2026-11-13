# Lane report: retire

## Unfinished or unproven
- **Windows tests not executed.** All changed code is Windows-only. Here it was type-checked, clippy'd and rustdoc'd for `x86_64-pc-windows-msvc` only. CI stops at the known submodule baseline; once `lane/ci` is green, merge it here and rerun.
- **Unproven native assumption:** renaming a folder fails while any file inside is open, even with delete sharing. `a_tree_with_an_open_file_is_left_untouched` and `a_generation_in_use_survives_and_is_removed_once_released` rely on it.
- **An interrupted staging generation is only partly reclaimed.** Preparation copies the payload before writing `application-package.json`, so a staging folder superseded before then loses only its records and stage files; its payload stays in the tombstone. Fix in `transaction/candidate.rs` (lane trust): retain the inventory before copying.
- **No native proof** of a real update cycle reclaiming disk on a home PC.

## Decisions made
- The keep set also includes the stable entry's repair source (`.launcher-generation.json`) and the running generation. `rollback` and `repair` generations are removed, because nothing reads their files.
- If the installation has no `update-trust` folder, no updater can prepare, so the turn proceeds under run custody alone. Run custody already excludes the installer.
- Names in `releases\` that are not generations are left untouched and counted, with one warning. `.generation-init-*` folders are retired using only their marker.
- The turn starts at the same point as shell publication: after readiness for an accepted launch, or after the published acceptance of a trial. It gets a 300 s deadline on the execution clock. Interrupted work resumes at the next start, because every turn resumes tombstones first.
- `TreeInventory::stages(prefix)` reclaims owned stage files (`.payload-stage-`, `.record-stage-`, `.setup-entry-stage-`, `.substitute-publication-`).
- The tombstone name is registered as the frozen format `native-retirement-tombstone-v1`.
- Force-push was refused, so a merge commit with main's tree supersedes the snapshot and the work re-lands on top, fast-forward only.
- Fingerprints were recomputed with a script matching `tools/xtask/src/source.rs`, checked against main.

## Commits
- `fe16efc2` merge: move lane/retire into main's monorepo layout under Substitute
- `73197656` feat(storage): retire an owner's files only below an ownership-verified folder, restart-safely
- `71fa5640` feat(updates): remove old app versions nothing can start, so updates never fill the disk

## Verification
- `cargo xtask governance`: passed.
- `cargo xtask check` on Linux fails at `clippy --workspace --all-features` in `sugarsubstitute-execution`, and Linux clippy on `sugarsubstitute-supervision` fails in `command/directory.rs`. Neither file is changed on this branch, so both failures are pre-existing.
- Windows target: clippy is clean for both crates (`--all-targets`, default and all features), and rustdoc for supervision builds with no warnings.
- CI run 43339422045 (Substitute verification) fails on all three OS only at the known baseline (`resolve third_party/comfy-desktop/...`), before tests. Run 43339422184 (Sugar quality) is for pack changes; none here.

## Next for other lanes
- **storage / integrator:** once `pointed_generations` lands, make the body of `LaunchPointerGenerations::pointed_generations` (`setup_storage/generation_retirement.rs`) delegate to it.
- **coord (C7):** on a disk shortage, call `GenerationReclaim::reclaim` through `RunningGenerationReclaim` in the coordinator. A preparation must release preparation custody first, otherwise the turn reports `PreparationBusy`. Read `GenerationReport::freed_bytes` from the result.
- **persist:** stored-base pruning can use `owned_retirement_root` + `OwnedRoot::retire` and `resume`.
