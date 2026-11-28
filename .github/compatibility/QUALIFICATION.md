# Initial upstream findings

On November 28, 2026 the Windows CPU qualification exercised SugarCubes from development commit `bca06cfd6228c2135477c655ee35f1528cbb4580`. Each release ran with its own pinned frontend. The table records observations from scenarios shared across the runs.

| ComfyUI | Frontend | Substitute toolbar, Chromium and Firefox | Native projected node loader, Chromium | Spectrum reference card, Chromium |
| --- | --- | --- | --- | --- |
| Development control `a1c421994cdc` | 1.47.11 | 44 px | Loaded the matching Vue renderer | 270 × 299 px |
| v0.44.0 `2c75dc1b379a` | 1.57.6 | Toolbar row missing | Module discovery exceeded its fetch limit | 270 × 297 px |
| v0.45.0 `989d93d0d968` | 1.58.5 | Toolbar row missing | Module discovery exceeded its fetch limit | 270 × 297 px |
| v0.46.0 `a277b5bf213e` | 1.59.6 | Toolbar row missing | Module discovery exceeded its fetch limit | 270 × 297 px |

The two-Cube authoring scenario preserved independent aliases and widths after serialization and replay on all three releases in both browsers. The project storage scenario preserved Unicode text, mask bytes and destination contents after a conflicting move on all three releases.

On the pinned control, Firefox opened the Substitute Settings dialog through the application menu and completed search and close. Its search field was inactive after opening, although the Settings workspace calls `focusSearch()` after mounting. The focus assertion stays enabled and appears as an existing control failure in comparisons. [Settings workspace](../../Sugar/frontend/comfyui/ui/settings/SettingsWorkspaceHost.ts) · [Settings scenario](../../Sugar/tests/comfy_contract/integration/workspace.live.ts)

## Repair evidence

- **Toolbar.** Comfy moved the workflow actions landmark from a button to a group. SugarCubes still searches for `button[aria-label="Workflow actions"]`. Inspect the native action bar in `TopMenuSection.vue`, update landmark discovery in both SugarCubes consumers, and require the shared row and its 44-pixel layout in both browsers. [Toolbar layout](../../Sugar/frontend/comfyui/ui/surface/frame/SubstituteToolbarLayout.ts) · [Override toolbar host](../../Sugar/frontend/comfyui/ui/relations/infrastructure/NativeOverrideToolbarHost.ts)
- **Graph links.** The representative graph exposed its private `_links` collection on the control. The member was absent on all three releases, the separately installed frontend and upstream HEAD in Chromium. The connected red/cyan output fixture still executed with the correct image for each Cube. Check which link collection each adapter reads when crossing a native subgraph boundary. [Boundary resolver](../../Sugar/frontend/comfyui/ui/cube/graph/NativeSubgraphBoundaryResolver.ts) · [Graph queries](../../Sugar/frontend/comfyui/ui/graph/GraphQuery.ts) · [Member comparison](../../Sugar/tests/comfy_contract/integration/host-members.live.ts)
- **Projected nodes.** The installed frontend must supply the native node component, its matching Vue runtime, node-data extraction and socket layout synchronization. The current traversal reaches its 128-module guard on all three Chromium runs. Compare the candidate's actual module graph and capability signatures with the control before changing discovery. Re-run the native component probe, projected field edits and renderer switching. [Module traversal](../../Sugar/frontend/comfyui/ui/surface/ComfyFrontendModuleGraph.ts) · [Capability discovery](../../Sugar/frontend/comfyui/ui/surface/ComfyRuntimeModuleLoader.ts) · [Live probe](../../Sugar/tests/comfy_contract/integration/native-host.live.ts)
- **Visual divergence.** On v0.46.0 the Chromium Spectrum card differed at 2,369 pixels. The PhotoPro card changed from 270 × 259 to 270 × 267 pixels, with 56,753 differing pixels. Compare the CSS bindings and native widget markup against the checked-in reference image, then replay the renderer transition with the same workflow. [Reference images](../../Sugar/tests/comfy_contract/support/reference-images) · [Renderer scenarios](../../Sugar/tests/comfy_contract/integration/renderers.live.ts)

On v0.46.0 with standalone frontend 1.59.15 and on upstream core `5f0d863aa0c5` with frontend 1.59.15, the Chromium toolbar and native renderer probes also failed. The browser queue reported the completed fixture under its authored workflow name on both hosts.

The failure drill imposed a 52-pixel minimum on the upstream toolbar in a disposable frontend. The unchanged control passed. The altered frontend failed, and the incident report contained the expected 44 pixels, the observed 52 pixels, the owning code and the replay scenario. [Fault drill](drill.py)
