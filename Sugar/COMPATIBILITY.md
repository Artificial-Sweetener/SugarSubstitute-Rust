# ComfyUI compatibility

The upstream workflow compares the same SugarCubes source with its development control, the three newest stable ComfyUI releases and their shipped frontend wheels, the latest standalone frontend on the latest release, and upstream HEAD when it differs. The [control](tests/comfy_contract/support/control.json) records the exact development commit and frontend.

## Reproduce an incident

From `Sugar`, use a Git checkout at the reported ComfyUI revision, Python 3.12 and Node. These Windows commands use a separate CPU environment and keep evidence on the data drive.

```powershell
$comfy = 'E:\devprojects\ComfyUI'
$data = 'D:\SugarSubstitute\comfy-compatibility'
$tempFolder = New-Item -ItemType Directory -Force "$data\tmp"
$env:TEMP = $tempFolder.FullName
$env:TMP = $env:TEMP
$python = "$data\host-env\Scripts\python.exe"
python -m venv "$data\host-env"
& $python -m pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cpu
& $python -m pip install -r "$comfy\requirements.txt" -r requirements.txt
npm ci
npx playwright install chromium firefox
python -m tools.compatibility.host --comfy $comfy --python $python --output "$data\replay-01"
```

For an independently updated frontend, install the exact wheel in that environment before replay, for example `& $python -m pip install comfyui-frontend-package==1.59.15`. Append `-- --project chromium --grep "44px toolbar"` to run one scenario. The runner stages the pack and core sources in a private profile, seeds a small Cube catalog, and starts a loopback Comfy server. Each run requires a fresh output directory. It retains package versions, the host log, upstream source locations, assertion results and browser failure artifacts.

## Locate a dependency and its intended outcome

[The source index](tests/comfy_contract/support/host-dependencies.json) lists host-facing expressions by production file. Generate the line-level ledger with `python -m tools.compatibility.inventory --output "$data\touch-points.json"`. Entries include source locations, surrounding intent where available, API classification, local test imports and directly assigned live contracts. An empty contracts list identifies a gap in direct live coverage. A local test import helps locate characterization tests. Run an area's existing tests with `npm run test:run -- <area>`. The inventory test detects changes in the source index. Review the changed dependencies before running `python -m tools.compatibility.inventory --write-index`.

[The contracts](tests/comfy_contract/support/host-contracts.toml) describe expected outcomes, source ownership and repair guidance. [The scenarios](tests/comfy_contract/integration/) exercise installed releases. [The reference artifacts](tests/comfy_contract/support/reference-images/) preserve the control. The fixtures and assertions give the authored values, geometry and state transitions. Inspect `checks.json`, the expected/actual/diff images and the retained browser trace. Open a trace with `npx playwright show-trace <trace.zip>`. [ComfyUI extension hooks](https://docs.comfy.org/custom-nodes/js/javascript_hooks) document the lifecycle entry points.

The [scope ledger](tests/comfy_contract/support/source-scope.json) also tracks runtime sources where the scanner found no host expression. Adding or removing an authored runtime file requires a scope review. Each complete host run records measured Python source lines and Chromium function calls under `source-coverage`. Its `coverage-index.json` distinguishes direct outcome assertions from files entered during a scenario and files with no observed execution. Firefox runs the user-visible assertions independently.

Each browser scene also measures the literal DOM selectors and CSS custom properties in the inventory. The comparison lists lost element types, ARIA roles and changed token values in `presentation-changes.json` when the same scenario passed on both builds and both control browsers agreed on the original value. Each entry names the affected consumer and the scenario that reproduced the change.

Beside the line-level inventory, `local-characterizations.json` maps imported production modules to the repository tests that describe them. It records each test's declaration and line. Use those examples to recover intended behavior when investigating an upstream change in an adapter.

The comparison records changes to consumed Python implementations and frontend sources in `upstream-review.json`. A changed upstream dependency with a SugarCubes consumer lacking a direct live assertion triggers a review alert. The source hashes, callable locations, selector names and scenario evidence help maintainers decide whether the change affects a user. A matching hash or a passing fixture covers only the code or behavior it checks.

To compare local results, place one complete host run in `control/` and each candidate in a sibling directory. Run `python .github/compatibility/report.py --evidence <parent-directory>` from the repository root. Read `compatibility-report.md` in that directory. An incident yields a nonzero exit code.

## Alerts and recovery

[The workflow](../.github/workflows/sugar-upstream-compatibility.yml) polls at minutes 13 and 43 each hour on the default branch. GitHub scheduling and runner availability affect delivery time. A changed upstream matrix or SugarCubes revision starts a fresh comparison. A trusted default-branch run opens one incident issue and updates it when the failure set changes. A fully green comparison closes the issue. Pull requests produce a comparison artifact. An unavailable control, missing checks or unavailable source evidence leave qualification incomplete. The scheduled monitor retries unchanged incomplete results in the next six-hour UTC window. A new upstream or SugarCubes revision is eligible at the next half-hour poll. Manual dispatch always retries immediately. Source changes identify dependencies requiring review. Browser results identify observed divergences.

The repository needs Actions and Issues enabled. Scheduled runs start when the workflow reaches the default branch. Maintainers receive notifications according to their GitHub issue subscriptions and notification settings.

## Prove the failure path

With the recorded control and frontend installed, run the drill from the repository root.

```powershell
python .github/compatibility/drill.py --comfy $comfy --python $python --output "$data\toolbar-drill-01"
python -m unittest discover -s .github/compatibility -p 'test_*.py'
```

The drill serves the original frontend and a disposable copy with a 52-pixel toolbar constraint. It requires the original 44-pixel check to pass, the altered check to fail, and the normal incident report to contain both measurements. The issue lifecycle test verifies creation, deduplication, change notification and closure. Reference updates require the recorded control and review of the intended behavior change.
