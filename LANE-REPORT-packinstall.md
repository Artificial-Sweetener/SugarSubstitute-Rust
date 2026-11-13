# Lane report: packinstall

## Unfinished or unproven
- **Unproven: the headless real run with ComfyUI.** It should cover a fresh setup, a splash update (registry and GitHub), every verdict including both page failures, the rollback, the splash line and the caption line. It needs the shared proof lane.
- **Proven on Windows now:** the junction vectors (`ownership_vectors`, `replacement_vectors`), the WinHTTP loopback check, `fresh_setup_installs_sugarcubes_as_a_registry_install`, and the `sugarsubstitute` startup, child, parent, entry-runner and update-status tests.

## Decisions made
- **Package:** `sugarcubes/SugarCubes.zip` carries the exact registry archive. The folder form is gone (unreleased).
- **Accessor:** both suites read vectors only through `drive()`, and the xtask enforces it.
- **Lock:** the host takes SugarCubes' own `.sugarcubes/updates/.lock`.
- **Gate:** the update holds source inspection and launch.
- **Readiness:** backend ready counts as an unobservable start. A main reveal counts as the page loading.
- **Page failure (wire 15):** the child's attempt settlement carries one of three values: none, SugarCubes (the page reported SugarCubes unavailable or failed) or unattributed (any other failure that kept the page from loading).
  - SugarCubes: the retry is held behind a restore. With no retry, the next start restores.
  - Unattributed: a host-only `PageUnconfirmed`. A later page of the same run still keeps the version; otherwise the next start restores the previous one. So a version whose page never loads runs in only one launch: the second launch runs the previous version and says "The previous SugarCubes version is running."
  - A start whose page never decides at all (closed in the splash, power cut) counts toward the three-unwitnessed limit, so no judgement stays open and blocks updates. The shared vectors (one restart is pending) are unchanged. Python is unchanged too, because plain ComfyUI may run without a page.
  - A restored version is never reinstalled.
- **Display:** I merged `origin/lane/coord` to use its kind-22 update status. The combined lifecycle wire is 15.
  - The SugarCubes status travels inside `UpdateStatus` (`sugarcubes`, bounded on decode).
  - The splash shows "Updating SugarCubes…" while the update holds the start, ahead of "Updating to version X…".
  - The finished report's line is a one-time caption notice.
  - When the report needs attention (couldn't check, couldn't update, stopped, previous version running), the update button opens the window's update report. The report shows the SugarCubes lines after any app attempt.
  - The report the window shows is the same value that is written to `report.json`.
- **Attached ComfyUI:** read-only check, never written. Its report is now shown too.
- **Formats:** `desktop-update-status-v1` gains a defaulted field and the journal gains a phase. Both are unreleased, so there is no migration.

## Commits
See `git log origin/lane/packinstall..local/packinstall-w`.

## Next
- **release:** ship `SugarCubes-<version>.zip` as `sugarcubes/SugarCubes.zip`.
- **Integrator:** this branch contains lane/coord. Merge it after coord, or instead of it.
