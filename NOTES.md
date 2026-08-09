# SugarCubes and Comfy Subgraph Integration Research

## Research goal

Build an implementation-ready understanding of every user-visible or behaviorally significant place where ComfyUI treats a SugarCube as a generic Subgraph. Preserve the premise that a Cube is implemented with Comfy's native `SubgraphNode` and native subgraph definition, and preserve the ability to enter that definition to edit the Cube. For every mismatch, identify the visible trigger, structural classifier, action owner, resulting product risk, Cube-specific behavior, integration seam, and verification coverage.

This document is the working specification for separating Cube product semantics from generic Subgraph product semantics. It does not propose replacing, hiding from, or weakening Comfy's native Subgraph abstraction.

## Investigation and implementation status

The S01-S23 cleanup, native node-picker integration, fresh-placement sizing, and native-interface save-integrity repair are implemented for the installed versions recorded below. `Prompt by Region` was re-saved through the corrected application route as a one-output Cube and verified in SugarCubes, Sugar DSL, and SugarSubstitute. Chrome and Firefox now verify the corrected selection action in both renderers through the loopback-resolving `localtest.me` hostname; the broader picker and pointer-resize matrix remains tracked separately. Findings are tagged as live-observed or source-confirmed so verification can distinguish direct UI evidence from behavior established by exact-version source tracing.

- SugarCubes repository: `E:\ComfyUI\custom_nodes\SugarCubes`, `main` at `4c81d10d74f533abcee4e7e0710c2e27eff5ad49`, package version `0.11.0`.
- Running host: `E:\ComfyUI`, `master` at `a1c421994cdcc5044dbce2bb7628e89386311cc5`.
- Installed Comfy frontend: `comfyui_frontend_package 1.47.11`.
- Exact matching frontend source: `E:\ComfyUI\.codex-investigations\ComfyUI_frontend-v1.47.11`, tag `v1.47.11`, commit `e724849f5`.
- Live host: `http://127.0.0.1:8198`.
- Browser scope: Chrome for the original integration research; Chrome and Firefox for the selection-action tooltip, icon, and renderer regression matrix after a Firefox-specific failure was reported.
- Renderer scope: Nodes 1.0 and Nodes 2.0.
- Representative saved Cube: `Anima/Prompt by Region`, current version `3.2.0`, collection `Base-Cubes`, author `Artificial-Sweetener`.

The live Chrome pass established the selected-Cube toolbox, More Options menu, Cube face, Cube action menu, and native editor entry. Entering the Cube changed the URL to the native subgraph route, proving the custom `Edit Cube` affordance retained native navigation. Chrome's extension-managed loopback policy blocked the literal `127.0.0.1` and `localhost` origins during later work; the loopback-resolving `localtest.me` hostname reaches the same authorized `8198` instance and restored live automation. The corrected selection toolbox was observed after a Nodes 2.0-to-1.0-to-2.0 renderer round-trip.

The pre-change repository baseline passed `npm run check` with 150 test suites and 904 tests. The complete post-repair repository gate passed again on 2026-08-05 after the native selection-icon and Firefox tooltip-loop repair, with 180 TypeScript suites and 983 TypeScript tests plus the full Python suite, strict Python/TypeScript checks, standards audits, formatting, linting, and generated-build verification. Live browser results remain separate evidence and are recorded only after direct observation.

## Product and structural contract

A Cube has two identities at the same time:

1. **Structural identity:** a real Comfy `SubgraphNode` backed by a real native subgraph definition. This provides boundary inputs and outputs, execution, prompt flattening, workflow serialization, navigation, native graph editing, promoted-widget mechanics, renderer behavior, and nested traversal.
2. **Product identity:** a SugarCube with a stable Cube identity, `.cube` persistence, version and provenance metadata, save and read-only policy, sync behavior, custom presentation, and a separate library lifecycle.

SugarCubes records product identity without removing structural identity:

- `frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.ts` creates a node from the registered native subgraph type, requires `isSubgraphNode() === true`, and validates the native definition.
- The wrapper carries `properties.sugarcubes_kind` (`cube` or `cube_draft`) and `properties.sugarcubes_cube`.
- The definition carries corresponding Sugar metadata under `subgraph.extra`.
- `isCubeNode()` is the authoritative product discriminator.

Comfy's generic affordances only consult `isSubgraphNode()` or `instanceof SubgraphNode`. A Cube correctly satisfies both. The mismatch occurs when structural classification is allowed to decide product labels, availability, and persistence routing without consulting `isCubeNode()`.

The governing rule is:

> Use native Subgraph mechanics inside a Cube, but route product-level affordances through Cube semantics whenever the operand or active editor root is a Cube.

Entering the native definition is intended. Native Subgraph execution and serialization are intended. Publishing, unpacking, converting, naming, describing, or storing a Cube as though it were a generic Subgraph is not intended.

## Causal model

```text
Cube node
  |-- structural identity: isSubgraphNode() / instanceof SubgraphNode
  |     |-- selection flags
  |     |-- quick selection toolbox
  |     |-- Vue More Options menu
  |     |-- native LiteGraph node and canvas menus
  |     |-- core command and keybinding handlers
  |     |-- right-side Subgraph editor
  |     |-- Subgraph Blueprint persistence
  |     `-- execution, traversal, serialization, and navigation
  |
  `-- product identity: Sugar markers + Cube metadata
        |-- CubeNodeCatalog and lifecycle
        |-- Cube face and editor chrome
        |-- .cube save/version/sync flows
        `-- Cube metadata, read-only, historical, and dirty-state policy
```

The structural branch is necessary. The defect is that Comfy's presentation and application actions do not ask the product branch to refine their decision.

The problem is also not confined to one menu. The final visible action lists are assembled from independent sources:

1. hard-coded selection-toolbox Vue components;
2. Vue `useSelectionMenuOptions()` entries;
3. native `LGraphCanvas.getNodeMenuOptions()` entries;
4. `litegraphService.ts` additions to every `SubgraphNode`;
5. native canvas-context entries for multi-selection;
6. global commands and keybindings;
7. additive extension menu and toolbox entries.

`useMoreOptionsMenu.ts` merges native and Vue entries. `contextMenuConverter.ts` removes duplicates by visible label and prefers the Vue entry. Filtering one source cannot produce a complete or stable fix.

## Live surface inventory

With the saved Cube selected in Nodes 2.0, the floating toolbox showed:

- `Delete Selected Items`
- `Node Info`
- `Color`
- `Unpack the selected Subgraph`
- `Edit Subgraph Widgets`
- `Publish Subgraph`
- bypass control
- `More Options`

The More Options menu showed:

- Rename, Copy, Duplicate, Pin, Bypass
- `Convert to Subgraph`
- Minimize, Clone
- Node Info, Color
- `Edit Subgraph Widgets`
- `Unpack Subgraph`
- Shape
- Extensions disabled
- `Add Subgraph to Library`
- Remove

The SugarCubes-owned face correctly showed `Open Cube actions`, an `Edit Cube` footer, and Cube actions including `Save cube implementation` and `Save current values as cube defaults`.

## Mismatch catalog

### S01 — Quick action offers `Unpack the selected Subgraph`

- **Evidence:** Live-observed in Nodes 2.0; source applies to both renderers.
- **Why:** `useSelectionState.ts` sets `isSingleSubgraph` when the only selected node returns true from `isSubgraphNode()`. `SelectionToolbox.vue` always renders `ConvertToSubgraphButton.vue` for any selection. That component switches to unpack for a single structural Subgraph and executes `Comfy.Graph.UnpackSubgraph`.
- **Native effect:** `useSubgraphOperations.ts` filters selected items with `instanceof SubgraphNode` and calls `graph.unpackSubgraph()`. Native unpack clones the definition's internal nodes, groups, reroutes, and links into the parent graph, reconnects boundary links, and removes the wrapper. If the wrapper was the last reference, native removal also removes the definition. If other Cube instances share the definition, the definition remains, but the selected managed Cube instance is still destroyed.
- **Sugar effect:** root lifecycle removes the wrapper from `CubeNodeCatalog`. The resulting internal nodes are ordinary root graph nodes without a Cube lifecycle, `.cube` identity, or version relationship.
- **Required behavior:** hide unpack whenever its operand set contains a Cube. There is no Cube-equivalent operation because unpacking explicitly negates Cube identity. A command-level guard must also prevent keybindings or stale UI from reaching native unpack.

### S02 — Quick action offers `Publish Subgraph`

- **Evidence:** Live-observed in Nodes 2.0; source applies to both renderers. A later sustained-hover check exposed both Comfy's `Publish Subgraph` PrimeVue tooltip and SugarCubes' `Save Cube` browser tooltip at the same time.
- **Why:** `SaveToSubgraphLibrary.vue` treats one selected `instanceof SubgraphNode` as publishable and executes `Comfy.PublishSubgraph`. Its visible tooltip and accessible label are hard-coded to `commands.Comfy_PublishSubgraph.label`; they do not consume the command object's dynamic label. `ComfyCubeCommandAdapter` correctly reroutes execution and changes the command label, but `ComfyCubeSelectionSurfaceAdapter` separately sets `title="Save Cube"` and tries to find the PrimeVue tooltip through `aria-describedby`. PrimeVue 4's tooltip directive does not create that attribute, so the host tooltip remains `Publish Subgraph` while the added `title` creates the second `Save Cube` tooltip.
- **Effect:** Comfy writes a Subgraph Blueprint instead of saving the `.cube`; see S21 for the complete consequence.
- **Required behavior:** reuse this exact native UI point for the authoritative Cube save flow. A draft maps to `Save SugarCube`; a current saved Cube maps to `Save cube implementation`; historical, read-only, and stale cases delegate to existing Sugar save policy and feedback. The Comfy Subgraph store must never receive a Cube. Presentation must update the PrimeVue directive's tooltip source before hover, set the accessible label, avoid adding a native `title`, and restore the host state exactly outside Cube selection.
- **Stable integration decision:** keep the existing `Comfy.PublishSubgraph` command wrapper for execution and repurpose the existing native button in place. Isolate PrimeVue's host-owned tooltip fields (`$_ptooltipValue` and the current tooltip id) behind one small typed adapter, because Comfy exposes no transform hook for its built-in publish button. The official `getSelectionToolboxCommands` hook is additive only: it cannot suppress the built-in button, and Comfy 1.47.11's `ExtensionCommandButton` derives tooltip/ARIA text only from an i18n key with an empty fallback instead of the registered command label. Using that hook would either leave two buttons or produce an unlabeled replacement, so it is not a complete native solution in the current host.
- **Resolution:** `PrimeVueTooltipPresentationAdapter` now owns the narrow directive compatibility boundary. It captures and restores the exact host ARIA, `title`, and lazy-tooltip value; presents `Save Cube` through PrimeVue's own value; synchronizes an already-mounted tooltip only when its text differs; removes rather than replaces the browser-native `title`; and reports the unavailable-host-contract case once. `ComfyCubeSaveButtonPresenter` owns the reversible native icon composition and tooltip presentation, while `ComfyCubeSelectionSurfaceAdapter` remains responsible only for locating the semantic book-open action and applying Cube selection policy. The icon combines Comfy's shipped `icon-[lucide--box]` and `icon-[lucide--save]` masks; a theme-aware foreground plate behind the save glyph matches `--comfy-menu-bg` and physically occludes the cube's lower-right strokes.

### S03 — Quick action offers `Edit Subgraph Widgets`

- **Evidence:** Live-observed in Nodes 2.0; source applies to both renderers.
- **Why:** `SelectionToolbox.vue` renders `ConfigureSubgraph.vue` for every single structural Subgraph. The command opens the right-side panel's `subgraph` tab.
- **Effect:** The action presents Cube interface authoring as a generic Subgraph widget operation and provides a parallel, unbranded path to mutate promoted inputs and widgets.
- **Required behavior:** suppress this action for Cubes. Cube interfaces are authored through SugarCubes' own implementation and metadata flows; Comfy's generic Subgraph widget editor is not a Cube affordance. Ordinary Subgraphs retain the native action.

### S04 — More Options offers `Add Subgraph to Library`

- **Evidence:** Live-observed in Nodes 2.0; shared Vue menu applies to both renderers.
- **Why:** `useSelectionMenuOptions.ts` adds the item for exactly one selected structural Subgraph. Its action directly calls `useSubgraphOperations().addSubgraphToLibrary()`, which calls `subgraphStore.publishSubgraph()`.
- **Important detail:** this path does not execute `Comfy.PublishSubgraph`; overriding the command alone cannot fix it.
- **Required behavior:** replace it with the same dynamic Cube save action as S02. The quick button and More Options entry must delegate to one Sugar application owner.

### S05 — More Options and native right-click offer `Unpack Subgraph`

- **Evidence:** Live-observed in More Options; source-confirmed for native right-click in both renderers.
- **Why:** `litegraphService.ts` adds `Unpack Subgraph` to every `SubgraphNode`, while `useSelectionMenuOptions.ts` independently adds a Vue-owned item. `useMoreOptionsMenu.ts` merges both and label-deduplicates them.
- **Required behavior:** suppress the action from the final menu model for Cube operands and retain a command guard. Filtering only `getNodeMenuOptions()` leaves the Vue action; filtering only the Vue action leaves the native item.

### S06 — More Options and native right-click offer `Edit Subgraph Widgets`

- **Evidence:** Live-observed in More Options; source-confirmed for native right-click in both renderers.
- **Why:** the same dual-source construction as S05.
- **Required behavior:** suppress the generic interface action for Cubes at both the menu and command boundaries. Do not provide a Cube replacement action. Ordinary Subgraphs retain the native action.

### S07 — A single Cube offers `Convert to Subgraph`

- **Evidence:** Live-observed in Nodes 2.0 More Options; source-confirmed for native right-click in both renderers.
- **Why:** the Vue selection menu correctly omits its own convert action for one structural Subgraph, but `LGraphCanvas.getNodeMenuOptions()` begins with `Convert to Subgraph` for any node. The native item survives the More Options merge.
- **Native effect:** `LGraph.convertToSubgraph()` clones the selected Cube wrapper, including Sugar properties, into a newly created ordinary outer Subgraph, removes the root Cube, and reconnects the graph.
- **Sugar effect:** `CubeNodeCatalog` deliberately indexes only `rootGraph._nodes`. It drops the removed root Cube and ignores the marker-bearing nested clone. The result is an unmanaged nested Cube wrapper inside an ordinary Subgraph, with ambiguous save, dirty, placement, and lifecycle ownership.
- **Required behavior:** hide and guard convert for any selection containing a Cube.

### S08 — Mixed selections expose operations that consume Cubes

- **Evidence:** Source-confirmed.
- **Why:** with a Cube plus any other item selected, `hasSubgraphs` and `hasMultipleSelection` are true. The quick toolbox shows convert rather than unpack. The Vue More Options menu exposes convert and unpack. Native `getCanvasMenuOptions()` also adds `Convert to Subgraph` when more than one node is selected and the user opens the empty-canvas context menu.
- **Effect:** convert wraps the Cube into a new ordinary Subgraph. Unpack silently filters the mixed selection to `SubgraphNode` operands, so it can operate on only the Cube subset while the UI appears to target the whole selection.
- **Required behavior:** any selection operation whose effective operand set includes a Cube must be unavailable. Ordinary-only selections retain native behavior.

### S09 — Native context-menu title identifies a Cube as `Subgraph node`

- **Evidence:** Source-confirmed for Nodes 1.0 and Nodes 2.0 right-click paths.
- **Why:** `SubgraphNode.displayType` returns `Subgraph node`. `LGraphCanvas.processContextMenu()` uses `node.displayType ?? node.type` as the menu title. Sugar's renderer-specific hosts do not override this product label.
- **Required behavior:** the final menu title for a Cube is `SugarCube` or the Cube's product type/name. Do not change `SubgraphNode.displayType` globally and do not fake structural identity; adapt only Cube instances at the presentation seam.

### S10 — Global commands and keybindings bypass visible-menu fixes

- **Evidence:** Source-confirmed.
- **Why:** core commands register publish, convert, unpack, edit widgets, exit, description, and search aliases independently of menus. `Ctrl+Shift+E` is bound to `Comfy.Graph.ConvertToSubgraph`. Escape is bound to `Comfy.Graph.ExitSubgraph`. The keybinding editor enumerates every registered command, so all of these generic commands are discoverable and user-bindable.
- **Effect:** hiding a button does not prevent a keyboard invocation. `Ctrl+Shift+E` can create the unsupported nested-Cube state from S07/S08.
- **Required behavior:** Cube-aware guards and routing must exist at the action boundary as well as the presentation boundary. Labels that remain valid actions must be dynamic in Cube context (`Exit Cube`, Cube save). Structural, Blueprint, and generic interface actions must reject Cubes even when invoked programmatically.

### S11 — SugarCubes offers `Convert Selected Subgraph to SugarCube` on a Cube

- **Evidence:** Source-confirmed in SugarCubes.
- **Why:** `CubeAuthoringHostCommands.getCanvasMenuItems()` checks only selection count. For exactly one selected item it always advertises the conversion. `ComfyCubeAuthoringAdapter.readSelectedSubgraphNode()` later rejects an existing Cube with `The selected subgraph is already a SugarCube.`
- **Required behavior:** use one shared availability/execution predicate: exactly one native, non-Cube Subgraph. This label is correct when the target really is an ordinary Subgraph; the defect is advertising it for a Cube or an arbitrary single item.

### S12 — Right-side panel presents generic Subgraph configuration

- **Evidence:** Source-confirmed; the live toolbox reaches this panel.
- **Why:** `RightSidePanel.vue` computes `isSingleSubgraphNode` with `instanceof SubgraphNode`, exposes a `data-testid="subgraph-editor-toggle"` button with aria label `Edit subgraph`, and renders `SubgraphEditor`/`TabSubgraphInputs`.
- **Effect:** the visible language and accessibility tree treat a Cube as a generic Subgraph. The panel is nevertheless operating on useful Cube interface data: promoted public controls.
- **Required behavior:** hide the Subgraph editor toggle at a Cube root. SugarCubes does not expose Comfy's generic widget-promotion panel as a Cube interface editor. The toggle and native mechanics remain unchanged in ordinary Subgraph context.

### S13 — `Node Info` is offered but has no coherent Cube destination

- **Evidence:** Live-observed in both toolbox and More Options; behavior source-confirmed.
- **Why:** every workflow subgraph gets a generated node definition, so `useSelectionState.canOpenNodeInfo` is true. `openNodeInfo()` opens the `info` tab. `RightSidePanel.vue` intentionally omits the Info tab for a single `SubgraphNode`, and its watcher resets invalid tabs to parameters.
- **Effect:** the Cube receives an inert or surprising generic node action. It may also expose host-generated Subgraph description text rather than Cube metadata.
- **Required behavior:** replace Node Info with Cube details backed by the existing Cube identity/metadata model, or hide it until that destination exists. Do not route Cube identity through the host's generated subgraph node definition.

### S14 — Generated node definition describes a Cube as `Subgraph node for ...`

- **Evidence:** Source-confirmed.
- **Why:** `subgraphService.ts::createNodeDef()` creates every workflow subgraph definition with category `subgraph`, module `nodes`, and fallback description `Subgraph node for ${name}`.
- **Visibility:** workflow-local definitions are normally hidden from the Nodes Library, but the definition remains available to Node Info and tooltip consumers. The custom Cube face may mask some native tooltip paths, so this is source-confirmed rather than live-observed.
- **Required behavior:** Cube-facing info and tooltip surfaces use Cube metadata. The internal category/module can remain structural because Comfy uses them to filter workflow-local definitions.

### S15 — Generic Rename couples Cube instance title to native definition name

- **Evidence:** Source-confirmed.
- **Why:** More Options includes `Rename`. `TitleEditor.vue` and Nodes 2.0 `useNodeEventHandlers.handleNodeTitleUpdate()` set `node.title`, then also set `node.subgraph.name` for anything reporting `isSubgraphNode()`.
- **Semantic conflict:** Sugar presentation explicitly distinguishes `instanceTitle` from `definitionTitle` in `CubeIdentityPresentation.ts`. A graph-local Cube instance alias is not the Cube artifact's definition name. Generic rename mutates the shared native definition while leaving `default_alias`, Cube metadata, and the `.cube` artifact unchanged. Other wrappers that reference the definition can inherit inconsistent naming.
- **Related inconsistency:** editing the right-side panel title changes only `node.title`, so Comfy itself currently has two different rename semantics for the same Cube.
- **Required behavior:** outer-node Rename changes only the graph-local Cube instance title. Definition/default-alias changes belong to Cube metadata authoring and Cube save. No generic rename path may silently mutate `subgraph.name` for a Cube.

### S16 — Active Cube breadcrumb uses native Subgraph naming and rename behavior

- **Evidence:** Source-confirmed.
- **Why:** `SubgraphBreadcrumb.vue` builds labels from native `subgraph.name`. `SubgraphBreadcrumbItem.vue` lets the active nested item run `useWorkflowActionsMenu()` and its rename function directly changes `workflowStore.activeSubgraph.name` and updates wrapper titles.
- **Effect:** the breadcrumb may show a structural `Cube: ...` name, and Rename bypasses Cube metadata and save policy. This can recontextualize all wrappers referencing the same definition without updating the `.cube` artifact.
- **Required behavior:** detect the active Cube root using a shared Cube editor-context resolver. Render the Cube definition title from Cube metadata. Replace Rename with `Edit Cube metadata` (focusing the existing metadata editor/name control) or remove it; do not mutate the native name directly. Nested ordinary Subgraphs inside a Cube keep native breadcrumb semantics.

### S17 — `Exit Subgraph` is correct mechanics with wrong product language

- **Evidence:** Source-confirmed.
- **Why:** the command is global and the breadcrumb back action executes it. It moves to the prior navigation stack graph or root.
- **Required behavior:** retain the native navigation. Present `Exit Cube` or `Return to workflow` when the active editor root is a Cube. When the current nested item is an ordinary Subgraph inside a Cube, `Exit Subgraph` remains correct for that level.

### S18 — `Set Subgraph Description` and `Set Subgraph Search Aliases` mutate Blueprint metadata inside a Cube

- **Evidence:** Source-confirmed.
- **Why:** both commands act whenever `canvas.subgraph` exists. Description writes `subgraph.extra.BlueprintDescription`; aliases write `subgraph.extra.BlueprintSearchAliases`; both capture workflow change state. They do not inspect Sugar markers.
- **Effect:** a Cube editor can accumulate Comfy Blueprint-only metadata alongside authoritative Sugar metadata. Description can diverge from `sugarcubes_cube.description`; search aliases have no Cube domain contract or sync semantics.
- **Required behavior:** in a Cube root, map description editing to Cube metadata and the Cube save flow. Suppress search aliases until SugarCubes defines an equivalent domain field and persistence contract. Ordinary nested Subgraphs retain the commands.

### S19 — Missing-node messages say `in subgraph` for Cube internals

- **Evidence:** Source-confirmed.
- **Why:** after `beforeConfigureGraph`, `scripts/app.ts` scans all serialized subgraph definitions and emits `g.inSubgraph` with the definition display name. It does not inspect `definition.extra.sugarcubes_kind`.
- **Effect:** missing custom nodes inside a Cube are reported as being in a Subgraph.
- **Required behavior:** render `in Cube '{name}'` when the enclosing definition carries Cube identity. The current extension hook receives no per-error transformation seam, so the clean fix belongs in Comfy's container-label resolution or a new extension hook.

### S20 — Active Cube breadcrumb offers `Clear Workflow`

- **Evidence:** Source-confirmed.
- **Why:** `useWorkflowActionsMenu.ts` always adds `clear-workflow`, including for non-root breadcrumb items. `Comfy.ClearWorkflow` uses browser-native `confirm('Clear workflow?')`, calls `app.clean()`, then removes every non-I/O node from the active subgraph because `Subgraph.clear()` is not supported.
- **Effect:** while editing a Cube, a generic workflow action can erase the implementation. It is misnamed, bypasses Sugar's modal system, and does not frame the result as an unsaved Cube change.
- **Required behavior:** replace with `Clear Cube implementation`, use Sugar's confirmation modal, preserve native Cube I/O boundaries, mark the Cube dirty, and leave saving explicit. Ordinary workflow and ordinary Subgraph behavior remain host-owned.

### S21 — Publishing creates a second library path that can rehydrate as a Cube

- **Evidence:** Source-confirmed end to end.
- **Publish path:** `subgraphStore.publishSubgraph()` requires one selected `instanceof SubgraphNode`, calls `canvas._serializeItems([subgraphNode])`, builds a minimal workflow, prompts with `Save Subgraph to Library` / `Subgraph name`, writes `subgraphs/<name>.json`, and registers `SubgraphBlueprint.<name>` under `Subgraph Blueprints/User`.
- **Marker retention:** wrapper `properties.sugarcubes_kind` and `properties.sugarcubes_cube` survive normal node serialization. The definition's Sugar `extra` fields are moved to workflow `extra` when the Blueprint saves, then copied back to definition `extra` when it loads.
- **Placement path:** `litegraphService.addNodeOnGraph()` handles a Blueprint by deserializing the Blueprint's wrapper node and definitions with `_deserializeItems()`. The placed wrapper therefore still has Sugar properties and a Sugar-marked definition. Sugar's root lifecycle can recognize and catalog it as a Cube, assigning a fresh instance ID only if necessary.
- **Product consequence:** a Comfy Nodes Library item can produce a Cube-like managed instance without going through the Sugar library, `.cube` validation/materialization, version choice, provenance resolution, or sync path. The Blueprint file also remains a separate copy that can drift from the real Cube.
- **Required behavior:** prevent Cubes from entering `publishSubgraph()` at both UI and action boundaries. Existing accidental Blueprint files require a migration/cleanup decision during implementation, but new ones must be impossible.

### S22 — `Save Workflow`/Ctrl+S inside a Cube is an adjacent save-context collision

- **Evidence:** Source-confirmed; this is adjacent to, rather than caused by, a visible Subgraph label.
- **Why:** `Comfy.SaveWorkflow` always saves the active parent workflow. It does not inspect the active Cube editor context. Sugar's Cube editor has its own authoritative save action in `CubeEditorMetadataHud`/`CubeEditorSaveService`.
- **Effect:** a user editing a Cube can invoke the familiar save command and save only the containing workflow, leaving the `.cube` implementation unsaved.
- **Required behavior:** in an active Cube root, route the primary save command/keybinding to `CubeEditorSaveService`. Parent-workflow saving remains explicit outside Cube context. This should share the same context resolver and save controller as S02/S04 rather than becoming another special case.

### S23 — SugarCubes' draft-created toast calls the Cube a `native subgraph`

- **Evidence:** Source-confirmed in SugarCubes.
- **Why:** `CubeCreationService.#startCreation()` reports `Cube draft created`, then instructs the user: `Wire its native subgraph, then choose Save Cube from the Cube actions menu.`
- **Effect:** Sugar's own first-run guidance exposes the implementation abstraction at the exact moment it should establish Cube authoring language.
- **Required behavior:** say `Edit the Cube implementation, then choose Save Cube from the Cube actions menu.` The implementation can remain a native Subgraph without making that the product noun.

## Correct behavior and structural uses to preserve

### C01 — A Cube remains a real native Subgraph

`SubgraphNode.isSubgraphNode()` must remain true, and Cube nodes must remain `instanceof SubgraphNode`. Execution, prompt flattening, error traversal, boundary resolution, promoted widgets, preview behavior, price aggregation, missing-model traversal, serialization, and renderer integration depend on this.

### C02 — Entering the definition is the intended editing mechanism

`SubgraphNode.onTitleButtonClick()` calls `canvas.openSubgraph()`. `CubeEditorNavigationPresenter.open()` deliberately uses the same native API and preserves root view/selection plus nested navigation. This is foundational behavior.

### C03 — Nodes 2.0 already adapts `Enter subgraph` to `Edit Cube`

`ComfyVueCubeEditorFooter.ts` finds `[data-testid="subgraph-enter-button"]`, replaces label/aria/title with `Edit Cube`, prepares Cube editor context, and preserves native navigation. This is the best current precedent: retain mechanics, adapt product identity at the presentation boundary.

### C04 — Nodes 1.0 already routes Cube-specific edit interactions

`ComfyLiteGraphCubeNodeHost` replaces the generic face while retaining the native `enter_subgraph` icon geometry. `ComfyLiteGraphCubeNodeInteraction` intercepts the Cube edit action and Cube-header double-click and routes them to the Cube editor. It does not generally replace native right-click menus, which is why S05-S09 still apply.

### C05 — Sugar save owners already exist

- `CubeChromeOverlay` supplies draft `Save SugarCube`, saved `Save cube implementation`, and defaults actions.
- `OverlayManager` routes implementation saves to `CubeSaveService.saveImplementation()` and retains historical restrictions.
- `CubeSaveService` owns source selection, stable IDs, workflow serialization, read-only/stale choices, backend save, reconciliation, feedback, and dirty state.
- `CubeEditorSaveService` owns the native-editor save use case, including first-save metadata and existing-Cube metadata authoring.

Adapted host surfaces must delegate to these owners; they must not reproduce version or write policy.

### C06 — Workflow-local subgraph definitions are hidden from the normal Nodes Library

`subgraphService.ts` registers a node definition for every workflow definition, but `nodeDefStore.ts` filters category `subgraph` plus module `nodes`. A normal Cube does not independently appear in the Nodes Library. Only the wrongful Blueprint path in S21 creates a library leak. `Hide Subgraph Nodes` is therefore a structural filter, not a Cube mismatch.

### C07 — Cube editor boundary affordances are already product-neutral

For empty drafts, `labelEmptyCubeBoundaryAffordances()` names transient slots `Add Input` and `Add Output`. Authored slots have their own names. Comfy's internal fallback `Subgraph Output` context title is not normally reached for these slots. The native boundary objects remain valid implementation details.

### C08 — Ordinary nested Subgraphs inside a Cube remain ordinary Subgraphs

Active Cube context must not globally disable Subgraph tools. If the selected operand is an ordinary nested Subgraph, native edit, unpack, convert, description, and Blueprint behavior remain available. Policy decisions must distinguish the Cube root/wrapper from ordinary Subgraphs nested within its implementation.

### C09 — Generic node-instance actions remain valid for Cubes

Delete, Copy, Duplicate/Clone, Pin, Bypass, Color, Shape, Minimize, and connection operations are generic node-instance behavior rather than Subgraph product behavior. `ComfyCubeNodeLifecycleAdapter` reconciles copied root Cubes and assigns a fresh `instance_id` when a copied identity collides. These actions should remain available unless a separate Cube-domain rule applies; the affordance fix must not indiscriminately replace the entire node menu.

## Required action mapping

| Context                          | Host affordance                            | Cube behavior                                                           |
| -------------------------------- | ------------------------------------------ | ----------------------------------------------------------------------- |
| One ordinary Subgraph            | All native Subgraph actions                | Unchanged                                                               |
| One draft Cube                   | Publish / Add to Library                   | `Save SugarCube` through first-save authoring                           |
| One current saved Cube           | Publish / Add to Library                   | `Save cube implementation`                                              |
| Historical/read-only/stale Cube  | Publish / Add to Library                   | Delegate to existing Cube policy, including fork/rejection/feedback     |
| Any selection containing a Cube  | Convert / Unpack                           | Hidden and action-guarded; no Cube equivalent                           |
| One Cube                         | Edit Subgraph Widgets / right panel toggle | Hidden and command-guarded; no Cube replacement action                  |
| One Cube                         | Node Info                                  | Cube details, or hidden until a Cube details destination exists         |
| One Cube                         | Rename                                     | Rename graph-local instance only; never mutate Cube definition metadata |
| Active Cube root                 | Breadcrumb title                           | Cube definition title from Sugar metadata                               |
| Active Cube root                 | Breadcrumb Rename                          | Focus `Edit Cube metadata`; no native name mutation                     |
| Active Cube root                 | Exit Subgraph                              | `Exit Cube` / `Return to workflow`, same native navigation              |
| Active Cube root                 | Set Description                            | Cube metadata description and Cube save flow                            |
| Active Cube root                 | Set Search Aliases                         | Suppressed until a Cube domain field exists                             |
| Active Cube root                 | Clear Workflow                             | `Clear Cube implementation` with Sugar confirmation and dirty state     |
| Active Cube root                 | Save Workflow / Ctrl+S                     | Save Cube through `CubeEditorSaveService`                               |
| Ordinary Subgraph nested in Cube | Native Subgraph actions                    | Unchanged for that selected/navigated ordinary Subgraph                 |

## Ownership model for the fix

### 1. Extract one shared Cube editor-context resolver

`CubeEditorNavigationPresenter` already contains the correct concept: locate the active graph within one root Cube definition hierarchy and retain the visited path through nested native Subgraphs. Its `findEditorContext()` logic is currently private.

Extract a focused, typed `CubeEditorContextResolver` and characterize existing navigation behavior before the refactor. Navigation presentation, commands, breadcrumbs, right-side panel adaptation, and error labels must call this owner rather than duplicate graph traversal.

The resolver returns at least:

- active root Cube node, if any;
- graph path from Cube definition root to current graph;
- whether the current graph is the Cube root or an ordinary nested Subgraph;
- selected Cube operands and selected ordinary Subgraph operands.

### 2. Add one pure Cube affordance policy

Create a presentation/application-boundary policy that consumes selection plus editor context and returns stable action decisions. It must not call the DOM, Comfy stores, filesystem, browser APIs, or save services.

The decision model includes:

- stable action ID;
- visibility/enabled state;
- Cube-specific label and aria text;
- target kind (`draft`, `current`, `historical/read-only`, `ordinary-subgraph`, `mixed-with-cube`, `cube-editor-root`, `nested-subgraph`);
- application intent (`save-cube`, `edit-cube`, `edit-cube-metadata`, `clear-cube`, `exit-cube`, or `blocked`). Generic Subgraph interface configuration resolves to `blocked` for Cubes.

### 3. Add one Cube host-affordance controller

The controller translates policy intents into existing application owners:

- Cube save intents delegate to `CubeEditorSaveService` or `CubeSaveService` as appropriate;
- edit intent delegates to `CubeEditorNavigationPresenter`;
- generic Subgraph interface configuration is blocked for Cubes while ordinary Subgraphs retain Comfy's native promotion mechanics;
- metadata intent focuses the existing Cube metadata HUD;
- clear uses Sugar modal/dialog services, native non-I/O removal, and dirty refresh;
- blocked structural actions produce Cube-specific non-blocking feedback only when invoked through a stale command or programmatic path.

It must never call `subgraphStore.publishSubgraph()` for a Cube and must not duplicate save/version/read-only rules.

### 4. Use focused host adapters per surface

Avoid one monolithic UI patch. Use cohesive adapters for:

- selection toolbox and More Options;
- native LiteGraph node/canvas context menus;
- command/keybinding routing;
- right-side panel and Node Info;
- breadcrumb/editor workflow chrome;
- missing-node container labels.

Both renderers call the same policy/controller. Renderer-specific adapters own only rendering and event interception.

## Comfy integration seam analysis

The current `ComfyExtension` surface is additive:

- `getSelectionToolboxCommands()` adds commands after hard-coded core buttons;
- `getCanvasMenuItems()` adds canvas items;
- `getNodeMenuItems()` adds node items.

There is no supported hook to suppress, replace, or relabel core toolbox actions or Vue selection-menu entries. `useContextMenuTranslation.ts` appends extension node items after native items. More Options also adds Vue items directly, and those actions often bypass command IDs.

Registering a duplicate core command ID is technically possible because `commandStore.registerCommand()` warns and overwrites. It can provide a safety wrapper and dynamic keybinding label, but it is not a complete presentation fix:

- quick button labels come from fixed localization keys;
- More Options actions call composables directly;
- native and Vue items are deduplicated by visible label rather than stable action ID;
- a global override must retain ordinary Subgraph behavior.

### Preferred durable Comfy seam

Contribute a small host change that centralizes structural actions as stable descriptors before rendering:

1. Give every convert/unpack/publish/interface action a stable action or command ID.
2. Make selection toolbox, More Options, native context conversion, and keybindings execute the same command-backed descriptors.
3. Deduplicate by stable ID, not localized label.
4. Add an extension transformation hook over the final action list with selection and active-graph context.
5. Allow an extension to hide, replace, relabel, and reroute a descriptor without changing the target node's structural prototype.
6. Add an extension container-label hook for breadcrumbs, right-panel accessibility text, and missing-node hints.

SugarCubes then supplies policy decisions; Comfy remains unaware of Sugar-specific fields.

### Compatibility path if Comfy cannot expose the seam immediately

A local compatibility layer can be built, but it is intentionally second choice:

- wrap core commands to guard Cubes and preserve captured native behavior for ordinary Subgraphs;
- use `getSelectionToolboxCommands()` only for genuinely additive Cube actions whose command IDs have a host-recognized accessible label; it cannot replace Comfy's built-in publish button in 1.47.11;
- repurpose the existing publish button in place for Cube selections, adapting its command execution, accessible label, and PrimeVue tooltip source while restoring all host state for ordinary Subgraphs;
- wrap final native node/canvas menu production for Cube instances;
- adapt the Vue toolbox/More Options DOM and capture clicks, following the lifecycle discipline already used by `ComfyVueCubeEditorFooter`;
- add action-boundary guards so DOM timing or localization changes cannot re-enable destructive behavior;
- test host-version compatibility explicitly.

Do not solve this by returning false from `isSubgraphNode()`, changing `instanceof`, globally replacing `SubgraphNode.displayType`, or stripping Sugar markers. Those approaches damage execution/serialization or lose Cube identity.

## Complete English Subgraph-string classification

The exact installed frontend's English strings and hard-coded labels were searched and classified:

| String/surface                                                                                | Classification for Cubes                                                                                                  |
| --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `Enter subgraph`                                                                              | Already adapted to `Edit Cube` in Nodes 2.0; Nodes 1.0 uses Cube edit interaction                                         |
| `in subgraph '{name}'`                                                                        | Adapt missing-node hint to Cube; S19                                                                                      |
| `Convert Selection to Subgraph` / `Convert to Subgraph`                                       | Hide/guard for Cube operands; S07/S08/S10                                                                                 |
| `Unpack the selected Subgraph` / `Unpack Subgraph`                                            | Hide/guard for Cube operands; S01/S05/S08/S10                                                                             |
| `Publish Subgraph`                                                                            | Replace with Cube save; S02/S10/S21                                                                                       |
| `Add Subgraph to Library`                                                                     | Replace with Cube save; S04/S21                                                                                           |
| `Edit Subgraph Widgets` / `Edit subgraph`                                                     | Suppress for Cubes; preserve for ordinary Subgraphs; S03/S06/S12                                                          |
| `Exit Subgraph`                                                                               | Dynamic `Exit Cube` at Cube root; native wording for nested ordinary Subgraph; S17                                        |
| `Set Subgraph Description`                                                                    | Map to Cube metadata; S18                                                                                                 |
| `Set Subgraph Search Aliases`                                                                 | Suppress in Cube root; S18                                                                                                |
| `Save Subgraph to Library`, `Subgraph name`, publish-success Blueprint text                   | Must be unreachable for Cube; S21                                                                                         |
| `Subgraph Blueprints` / `Subgraph Blueprints/User`                                            | Correct for real Blueprints; a Cube reaches it only through S21                                                           |
| `Subgraph node` context title                                                                 | Adapt to Cube; S09                                                                                                        |
| `Subgraph node for ...` node-def description                                                  | Avoid in Cube details/tooltips; S13/S14                                                                                   |
| `Hide Subgraph Nodes`                                                                         | Correct structural node-library filter; C06                                                                               |
| deduplicate-subgraph-ID setting                                                               | Correct structural behavior; no Cube adaptation                                                                           |
| Blueprint overwrite setting/dialogs                                                           | Correct for real Blueprints; must be unreachable for Cubes                                                                |
| widget promotion labels `Shown on node`, `Hidden / nested parameters`, `Promote Widget`       | Native ordinary-Subgraph mechanics; not exposed as Cube affordances                                                       |
| internal Subgraph input/output classes and execution errors                                   | Structural implementation details; retain                                                                                 |
| fallback `Subgraph Output` slot-menu title                                                    | Normally unreachable because Cube slots are named; retain unless a live Cube path exposes it                              |
| Sugar draft toast `Wire its native subgraph`                                                  | Reword as editing the Cube implementation; S23                                                                            |
| Sugar import/validation references to `implementation.subgraphs` or embedded Subgraph entries | Correct technical schema terminology: Cubes contain serialized native definitions; do not relabel them as the Cube itself |

## Implementation sequence

This sequence contains no unresolved technical questions. Product mappings are defined above.

1. **Characterize current boundaries.** Add tests for the existing Cube context traversal, renderer footer behavior, save delegation, root-only catalog, command registration, and ordinary Subgraph behavior.
2. **Extract context ownership.** Introduce the shared `CubeEditorContextResolver`; update navigation presenter call sites; remove duplicate/private traversal logic.
3. **Implement pure action policy.** Cover draft/current/historical, single/mixed, active Cube root, and nested ordinary Subgraph cases.
4. **Implement application controller.** Delegate to existing save/editor/dialog/dirty owners; add action-boundary guards before native convert/unpack/publish.
5. **Fix Sugar's own authoring predicate.** Show `Convert Selected Subgraph to SugarCube` only for one ordinary native Subgraph.
6. **Fix Sugar's own authoring language.** Reword the draft-created guidance to describe editing the Cube implementation.
7. **Land the Comfy action-transformation seam.** Centralize stable action descriptors and command-backed execution. If host contribution timing prevents this, land the version-scoped compatibility adapters with the same policy/controller.
8. **Adapt root selection surfaces.** Toolbox, More Options, native right-click title/menu, mixed-selection canvas menu, Node Info, and Rename.
9. **Adapt Cube editor chrome.** Right-side panel, breadcrumb title/metadata action, Exit label, Clear Cube, description, save keybinding, and nested-Subgraph distinctions.
10. **Adapt missing-node container language.** Use the serialized definition's Cube marker through the host label hook.
11. **Audit accidental Blueprints.** Detect Blueprint documents whose wrapper/definition carries Sugar markers; provide a deliberate one-time cleanup or migration path without deleting user data automatically.
12. **Run focused tests, both renderer tests, Chrome manual matrix, then `npm run check`.**

## Verification specification

### Policy and command tests

- Single ordinary Subgraph preserves all native actions.
- Single draft Cube maps publish/library to first-save authoring.
- Single current Cube maps publish/library to implementation save.
- Historical/read-only/stale Cube delegates to existing policy and never calls Comfy publish.
- Any selection containing a Cube rejects convert and unpack before native graph mutation.
- A selection containing only ordinary Subgraphs retains native convert/unpack.
- Active Cube root gets Cube labels and save/clear/description routing.
- Ordinary nested Subgraph inside a Cube retains Subgraph actions.
- Direct command invocation and assigned keybindings cannot bypass guards.

### Surface tests

- Nodes 1.0 and Nodes 2.0 selection toolbox show no generic Subgraph actions for a Cube.
- Both renderers' More Options and direct right-click menus contain one correct Cube action per intent and no hidden duplicate source.
- Native context-menu title is Cube-oriented.
- Mixed-selection empty-canvas menu does not offer convert when a Cube is included.
- Right-side panel aria/title uses Cube language and exposes no generic Subgraph interface editor.
- Node Info is replaced or absent.
- Outer Rename changes only the instance title.
- Draft-created guidance says Cube implementation rather than native Subgraph.
- Breadcrumb Rename does not mutate `subgraph.name` outside Cube metadata/save ownership.
- Escape/back retains correct navigation with Cube language at the Cube root.
- Clear Cube uses Sugar modal services and preserves I/O boundaries.
- Dynamic Cube names containing markup-like text render literally, satisfying DOM safety policy.

### Persistence and lifecycle tests

- No Cube path calls `subgraphStore.publishSubgraph()` or writes `subgraphs/<name>.json`.
- Cube save actions update `.cube` version/sync state through existing services.
- Draft first save, current update, historical/fork, and failure paths are covered.
- Convert/unpack guards leave `CubeNodeCatalog`, root graph, definition registry, links, and dirty state unchanged.
- A deliberately constructed legacy Sugar-marked Blueprint is detected for migration rather than silently treated as a normal library Cube.
- Ordinary Subgraph Blueprint publish/place remains unchanged.

### Error and editor-context tests

- Missing nodes in a Cube say `in Cube` and use Cube identity.
- Missing nodes in an ordinary Subgraph still say `in subgraph`.
- Cube description writes Sugar metadata; Blueprint fields are not added.
- Cube root suppresses Blueprint search aliases; nested ordinary Subgraph retains them.
- Ctrl+S in Cube root calls Cube editor save; outside Cube context it calls workflow save.

### Manual browser matrix

Completed against the running ComfyUI frontend 1.47.11 at `http://127.0.0.1:8198` with an isolated workflow:

- Nodes 2.0 renders a managed wrapper as `SugarCube`, exposes `Edit Cube` and one Cube-routed `Save Cube` selection action, and hides Node Info, configure widgets, convert, unpack, and Blueprint-library actions from the selection toolbox and More Options menu. A sustained hover mounted exactly one PrimeVue tooltip containing `Save Cube`; the action has no browser-native `title`.
- Nodes 1.0 uses `SugarCube` as the native LiteGraph context-menu title, exposes the same single `Save Cube` selection action, and omits generic Subgraph and interface actions. A sustained hover again mounted exactly one `Save Cube` PrimeVue tooltip and no browser-native `title`. Entry was exercised through the actual canvas-header double-click path.
- Direct root `convertToSubgraph()` and `unpackSubgraph()` calls return `null` for a Cube and leave the wrapper in place. Mixed Cube/ordinary-node conversion is also rejected without graph mutation.
- An ordinary node inside a Cube can still become an ordinary `Subgraph node`; its nested editor restores `Save`, `Clear Workflow`, and normal Subgraph language, and native unpack succeeds.
- An ordinary root Subgraph retains `Edit Subgraph Widgets`, `Unpack Subgraph`, and `Convert to Subgraph`; native unpack succeeds and the Cube remains present.
- The Cube root breadcrumb uses Sugar metadata, the back affordance is `Exit Cube`, the metadata HUD is present, and the Workflow actions dropdown shows `Save Cube` and `Clear Cube implementation` in both renderers.
- Clear opens the SugarCubes confirmation modal, emits no browser-native dialog, preserves the implementation when cancelled, and retains Cube boundary ownership. Ctrl+S is consumed by the Cube editor save route.
- A renderer round-trip from Nodes 2.0 to Nodes 1.0 and back to Nodes 2.0 retained Cube identity, editor chrome, and metadata HUD without runtime errors.
- A draft exposes `Save Cube` and opens the `Save SugarCube` first-save modal. A pack-owned read-only Cube retains the same save entry and delegates to the existing save policy. No native dialogs were emitted.
- A live attempted Sugar-marked write to `subgraphs/__sugarcubes_validation__.json` was rejected synchronously at `api.storeUserData`; no Blueprint file was written.
- Saved, draft, and pack Cube definitions expose Sugar descriptions through native node-definition metadata rather than Comfy's generated `Subgraph node for ...` description.
- After Firefox exposed a self-triggering tooltip reconciliation loop, both Firefox renderers were exercised with sustained hover. Nodes 1.0 and Nodes 2.0 each remained responsive at approximately 142 frames per second and mounted exactly one `Save Cube` tooltip. Chrome and Firefox also confirmed the native box/save composition, the foreground disk layer, and a badge plate whose computed background exactly matches the native selection toolbar. Both browsers were restored to Nodes 2.0.

## Invariants for implementation review

- `isCubeNode()` refines structural Subgraph identity; it never replaces it.
- Ordinary Subgraphs are behaviorally unchanged.
- Native execution, traversal, serialization, boundary promotion, and editor navigation remain intact.
- No Cube reaches Comfy Blueprint persistence.
- No native convert/unpack operation receives a Cube operand.
- One policy owns affordance decisions; one controller owns intent routing.
- Existing save/version/read-only services remain authoritative.
- Active Cube context is resolved once and shared.
- Renderer adapters contain presentation wiring only.
- All dynamic Cube text uses safe DOM APIs.
- No browser-native dialogs are introduced; adapted clear/save feedback uses Sugar UI.
- Accidental existing Blueprint data is never deleted without an explicit, recoverable migration decision.

## Implementation ledger

This section is the authoritative status for the cleanup. It is updated in place as work lands; the mismatch catalog above remains the behavioral specification rather than a chronological log.

| Area                                         | Mismatches                 | Status                  | Authoritative owner / verification                                                                                                                                                                                                                                                                                                             |
| -------------------------------------------- | -------------------------- | ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Shared editor and selection context          | S08, S10, S16-S22          | Implemented             | `CubeEditorContextResolver` owns root Cube, nested ordinary Subgraph, and selection classification; navigation and all affordance adapters share it.                                                                                                                                                                                           |
| Pure affordance decisions                    | S01-S22                    | Implemented             | `CubeAffordancePolicy` is DOM-free and maps stable action IDs to visibility, labels, target kinds, and intents.                                                                                                                                                                                                                                |
| Intent routing and safety guards             | S01-S08, S10, S18, S20-S22 | Implemented             | `CubeHostAffordanceController`, `CubeStructuralOperationGuard`, and in-place command-object routing delegate existing save/HUD/dialog owners and reject Cube structural mutation.                                                                                                                                                              |
| Nodes 2.0 selection toolbox and More Options | S01-S08, S11, S13          | Complete; live verified | Stable test IDs and semantic icon classes hide destructive/generic actions and restore host DOM outside Cube context. Execution routes through Cube save. Focused presentation owners adapt PrimeVue tooltip state and compose Comfy's native box/save masks with the disk occluding the cube. Chrome and Firefox checks mounted exactly one `Save Cube` tooltip with no `title` in Nodes 2.0 and Nodes 1.0. |
| Nodes 1.0 native menus                       | S05-S09, S11, S13, S15     | Complete; live verified | Instance-scoped final-menu filters preserve ordinary Subgraphs; the official extension node-menu item adds only Save Cube; `CubeInstanceRenameGuard` preserves definition identity in both native title editors. Chrome confirmed the `SugarCube` menu title, absent interface action, guarded direct command, and canvas-driven editor entry. |
| Cube editor chrome                           | S12, S16-S20, S22          | Complete; live verified | Stable breadcrumb/right-panel and semantic Reka-menu anchors provide Cube labels, metadata/clear routing, native exit mechanics, Cube Ctrl+S, and exact restoration in nested ordinary Subgraphs. Both renderers show `Save Cube` and `Clear Cube implementation`.                                                                             |
| Language and metadata                        | S14, S18, S19, S23         | Implemented             | One definition identity writer owns native description/markers; preconfigure supplies descriptions before Comfy node-def registration; mounted tooltip and missing-node adapters cover remaining renderer surfaces.                                                                                                                            |
| Blueprint exclusion and legacy detection     | S21                        | Implemented             | UI/action guards plus the final `storeUserData` namespace boundary prevent new Cube Blueprints; legacy marked Blueprint placements are reported once and remain untouched ordinary data.                                                                                                                                                       |
| Automated verification                       | All                        | Complete                | The full `npm run check` passes strict TypeScript, strict mypy, formatting, linting, standards audits, generated-build verification, 180 suites, and 983 tests. S02 coverage models PrimeVue's actual lazy target property, idempotent mounted-tooltip reconciliation, absent-contract diagnostics, exact host restoration, native icon composition, foreground occlusion, and the absence of a native `title`; it no longer fabricates `aria-describedby`. |
| Chrome and Firefox Nodes 1.0 / Nodes 2.0 matrix | All                      | Complete for S02        | The corrected toolbox was observed with real Cubes through renderer round-trips. Each browser and renderer exposed the native box/save composition with the foreground plate matching the toolbar; each mounted exactly one `Save Cube` tooltip with no native `title`. Firefox sustained hover remained responsive at approximately 142 frames per second. Nodes 2.0 was restored in both browsers. The broader picker and pointer-resize matrix is tracked separately below. |

### Live-only findings incorporated during implementation

These findings changed the implementation itself and are reflected in the owners above:

- Comfy does not guarantee `app.canvas` during extension-module evaluation. `CubeAffordanceHostLifecycle` now captures the current document/canvas at `beforeConfigureGraph`, creates the integration lazily, and reattaches it with each graph-bound runtime. This avoids relying on frontend bootstrap timing.
- Vue's node-title editor commits after the input event handler returns. `CubeInstanceRenameGuard` restores the Cube definition name on the next window task, after the host commit, while retaining the graph-local instance title.
- The Cube editor HUD position observer receives mutations caused by its own CSS custom-property writes. `CubeEditorHudPositioner` now writes only changed values, preventing a self-sustaining MutationObserver microtask loop.
- The top-level Workflow actions dropdown is a separate portalled Reka menu from the Subgraph breadcrumb menu. `ComfyCubeWorkflowActionsMenuAdapter` owns that surface, identifies it by semantic menu roles and the stable save/download/trash/delete icon set, relabels only primary Save and Clear at the Cube root, and leaves execution routing with `ComfyCubeCommandAdapter`.
- PrimeVue's tooltip directive stores the text used to create each lazy tooltip on the target element as `$_ptooltipValue`; it does not expose the tooltip through `aria-describedby`. `PrimeVueTooltipPresentationAdapter` owns this host-specific field, visible-tooltip synchronization, exact restoration, and compatibility diagnostics so the selection adapter remains concerned with Cube policy rather than third-party directive internals.
- Writing the same `textContent` to an already-correct tooltip still replaces its text node. Because the selection adapter observes `document.body`, that write caused another refresh and an unbounded MutationObserver cycle that slowed and eventually crashed Firefox. Tooltip reconciliation now compares the mounted value before writing; a mutation-observer regression test proves a correct tooltip receives zero mutations.
- Comfy 1.47.11 ships both `icon-[lucide--box]` and `icon-[lucide--save]`. `ComfyCubeSaveButtonPresenter` uses those native masks, preserves and restores the original book-open icon exactly, and owns a theme-aware disk plate that sits after the cube in paint order and masks the underlying cube strokes.

### Stable-host integration decision

The installed Comfy frontend provides additive extension hooks but no suppression/transform hook. The compatibility implementation therefore uses a layered, fail-safe contract:

1. lifecycle capture occurs at Comfy's `beforeConfigureGraph` extension hook, after the live canvas exists, rather than at module evaluation time;
2. stable Comfy command IDs are wrapped in place at the command store boundary so existing keybindings and command-palette references cannot bypass Cube policy;
3. graph structural methods are guarded by marked Cube identity before native mutation;
4. extension-provided node-menu items supply native-looking Cube replacements through Comfy's supported additive menu hook;
5. Vue-only hard-coded affordances use existing `data-testid` anchors, ARIA roles, Reka state, and semantic icon classes, never Cube names or translated labels;
6. native LiteGraph menu filtering is isolated behind one instance-scoped adapter and applies only when a marked Cube is an operand;
7. the stable `api.storeUserData` namespace boundary rejects Sugar-marked `subgraphs/` writes even if stale UI or a future command path bypasses presentation guards;
8. every compatibility mismatch fails closed for Cube-destructive actions and logs an actionable compatibility diagnostic while leaving ordinary Subgraphs untouched.

This is the most stable shippable seam available to a custom-node extension in ComfyUI frontend 1.47.11. The preferred upstream action-transformation hook remains the longer-term replacement for the compatibility adapters; the Sugar policy and controller are intentionally independent of either host mechanism.

## Node picker and search integration research

### Product contract

SugarCubes should participate in Comfy's ordinary node-discovery experience without becoming Subgraph Blueprints:

- The node picker/search lists one item per available Cube under `SugarCubes/<target model>`.
- In the current default search UI, `SugarCubes` is the first ordinary category after `Subgraph Blueprints`; expanding it exposes one child for each declared primary target model.
- Model grouping uses `targetModel`, not the broader `supportedModels` compatibility list. A legacy artifact with no primary target model is grouped under `Unspecified` instead of disappearing or being guessed into a model family.
- A result's secondary detail identifies its Cube pack (`Base-Cubes`, for example), while the category tree remains model-oriented. Packs do not introduce another navigation level.
- Cube name, description, source, tags, model metadata, and canonical identity contribute to discovery without borrowing Blueprint identity.
- Selecting an item places a new, independently editable Cube instance through the existing Sugar import and placement workflow.
- The picker and its node preview advertise only the Cube's explicit input and output boundary contract.
- Surface controls, promoted widgets, internal node ports, marker placement, and whatever is currently visible on the Cube face do not become advertised node inputs or outputs.
- Ordinary backend nodes and Subgraph Blueprints continue to register, search, preview, drag, place, and auto-connect exactly as Comfy implements them.
- Nodes 1.0 and Nodes 2.0 receive the same catalog and boundary contract. Renderer choice changes the placed node's presentation, not what the Cube means in search.

This is a discovery and creation integration. It does not make `.cube` files Comfy Blueprint files, place them in `subgraphs/`, give them `SubgraphBlueprint.*` type names, or opt them into Blueprint publish/edit behavior.

### Running-system evidence

The running service at `http://127.0.0.1:8198` serves the frontend bundle identified during this investigation as Comfy frontend 1.47.11. The live Sugar catalog contains 25 Cubes. `/sugarcubes/list` returns identity, source, search metadata, and layout counts but no canonical boundary projection.

Live `/sugarcubes/picker_catalog` and `POST /sugarcubes/load` results, checked against the installed validated `.cube` artifacts, establish why picker I/O must not use face state. The prepared placement payload deliberately omits the `surface` document; the catalog service projects boundaries from the validated Cube artifact rather than trying to recover them from placement or rendered state:

- `Artificial-Sweetener/Base-Cubes/Anima/Automask Detailer.cube` has one canonical `IMAGE` input labelled `IMAGE Input` and one canonical `IMAGE` output labelled `IMAGE Output`, despite also having surface metadata and six internal nodes.
- `Artificial-Sweetener/Base-Cubes/Anima/Prompt by Region.cube` initially exposed the save defect as version `2.2.0` with zero canonical outputs despite a connected native `IMAGE` output sourced from `VAEDecode`. The repaired version `3.0.0` now stores `output.image -> ["vae_decode", 0]`; the live picker advertises that one output and no inputs. Its 20 face controls still do not become advertised boundary inputs.
- Across a survey of the live catalog, boundary counts and surface-control counts routinely differ. They are separate product concepts, not alternate representations of one interface.

The live picker catalog currently advertises 25 Cubes with zero structural projection errors. All 25 corresponding load requests returned a payload whose Cube identity and version matched the advertised descriptor. The initial survey proved transport and identity consistency but also exposed that an already-corrupted empty canonical boundary map could pass projection. After the save-integrity repair and normal re-save, a focused live catalog/load recheck proves that Prompt by Region now advertises and reloads its one `IMAGE` output from `vae_decode:0`.

The 25 current `.cube` files total approximately 0.82 MiB, with the largest approximately 72.4 KiB. Preloading every prepared payload is technically affordable for this particular catalog, but catalog size is unbounded and packs are syncable. That observation supports a compatibility fallback; it is not a sound long-term loading contract.

Chrome automation was connected through the requested extension. Literal `127.0.0.1` and `localhost` navigation remains blocked by that extension, so live UI verification uses `http://localtest.me:8198`, which resolves to the same authorized loopback service. The default picker was directly observed in Nodes 2.0 and Nodes 1.0. In both renderers, `SugarCubes` expanded to `Anima`, `Any`, `SDXL`, `SeedVR2`, `Flux`, `Flux2 Klein`, and `Unspecified`; the `Anima` child returned only Anima Cubes; `Prompt by Region` displayed `Base-Cubes` before its description; and ordinary KSampler results retained their native Comfy source/category detail with no Sugar marker. Nodes 2.0 was restored after the matrix.

### How Comfy builds the picker catalog

Comfy has one important shared discovery pipeline:

1. `app.registerNodes()` fetches backend object definitions.
2. It awaits every extension's `addCustomNodeDefs(defs, app)` hook.
3. It registers each resulting definition as a LiteGraph type.
4. It invokes `registerCustomNodes(app)` after standard registration.
5. `updateVueAppNodeDefs()` builds the Vue-facing definition collection, invokes the synchronous `beforeRegisterVueAppNodeDefs(defs, app)` hook, and commits the definitions to `nodeDefStore`.
6. `nodeDefStore.visibleNodeDefs` feeds both node search implementations and both Node Library designs. `nodeSearchService` is rebuilt reactively from that collection.

`nodeDefStore.nodeDefs` prepends `subgraphStore.subgraphBlueprints` to the ordinary node-definition values. This is why Blueprints are naturally first in the default search category tree.

The default V2 picker (`Comfy.NodeSearchBoxImpl = default`) derives its left category tree solely from `nodeDef.category`. `NodeSearchCategorySidebar` asks `nodeOrganizationService` to group the current visible definitions by category. `buildTree` creates each category when it first encounters it and preserves that first-occurrence order. It has no public category priority API.

The V1 search implementation consumes the same searchable definitions but is list-oriented and has no category sidebar. The LiteGraph legacy picker is a third, older implementation based directly on registered LiteGraph types. A correct integration must remain searchable in all three, while the requirement to appear visually beneath `Subgraph Blueprints` applies to the default picker that actually renders a category tree.

Search text is currently indexed from `name`, `display_name`, and `search_aliases`. Category and source are structured filters, not free-text search keys. Cube metadata that should be searchable must therefore be projected deliberately into `search_aliases` rather than merely attached as unused fields.

The empty default search view is frequency-ranked. Dynamically supplied Cube definitions have no existing entries in Comfy's backend frequency map, so a Cube should not be expected in `Most Relevant` until it has a frequency record or the user searches/selects the SugarCubes category. This does not affect explicit search or category browsing.

### Category and source identity

Each contributed Cube definition now uses:

- `category: 'SugarCubes/<targetModel>'`, with `SugarCubes/Unspecified` as the explicit missing-model fallback
- `python_module: 'custom_nodes.<pack name>'`
- a Sugar-owned `sugarcubes_pack_name` presentation field resolved from the same pack identity owner used by the Cube browser
- a collision-resistant type name derived from canonical Cube identity, such as `SugarCubes.Cube.<sha256(cube_id)>`
- `display_name` equal to the Cube's default alias
- `description` equal to its Cube description
- `search_aliases` containing useful, deduplicated values from canonical Cube ID, source-relative name/path, tags, author/pack, target model, and supported models

The type key must not contain mutable alias, version, file path, or pack checkout path. The canonical Cube ID is the identity; a hash keeps the host type safe and bounded. Version remains placement metadata so syncing a new version updates what future selections load without changing bookmarks or search identity.

The category must not be `Subgraph Blueprints/SugarCubes`. Comfy treats every category beginning with `Subgraph Blueprints` as a Blueprint in the V2 filter bar, and its Blueprint placement branch is selected by the `SubgraphBlueprint.*` type prefix. Borrowing either namespace would recreate the product confusion this project already removed.

The pack-specific `python_module` preserves the useful native source badge and still classifies the synthetic definition as a custom-node contribution. Consequently, the redesigned V2 Node Library classifies these definitions in its hard-coded `Extensions` section. This means the following two visual requirements are different:

- **Default node picker/search:** `SugarCubes` can be a top-level category directly after `Subgraph Blueprints`.
- **Redesigned Node Library sidebar:** `SugarCubes` is a category inside `Extensions` unless Comfy gains an extensible section/source registry.

Pretending SugarCubes is Core or Blueprint to force the Library section upward is not acceptable. If a first-class Library section becomes a product requirement, Comfy needs a section contribution seam; DOM reparenting is not a stable substitute. The hard-coded section order itself changed between frontend 1.45.20 and 1.47.11, while the data-driven picker category component did not.

Comfy currently uses the same `nodeDef.category` value for two responsibilities: slash-delimited category-tree structure and the secondary category text shown before a result description. There is no supported definition field or presentation transform that can make navigation say `SugarCubes / Anima` while the result detail says `Base-Cubes`. SugarCubes therefore isolates the unavoidable current-version presentation compatibility in `ComfyCubePickerResultPresenter` instead of contaminating catalog or category semantics:

- it observes only the mounted `#results-list`, never the document body;
- it identifies result roots through Comfy's upstream-owned `data-testid="result-item"` anchor;
- it matches a Cube against the live typed registry by exact display name and description and fails closed if that identity is ambiguous;
- it replaces only the category text node, records the claimed Cube type on that node, and performs no write when the desired pack label is already present;
- pointer, keyboard, and input activity schedule a bounded refresh, while one scoped observer covers Vue result replacement without the document-wide mutation loop that previously caused Firefox slowdown.

The durable host improvement is a separate result-detail/provenance presentation field or node-definition presentation hook. If Comfy changes the result contract, this focused adapter can fail closed without affecting definition registration, search, placement, or ordinary nodes.

### Target-model title presentation

The target-model pill is a presentation of existing Cube identity, not new persisted meaning. `target_model` is already validated Cube metadata, `default_alias` already carries the canonical model/name route, and `instance_alias` already owns the graph-local user name. `CubeModelTitlePresentation` exposes a pill only when the authoritative target model exactly matches the leading alias route segment. It never infers a model from arbitrary text before `/`, and it leaves a custom instance alias as plain text. The centered definition title always uses the canonical default alias; the instance-title lane uses the model pill only while its resolved title equals that default alias.

The three rendering surfaces have different stable owners:

- Picker results remain a current-Comfy compatibility surface. The scoped `ComfyCubePickerResultPresenter` resolves one typed Cube definition, then delegates pack detail and model-title presentation to `ComfyCubePickerPackPresenter` and `ComfyCubePickerModelTitlePresenter`. Model presentation finds one unique visible title leaf and fails closed if Comfy's result structure is ambiguous. It does not change `display_name`, search identity, category grouping, or placement.
- Nodes 2.0 Cube headers are Sugar-owned DOM. `CubeFaceHeaderView` now owns the identity and action lanes extracted from `CubeSurfaceView`, keeping model-title DOM behavior out of masonry, preview, measurement, and resize responsibilities.
- Nodes 1.0 Cube headers are Sugar-owned Canvas composition. `CubeCanvasChromeRenderer` remains the orchestrator and delegates model-pill measurement and drawing to `CubeModelPillCanvasRenderer`. LiteGraph's live title-text color and the actual Cube header color provide the fill and punch-out colors.

One host-neutral title-segmentation model feeds both DOM and Canvas renderers. `CubeModelPillDomRenderer` constructs pill, name, and suffix nodes with `createElement`, `textContent`, and `replaceChildren`; its compact shared stylesheet inherits Comfy text color and uses the current Comfy menu/content surface for inverse lettering. `CubeModelPillCanvasRenderer` implements the same proportions without importing browser DOM APIs. The picker receives the projected authoritative `sugarcubes_target_model`; no string inference or `.cube` schema change is involved.

Automated coverage proves semantic matching, nonmatching routes, default versus custom aliases, literal rendering of markup-like metadata, idempotent picker reconciliation, compact DOM geometry, Canvas measurement/alignment, and both Cube header integrations. The final repository gate passes with 185 TypeScript suites and 999 TypeScript tests plus 429 Python tests, strict typing, standards audits, formatting, linting, and generated-build verification.

Chrome live verification at `http://localtest.me:8198` confirms the native `SugarCubes` model hierarchy, Base-Cubes result provenance, pill titles in the picker, centered definition titles, default instance titles, and plain custom aliases. The custom-alias check was reverted with Comfy's undo history. Nodes 2.0 DOM and Nodes 1.0 Canvas rendering were observed in Obsidian Dark and Light. The Light pass exposed and then verified the repair of a contrast defect: DOM punch-out text now uses the actual Comfy title/menu surface instead of the Cube body token. The browser was restored to Obsidian Dark and Nodes 2.0 with both original default aliases intact.

### Authoritative advertised interface

The authoritative interface is the prepared import's `boundaries` projection:

- Backend `project_cube_boundaries()` derives typed inputs and outputs from `implementation.inputs`, `implementation.outputs`, embedded definitions/subgraphs, and validated boundary marker identity.
- `CubeBoundaryProjection` carries inputs with `{id, name, label, type, targets}` and outputs with `{id, name, label, type, source}`.
- Frontend `readCubeBoundaryDefinitions()` validates that dynamic payload before it crosses into graph construction.
- `CubePayloadTopology` and `ComfyCubeGraphBuilder` already use those canonical definitions to create the real native subgraph boundaries.

The picker projection must call the same domain owner. It must not independently infer an interface from `surface.controls`, `node.widgets`, preview exposures, current face DOM, internal implementation node schemas, or marker geometry.

For Comfy's current V1 contribution schema, map canonical boundaries as follows:

```text
input.required[boundary.name] = [boundary.type, {
  forceInput: true,
  display_name: boundary.label
}]

output[index] = boundary.type
output_name[index] = boundary.label
output_is_list[index] = false
```

`forceInput: true` is mandatory even for scalar types. Comfy has built-in widget constructors for `INT`, `FLOAT`, `BOOLEAN`, `STRING`, and other types. Without `forceInput`, its shared node builder suppresses the input socket and creates a widget, falsely advertising a configurable face value instead of a required Cube boundary. The actual LiteGraph node builder honors `forceInput`, so placed nodes receive a socket and no widget. Input order follows the canonical boundary array; output indices follow the canonical output array.

The current V1-to-V2 migration copies input options, including `forceInput` and `display_name`, into V2 input specs. It maps `output_name` to V2 output names. The shared `litegraphService.addInputSocket()` and `addOutputs()` methods then build the correct placed-node slots in both renderers.

Comfy's preview implementations are not fully consistent with its node builder. Exact 1.47.11 source tracing confirms the mismatch is in shared host code, after Sugar's V1 definition has already been migrated truthfully:

- The default picker uses `NodePreviewCard`, which lists every non-hidden definition input and output and therefore advertises the right boundary set. It currently displays an input's record key rather than `display_name`.
- The V1 search and several Node Library hover paths use `NodePreview` / `LGraphNodePreview`. They divide sockets from widgets with `widgetStore.inputIsWidget(input)`, which checks only whether a widget constructor exists for the type and ignores `forceInput`. A canonical scalar boundary can therefore be shown as a widget in those previews even though the placed node correctly has a socket.
- Those legacy/Vue previews also use the input's internal name rather than its `display_name` label.

Specifically, `transformInputSpecV1ToV2()` spreads Sugar's options into the V2 input spec, preserving both fields. `widgetStore.inputIsWidget()` then reduces classification to `widgets.value.has(type)` and never consults `forceInput`. `NodePreviewCard.vue` maps the input record key to its displayed name, while `NodePreview.vue` and `LGraphNodePreview.vue` likewise use the internal `name`. The stable extension hooks can contribute definitions and creation behavior, but they do not expose either shared resolver. Correcting this from Sugar would require DOM manipulation, replacing a host store, or falsifying the input type/name; all three violate this design's stability and semantic requirements.

This is a host preview defect, not a reason to falsify the Cube node definition. The durable Comfy change should introduce one shared predicate equivalent to the actual construction rule—widget-capable **and not** `forceInput`—and one shared input-display-name resolver. `NodePreview`, `LGraphNodePreview`, and `NodePreviewCard` should use them. SugarCubes should not invent custom scalar type aliases, delete widget registrations, or rewrite boundary names to manipulate preview rendering.

### Missing Sugar catalog projection

The current `/sugarcubes/list` response is not sufficient to build truthful picker definitions because it omits boundaries. `summarize_cube_file()` already opens each Cube and extracts identity, metadata, tags, source, and layout counts, but adding Comfy node-schema details directly to that function would give a summary module a second, host-specific responsibility.

Add a focused backend owner, conceptually `CubePickerCatalogProjection`, with a typed output such as:

```text
CubePickerDescriptor {
  type_key
  cube_id
  version
  default_alias
  description
  search_aliases
  source_reference
  inputs: CubePickerInput[]
  outputs: CubePickerOutput[]
}
```

That owner reads the validated Cube document, delegates boundary meaning to the existing boundary projector, and emits host-neutral picker descriptors. A thin adapter then translates descriptors to Comfy V1 node definitions. Neither the backend domain projection nor the frontend registry should know how the Cube face renders.

A dedicated `GET /sugarcubes/picker_catalog` endpoint is preferable to making `/sugarcubes/list` accumulate another consumer contract. Its response should include a schema version and `catalogRevision`, allowing the frontend to reject unknown formats and reconcile only when identity/version/source state changes. The existing library already owns `catalog_revision()` and the settings flow already calls `invalidateDependentCatalogs()` after pack add, remove, toggle, and sync; the picker registry should become one of those dependents.

The endpoint should remain lightweight. It should not contain complete prepared placement payloads in the durable design.

### The placement gap in Comfy

Discovery and placement are different host concerns. Comfy exposes a usable discovery hook but no extension placement hook.

`NodeSearchBoxPopover`, legacy and redesigned Node Library clicks, Library drag-to-canvas, search drag/ghost placement, and other programmatic consumers converge on `litegraphService.addNodeOnGraph(nodeDef, options, addOptions)`. That service has only two creation policies:

1. `SubgraphBlueprint.*` definitions are deserialized through `subgraphStore` as Blueprints.
2. Everything else calls synchronous `LiteGraph.createNode(nodeDef.name)`, then adds the result to the active graph.

A Cube selection must instead perform an asynchronous Sugar load/revision operation and then call the existing `CubePreparedImportService` / `CubePlacementService`, which creates a fresh native subgraph definition and one configured native SubgraphNode instance. Simply injecting a node definition therefore makes a Cube searchable but not correctly placeable.

The distinction matters because every placed Cube currently receives its own native subgraph definition. Pre-registering one shared subgraph type per library Cube would make all instances share definition edits and native metadata. That conflicts with independent instance identity, dirty tracking, version history, and the intended ability to edit one Cube implementation without silently editing every other placement.

The Blueprint placement branch cannot be reused. It carries Blueprint storage, type identity, filters, editing semantics, and affordances that SugarCubes intentionally rejects.

### Stability assessment of available seams

| Seam                                                                             | Stability                    | Appropriate use                                                           |
| -------------------------------------------------------------------------------- | ---------------------------- | ------------------------------------------------------------------------- |
| Generic async node-definition provider plus centralized async placement provider | High, once accepted by Comfy | Durable catalog, creation, invalidation, and all-surface behavior         |
| Existing `addCustomNodeDefs`                                                     | Medium-high at startup       | Fetch and contribute typed Cube definitions; incomplete on reload         |
| Existing `beforeRegisterVueAppNodeDefs`                                          | Medium                       | Restore cached definitions after Vue refresh; compatibility ordering only |
| Existing `registerCustomNodes` / `LiteGraph.registerNodeType`                    | Medium                       | Reconcile legacy LiteGraph discovery types; incomplete on reload          |
| Narrow, chain-safe `LiteGraph.createNode` wrapper                                | Medium-low                   | Synchronous custom-node-only placement fallback with prepared cache       |
| Definition-array first-occurrence ordering                                       | Low-medium                   | Temporary placement directly after prepended Blueprints                   |
| Scoped result-detail presenter at `#results-list` / upstream result test anchor  | Low-medium                   | Current fallback for pack detail until Comfy separates it from category   |
| Broad picker DOM mutation, reparenting, or document-wide observation             | Low                          | Reject                                                                    |
| Blueprint namespace/store reuse                                                  | Semantically invalid         | Reject regardless of mechanical stability                                 |

The extension hooks, default category-sidebar grouping, and centralized generic/Blueprint branch were materially unchanged between the inspected frontend 1.45.20 and 1.47.11 sources. The redesigned Node Library's section organization changed during the same interval. The scalar-preview misclassification also exists in both inspected versions. This supports building on definition data and centralized creation while avoiding Library/search component structure, but it does not turn missing reload/placement contracts into APIs.

### Preferred durable Comfy seam

The clean solution is a small, generic Comfy extension contract at the two existing centralized ownership points. This is a host improvement, not Sugar-specific logic in Comfy.

1. **Node-definition providers.** Add an async provider contribution invoked during initial registration and every node-definition reload. A provider returns typed definitions plus stable provider/source identity. Comfy reconciles additions, updates, and removals and exposes a public invalidation method. This replaces the current mismatch where `addCustomNodeDefs` runs at startup but not during `reloadNodeDefs()`.
2. **Node-placement providers.** Add a provider registry inside `litegraphService.addNodeOnGraph`. A provider may claim a definition namespace and asynchronously return a placement outcome containing the real placed node. Generic and Blueprint creation remain built-in providers. Search, click, drag, ghost placement, link-release auto-connect, undo/history, and future consumers all await the same result instead of each surface learning about Cubes.
3. **Category ordering metadata.** Add a category contribution/priority transform, or allow definitions/providers to declare an ordering relation such as `after: 'Subgraph Blueprints'`. The organizer owns ordering. Extensions do not reorder host arrays or mutate picker DOM.
4. **Optional Library sections.** If SugarCubes must become a top-level redesigned Node Library section, add a source/section contribution registry with a category tree and priority. This is separate from picker category order.
5. **Shared preview semantics.** Make preview input classification use the same `forceInput`-aware socket/widget rule as node construction and display `display_name` when present. This corrects an existing Comfy inconsistency for every node definition, not just Cubes.

A suitable placement contract is conceptually:

```text
NodePlacementProvider {
  id
  canPlace(nodeDef): boolean
  place({ nodeDef, position, graph, addOptions, source }):
    Promise<{ handled: true, node } | { handled: false }>
}
```

Comfy should own graph selection, graph insertion, ghost/link follow-up, and history boundaries around the returned real node. The provider should own only construction semantics. Sugar's provider resolves the type key to canonical Cube identity, loads the current allowed revision, and delegates validation/preparation to existing import owners. The current `CubePlacementService` combines detached construction, graph insertion, catalog registration, and history. Implementation should first extract a focused detached `CubeConstructionService` (or equivalent factory use case), then let the existing placement orchestrator and the host provider call that owner from their respective insertion flows. Do not add a flag-heavy branch to `CubePlacementService`. Comfy never parses `.cube` fields.

This seam is more stable than a Sugar-only UI patch because it lives where Comfy already centralizes definitions and creation. Changes to search presentation, category components, Node Library tabs, or renderer implementation continue to consume the same contract.

### Best custom-node-only compatibility path

Until Comfy exposes an async placement provider, the least fragile shippable fallback is data integration plus one narrowly isolated creation adapter:

1. Use the official async `addCustomNodeDefs` hook to load the picker catalog and add Cube node definitions.
2. Use `beforeRegisterVueAppNodeDefs` to reinsert the cached Cube definitions on every Vue definition refresh and move only those definitions to the beginning of the ordinary-definition array. Because Comfy prepends Blueprints itself, the default category tree becomes `Subgraph Blueprints`, then `SugarCubes`.
3. Use `registerCustomNodes` and a focused registry to reconcile dynamic LiteGraph types. Remove stale type keys when packs disappear; do not leave compatibility aliases.
4. Preload validated prepared placement payloads before advertising the corresponding definitions. A Cube whose payload is unavailable is omitted or disabled with an actionable diagnostic rather than exposing a picker entry that cannot place.
5. Wrap the stable global `LiteGraph.createNode` boundary once, claim only the reserved `SugarCubes.Cube.*` namespace, synchronously construct a fresh native subgraph and detached configured node from the prepared cache, and return that real node to Comfy's existing `addNodeOnGraph` flow. Delegate every other type byte-for-byte to the captured host function.
6. Connect `invalidateDependentCatalogs()` to a picker-catalog reconciler. Refresh cache first, then definitions/types, then invoke the narrowest available Comfy definition refresh. Never expose a new definition before its placement cache is ready.
7. Keep the model hierarchy in `nodeDef.category` and isolate pack-detail presentation in one scoped, idempotent host adapter. Do not let the DOM compatibility boundary become a second owner of grouping, source identity, or placement.

Wrapping `LiteGraph.createNode` is intentionally a compatibility adapter, not domain code and not an endorsed Comfy API. It is preferable to intercepting buttons or picker DOM because all current creation surfaces already converge there, including the LiteGraph legacy picker. It also preserves Comfy's normal graph add, return value, ghost placement, and link auto-connect behavior because the wrapper returns the final native Cube node synchronously.

The adapter still has real costs:

- every advertised Cube payload must be resident before synchronous creation;
- a Comfy change that stops using `LiteGraph.createNode` for ordinary definitions would require adapter work;
- global wrappers must be chain-safe and idempotent because other extensions may also wrap the function;
- native subgraph creation has registry side effects, so partial construction must roll back its fresh definition or fail before mutation;
- Comfy still skips `addCustomNodeDefs` and `registerCustomNodes` during `reloadNodeDefs()`, so Sugar's compatibility adapter must explicitly register its refreshed types before asking Comfy to reload and must reinsert its cached definitions through `beforeRegisterVueAppNodeDefs`.

These constraints are why the async provider is the target architecture. The compatibility adapter belongs in one typed infrastructure module behind a host capability check. Sugar application/domain code must depend on a placement port, never on the wrapper.

### Rejected approaches

- **Put Cubes in `subgraphStore` or use `SubgraphBlueprint.*`.** This restores Blueprint persistence and behavior and makes the category/filter identity false.
- **Create one shared native subgraph definition per catalog Cube.** Placed Cubes would share implementation definition state.
- **Inject or reparent picker DOM.** Component structure, portals, test IDs, and section order change independently of the data contract and would miss nonvisual creation paths.
- **Patch `NodeSearchBoxPopover` only.** Node Library clicks, drag, bookmarks, link release, and legacy pickers bypass it.
- **Return a temporary proxy node and replace it asynchronously in `onAdded`.** Callers immediately consume the returned node for ghost placement, selection, and link auto-connect. Replacement breaks those semantics and creates history/undo ambiguity.
- **Use a custom constructor that returns a native SubgraphNode.** `LiteGraph.createNode` overwrites the returned object's `type`, applies options, and calls `onNodeCreated` after construction. Correcting that through one-shot lifecycle tricks depends on undocumented constructor-return behavior and risks creating subgraphs during type probing. The narrow `createNode` adapter is more explicit if the compatibility route is necessary.
- **Infer sockets from `surface.controls` or rendered widgets.** Face controls and public graph boundaries remain separate contracts. `Prompt by Region` has 20 face controls and one intended `IMAGE` boundary output; none of those controls are boundary inputs.
- **Lie about `python_module`, `api_node`, or Blueprint category to force section placement.** This corrupts source filters and future behavior.
- **Rely permanently on insertion order.** It is acceptable inside the isolated compatibility hook because the current picker has no priority API, but the durable design gives ordering to the organizer.

### Native interface save integrity

`Artificial-Sweetener/Base-Cubes/Anima/Prompt by Region.cube` was the first newly authored Cube saved through the native-subgraph path after retiring explicit authored `CubeOutput` marker nodes. Its version `2.2.0` artifact exposed a blocking save defect:

- The persisted artifact contains `implementation.outputs: {}`.
- The same artifact contains `implementation.nodes.vae_decode`, whose embedded `VAEDecode` definition declares output slot `0` as `IMAGE`.
- The persisted original execution id is `31667ded-c140-4a32-8d6f-73a65f112212:953141f7-f9a8-4ab5-8337-861a09c07ea2`, proving that the immediate native subgraph node id is the string UUID `953141f7-f9a8-4ab5-8337-861a09c07ea2`.
- The intended native Cube interface has one connected `image` output socket. The correct canonical binding is therefore `"output.image": ["vae_decode", 0]`.

The corruption occurs before the backend exporter sees the native definition:

1. `CubeSaveService` calls Comfy's `graphToPrompt()` and passes the workflow through `enrichWorkflowPayload()`.
2. `WorkflowPayloadBuilder` correctly prefers each live native subgraph's `asSerialisable()` result.
3. `SubgraphSerialization.normalizeLinkEntry()` then models `origin_id` and `target_id` as numbers and calls `coerceInteger(..., 0)` on both endpoints.
4. Comfy 1.47.11's authoritative `SerializedNodeId` type is `number | string`; its `SerialisableLLink` uses that type for both endpoints. UUID-backed Cube nodes are therefore normal host data, not a compatibility edge case.
5. A real output link such as `{origin_id: "9531...", origin_slot: 0, target_id: -20, target_slot: 0}` becomes `{origin_id: 0, ...}` while the serialized node retains its string UUID.
6. `project_native_cube_exports()` indexes definition nodes by their real string ids. It cannot match the corrupted `0` endpoint, so it creates no export-only output marker.
7. `analyze_cubes()` and `_build_outputs()` remain marker-centric. With no projected marker, `_build_outputs()` returns `{}` and `migrate_legacy_payload()` faithfully writes that empty map to `implementation.outputs`.

The pre-fix tests missed the defect at the layer boundary. `test_native_subgraph_export_adapter.py` used string node ids but called the backend adapter directly, bypassing frontend normalization. `ui_save_workflow.test.ts` exercised normalization with numeric node ids. No test sent one real current-format UUID subgraph through frontend normalization and backend projection while asserting the final nonempty canonical output map.

Both required downstream consumers were executed against the broken artifact before repair:

- `E:\devprojects\sugar-dsl` validates the document but returns `outputs: {}`. Its compiler can only resolve declared outputs and only generates runtime `SugarCubes.CubeOutput` nodes from `implementation.outputs`.
- `E:\devprojects\sugarsubstitute` also validates and materializes the document with `outputs: {}`. Its picker classification counts this map, so it necessarily classifies the Cube as having no output.
- With only the in-memory output map corrected to `{"output.image": ["vae_decode", 0]}`, both consumers accept the document. Sugar DSL generates a runtime `CubeOutput` linked to `PromptByRegion.vae_decode` slot `0`; SugarSubstitute materializes the same canonical mapping.

There is a second independent serialization defect in `_build_outputs()`: it writes only the source symbol and discards `Edge.source_slot`. A slot-0 output survives because the downstream shorthand string means slot `0`, but any Cube boundary connected to output slot `1+` is silently rebound to slot `0`. Canonical output values must preserve `[symbol, slot]` unless the serializer deliberately proves and tests an equivalent slot-0 shorthand.

Implemented cleanup and ownership:

1. `SubgraphConnectionSerialization` now owns Comfy connection primitive normalization. It preserves the authoritative `number | string` node-id union, accepts object and tuple link shapes, preserves numeric ordinary-subgraph ids, and throws on any malformed link instead of inventing node `0` or silently returning a smaller topology.
2. `native_subgraph_links.py` validates every serialized link at the backend adapter boundary. Projection no longer skips malformed topology.
3. `native_boundary_contract.py` compares current `linkIds` metadata with real body-node boundary links, validates boundary direction and public slot ranges, and verifies every serialized link against the referenced implementation node's exact input/output slot and endpoint-owned link id. It rejects nonexistent implementation slots, stale node endpoint metadata, missing body endpoints, and multiple output sources while retaining compatibility for older interface declarations that omit `linkIds`. Native Cube projection fails before export and persistence when either the public interface or implementation topology is inconsistent.
4. `boundary_bindings.py` is the authoritative canonical marker-to-binding owner extracted from the broad serializer. It serves both legacy marker graphs and export-only native marker projection, and output values always retain `[symbol, source_slot]`, including slot `0`.
5. Characterization coverage now includes object and tuple UUID endpoints, numeric ordinary-subgraph endpoints, fail-closed malformed frontend links, UUID-backed final native output export, slot-0 and nonzero-slot canonical outputs, missing declared links, corrupted input/output endpoints, nonexistent source and target implementation slots, stale source and target endpoint link ownership, malformed backend links, legacy definitions without `linkIds`, and genuinely blank native Cubes.
6. `Prompt by Region` was re-saved through `/sugarcubes/save_implementation`, then its two temporary intermediate save commits were collapsed into one clean local Base-Cubes commit, `569c5da` (`fix(anima): restore Prompt by Region image output`). Version `3.0.0` stores `"output.image": ["vae_decode", 0]`, all 20 original face controls, and all 18 portable authored values. The running SugarCubes `/load` route reloads an `IMAGE` boundary sourced from `vae_decode:0`, and `/picker_catalog` advertises zero inputs and one `IMAGE` output. Sugar DSL materializes `PromptByRegion.cube_output_image` linked to `PromptByRegion.vae_decode` slot `0`; SugarSubstitute validates the runtime contract and materializes the same output map. Both consumers also retain the authored width `960` and VAE choice `auto`.

This cleanup belongs in focused graph-id normalization, boundary projection, canonical binding, and validation owners. It must not be added as another special case in `CubeSaveService`, the UI entry point, or the already broad serializer. If the touched serializer/exporter blast area exposes mixed ownership, extract the boundary-binding responsibility and complete all callsite migration rather than layering a compatibility shim.

### Fresh Cube placement geometry

The live installed catalog exposed a second picker-placement defect after native integration: fresh Cube nodes inherited artifact geometry that described the authored implementation or a previously saved face, rather than receiving a predictable new-instance frame.

`CubeConstructionService.readPayloadGeometry()` previously selected `cube.metadata.surface_size`, then the managed authored group's `bounding` width and height, then `[900, 600]`. This mixed three different geometry meanings:

- `layout.groups[].bounding` is model-space implementation geometry. It encloses the internal graph used to build the native definition and is not a suitable Cube-face dimension.
- `metadata.surface_size` records artifact face geometry. It remains valid format data for editing and round-tripping, but does not own the size of every newly placed instance.
- A Comfy workflow's serialized native node `size` is instance geometry. Comfy restores it through the native node lifecycle and it must remain authoritative for reopened workflows and user resizing.

The 25 live installed Cube payloads proved the practical effect. Authored bounds ranged from approximately `204 × 26` to `1670 × 930`; representative detailer, inpaint, and upscale artifacts commonly exceeded 1300 pixels wide. Fresh placement therefore produced frames whose size depended on unrelated internal graph organization. `Anima/Prompt by Region` carries a deliberately authored `surface_size` of `1190 × 775`, while other artifacts without that field fell back to implementation bounds. This explains why otherwise related Cubes arrived at radically different visual scales.

Fresh size is now a separate presentation policy:

- `CubeInitialSizePolicy` is the lower-level construction port. `CubeConstructionService` asks it for fresh size without importing renderer or face modules.
- `CubeInitialSurfaceSize` is the presentation owner injected by `ComfyCubeRuntime`. It reserves two masonry columns, the canonical input gutter when inputs exist, and a useful preview whenever Cube face state makes the preview visible. Native output sockets remain an independent outer-node concern.
- Preview presentation and advertised output sockets are separate contracts. A Cube can correctly advertise zero canonical outputs while retaining a preview rail that says `No preview available` before execution. Neither fresh sizing nor either renderer may infer preview visibility from canonical boundary counts.
- Default Cubes start at `836 × 600` without an input gutter and `920 × 600` with one. Both sizes preserve two `240`-pixel masonry columns, their `12`-pixel gap, and a `320`-pixel preview in Nodes 1.0 and Nodes 2.0. The policy uses the larger renderer-specific frame requirement rather than assuming their content insets are identical.
- The installed `Anima/Prompt by Region` artifact persists a visible `513.684`-pixel preview and advertises one `IMAGE` output. Its fresh frame must provide two masonry columns plus that preview rail while still reserving the normal native output-port presentation. It is not a zero-output regression fixture.
- Persisted face preferences can influence fresh layout within bounded limits. Initial column width is capped at `360`, masonry gap at `32`, and preview width at `520`, preventing malformed or historical values from recreating extreme frames.
- Cubes without canonical inputs omit the input gutter. Cubes without canonical outputs omit output sockets and their gutter, not the face preview. A deliberately hidden preview remains hidden.
- The renderer's existing minimum-height policy may grow the initial `600`-pixel height when the visible face controls genuinely need more room. It does not inflate width.
- `constructBuilt()` continues to preserve explicit caller geometry. This safeguards selection-to-Cube authoring and retired-container migration. Existing workflow instances and later user resize operations do not use the fresh-placement policy.

Outer-frame resizing and divider movement are separate presentation commands. A mounted Cube first establishes its current frame width and saved preview width as the allocation baseline. Later ordinary width changes are assigned to the preview rail, so the default two-column masonry area retains its width. Moving the divider at a constant frame width explicitly changes the masonry/preview split and establishes the allocation the next frame resize preserves. Both renderers apply this policy through the focused `CubePreviewFrameResizePolicy`; Nodes 1.0 persists the adjusted preview width into native surface state and Nodes 2.0 commits it through the normal surface-state callback. Narrow stacked-preview mode neither clamps nor overwrites the saved side-rail allocation and resets the resize baseline before side-by-side layout resumes.

Output association leaders and native sockets follow transient resize geometry rather than waiting for the history transaction to commit. Nodes 1.0 emits a geometry-change event from each active resize move; the face host immediately recomputes its current layout and updates native slot positions before Comfy paints the next graph-link frame. Nodes 2.0 observes both the outer native node and the replaceable native body that owns the SVG leader, then reflows the face and remeasures boundary geometry on each reported size change. Pointer release remains the sole history-commit boundary, avoiding repeated undo entries.

Host notification ordering alone is not a sufficient paint-safety invariant. Comfy-owned resize paths can present a new boundary before an observer callback recalculates the SVG path, which previously allowed one stale endpoint to paint beyond the moved socket. The Nodes 2.0 leader SVG therefore owns a hard hidden-overflow viewport in both its element construction and structural stylesheet. Nodes 1.0 leaders already render inside the canvas renderer's rounded-frame clip. Transient geometry synchronization keeps the line accurate; renderer-owned clipping guarantees a stale intermediate path cannot escape the Cube frame.

This behavior needs no additional `.cube` field. The existing face preview width records the user's divider allocation, while the workflow node size records the instance frame. The stateful mounted-surface policy interprets changes between those two existing values without conflating artifact geometry, fresh-placement geometry, and per-instance geometry.

The policy intentionally ignores `metadata.surface_size` and authored group bounds only when creating a fresh placed instance. The `.cube` format does not need another size field: the artifact already has editor/face geometry, and Comfy workflow serialization already owns per-instance geometry. Adding a third persisted size would reproduce the ownership ambiguity that caused the defect.

Automated coverage separates these contracts and uses a genuinely outputless Cube rather than treating `Prompt by Region` as zero-output. Construction coverage proves fresh placement ignores extreme artifact bounds and prebuilt construction preserves explicit geometry; placement coverage proves surface state round-trips without dictating the new frame; focused policy coverage includes the corrected one-output Prompt-by-Region descriptor, hidden previews, renderer-specific insets, extreme-value bounds, frame growth and shrinkage, explicit divider reallocation, renderer constraints, and stacked-preview baseline resets; Nodes 1.0 and Nodes 2.0 layout coverage proves a preview rail remains alongside two masonry columns without inventing or suppressing canonical output sockets and that ordinary outer-frame growth leaves masonry width unchanged.

### Implementation ownership and status

The compatibility architecture is implemented through focused owners rather than feature code in the mixed-responsibility UI entry point:

1. **Complete — backend projection and truthful catalog.** `CubePickerCatalogService` owns the typed lightweight catalog response and `/sugarcubes/picker_catalog` is a thin route. The repaired artifact now projects one `IMAGE` output from `vae_decode:0`; face controls remain absent from advertised boundary inputs.
2. **Complete — descriptor projection.** `CubePickerDescriptor` strictly validates schema version, stable keys, identity, metadata arrays, and unique boundary IDs/names. `ComfyCubeNodeDefProjector` is the only descriptor-to-Comfy owner and emits the `SugarCubes/<targetModel>` category, explicit `Unspecified` fallback, pack provenance, scalar `forceInput`, canonical labels, outputs, and search aliases. `CubePackIdentity` is the shared pack resolver used by picker projection and Cube-browser grouping; neither consumer reinterprets current and legacy source shapes independently.
3. **Complete — atomic registry.** `CubePickerCatalogRegistry` owns revision state and preloads every prepared payload before advertising its definition. Per-Cube load failures omit only that Cube; catalog request/schema failures preserve the prior snapshot. Concurrent refreshes coalesce, a forced invalidation received during an active refresh queues exactly one forced follow-up, unchanged revisions do not reload payloads, removals replace the old snapshot, and callers receive isolated payload copies. Catalog parsing rejects duplicate type keys and duplicate Cube identities before they can compete for ownership.
4. **Complete — detached construction.** `CubeConstructionService` now owns graph assembly, fresh instance identity, Cube definition metadata, orchestration of fresh geometry policy, detached native node creation, and compensation when native node construction fails. `ComfyCubeNodeFactory` no longer inserts into a graph. `CubePlacementService` owns normal import insertion/history/catalog state; authoring and retired-container migration insert explicitly. `ComfyCubeGraphBuilder` compensates its registered definition if assembly throws.
5. **Complete — picker placement.** `CubePickerPlacementAdapter` clones/remaps the prepared cache, applies the active renderer geometry policy, registers nested definitions, and delegates to detached construction. It does not add to a graph or own history. Comfy's existing centralized creation flow receives the final native Cube node and continues to own active-graph selection, add options, ghost placement, link follow-up, and return values.
6. **Complete — current Comfy compatibility boundary.** `ComfyCubePickerCreationAdapter` is the only `LiteGraph.createNode` wrapper. It is idempotent, claims only `SugarCubes.Cube.*`, delegates ordinary calls with their exact arguments and receiver, applies the normal title/options contract to the final Cube, reports failures through Sugar feedback, and restores itself only when no later wrapper owns the seam.
7. **Complete — definition and invalidation lifecycle.** `ComfyCubePickerDefinitionAdapter` contributes through `addCustomNodeDefs`, reinserts/reorders cached definitions through `beforeRegisterVueAppNodeDefs`, explicitly registers additions/updates, unregisters removed LiteGraph types, and invokes Comfy's definition reload for current consumers. `CubeCatalogInvalidationCoordinator` is the single focused fan-out owner used after pack add, remove, toggle, sync, and version-affecting changes; it refreshes the native picker and legacy browser independently and logs either failure instead of silently swallowing picker errors.
8. **Complete — thin picker composition and nested lifecycle.** `ComfyCubePickerComposition` isolates dynamic Comfy capability validation and constructs the focused adapters. `CubePickerHostIntegration` presents the thin extension lifecycle. The picker additions to `frontend/comfyui/ui.ts` are composition plus delegated hooks; picker catalog, projection, reconciliation, placement, and error policy remain outside the entry point. The existing Cube lifecycle index now scans root and registered native subgraphs so picker-created nested Cubes receive normal identity and presentation behavior.
9. **Complete — automated safeguards.** The save-integrity regression matrix covers UUID link preservation, fail-closed malformed-link handling, exact implementation endpoint ownership, and exact canonical output slots. Focused strict TypeScript, strict mypy, frontend serialization, backend native projection, output binding, live backend catalog/load, and three-consumer artifact checks pass. The final `npm run check` passed with 185 TypeScript suites and 999 TypeScript tests plus 429 Python tests, including pack identity, model grouping, result-presentation idempotence, model-title semantics and rendering, ambiguity fail-closed behavior, standards audits, formatting, linting, strict typing, and generated-build verification.
10. **Partially complete — live picker and resize verification.** Chrome directly verifies the model hierarchy, category filtering, explicit text search, pack detail, and ordinary-node isolation in Nodes 2.0 and Nodes 1.0 through `http://localtest.me:8198`; Nodes 2.0 was restored. The selection-toolbar renderer round-trip is also verified through that origin. The broader creation and resize matrix remains separate: ordinary Subgraph Blueprints, pack reconciliation, repeat placement, undo/redo, picker edit/save, failure rollback, and pointer-driven frame-versus-divider allocation still require direct coverage.
11. **Known host preview boundary.** The extension emits the semantically correct definition. Current Comfy 1.47.11 legacy/Vue preview components still ignore `forceInput` during widget classification and ignore `display_name` for input labels. Sugar does not patch picker DOM or falsify input types/names. This can only be considered resolved when the shared Comfy preview predicate/resolver is corrected upstream or the live installed frontend proves it already contains that fix.

### Verification acceptance criteria

- `Subgraph Blueprints` and `SugarCubes` are distinct adjacent top-level categories in the default picker, and `SugarCubes` expands directly to primary target-model children without a pack nesting level.
- Each Cube result displays its resolved pack name before the description; missing primary target models appear under `Unspecified`.
- The Blueprint root filter excludes every SugarCube.
- The Extensions root filter includes SugarCubes because their source is truthful.
- Searching by alias, canonical ID, tag, author/pack, and model finds the expected Cube.
- A Cube with a scalar canonical input displays a socket, never a value widget, after placement and in every preview once the shared Comfy preview predicate is corrected.
- Input previews use the canonical boundary label while preserving the canonical boundary name as identity.
- `Prompt by Region` advertises one canonical `IMAGE` output sourced from `vae_decode` slot `0`; its 20 face controls do not become boundary inputs.
- Input/output names, labels, types, and order match the prepared boundary projection exactly.
- Click, drag, ghost/follow-cursor, link-release, keyboard selection, and legacy picker paths all return and operate on the final native Cube node.
- Repeated selection creates independent native subgraph definitions and independent Cube instance IDs.
- Pack sync updates future placement content without changing stable picker type identity; removal removes the definition and type.
- Catalog/load failure produces actionable Sugar logging/toast behavior and no malformed placeholder node or orphan definition.
- Ordinary nodes and ordinary Subgraph Blueprints remain behaviorally unchanged across Nodes 1.0 and Nodes 2.0.

## Root-only SugarCube placement

### Product invariant

A SugarCube wrapper is a root-workflow node. Its implementation remains a native Comfy Subgraph and may contain ordinary nodes and ordinary `SubgraphNode` instances, including nested ordinary Subgraphs. Opening that implementation for editing remains supported. A SugarCube wrapper itself must never be added to the implementation graph of a Cube or to any other non-root graph.

The invariant is based on two facts that must remain separate:

- SugarCube identity is the durable `sugarcubes_kind` marker on a real native Subgraph node.
- Placement scope is graph object identity: the target graph must be the runtime's validated root graph object.

Generic Subgraph identity, definition names, URLs, breadcrumbs, renderer DOM, and current picker category cannot decide placement validity. No `.cube` format field is required because placement scope is workflow context, not artifact data.

### Original ownership defects

The pre-change code deliberately supported nested SugarCube wrappers and therefore violated the new invariant:

1. `ComfyCubeNodeLifecycleAdapter` collects `_nodes` from both the root graph and every registered subgraph. It reidentifies nested SugarCubes and places them in the runtime catalog. The existing lifecycle test explicitly characterizes that behavior.
2. `CubePickerPlacementAdapter` registers definitions and constructs a detached Cube before Comfy's centralized creation flow adds the node to the active canvas graph. `ComfyCubePickerCreationAdapter` claims the picker type at `LiteGraph.createNode`, but neither adapter validates that `canvas.graph` is the root graph. Picker click, drag, ghost placement, link release, and other native picker routes therefore share the same missing precondition.
3. `CubePreparedImportService` checks that the root graph exists, then always calls the root-bound `CubePlacementService`. When the user is editing a nested graph this can place a Cube invisibly on the root instead of rejecting the command. Registration occurs before placement failure handling, so the scope check must precede registration.
4. `ComfyCubeAuthoringAdapter` mixes root insertion with active-graph selection conversion. Empty drafts are added to the root, while selection conversion uses `canvas.graph`; promotion accepts any cataloged Cube. Authoring availability and execution therefore need the same root precondition before conversion, definition creation, adoption, catalog mutation, or dialogs with mutable state.
5. Copy, paste, alt-drag clone, context-menu clone, workflow insertion, and host APIs do not use Sugar's placement service. Comfy 1.47.11 routes paste and clone through `LGraphCanvas._deserializeItems()`: it registers copied Subgraph definitions, creates nodes with `LiteGraph.createNode`, calls the active graph's `add`, and only then configures each node. A late canvas-event rejection would leave definitions, history state, or partially configured nodes behind.
6. Workflow hydration uses root `LGraph.configure()`, creates and configures every subgraph definition leaf-first, then creates and adds root nodes. Historical nested SugarCubes must remain loadable and serializable for recovery, so the defensive runtime guard must distinguish root workflow hydration from user mutations. After hydration, invalid nested wrappers must not enter the normal root Cube catalog or receive identity normalization.

### Stable host seams

Comfy's native `LGraph.add()` is the common graph-mutation boundary used by picker placement, paste, clone, programmatic addition, undo/redo restoration, and graph configuration. Its receiver is the authoritative target graph and exposes `rootGraph`, making an object-identity policy possible without UI inference. `LGraph.configure()` is the synchronous hydration boundary and provides the narrow context needed to preserve historical invalid data. These core graph methods are materially more stable than picker components, command IDs, context-menu DOM, or renderer-specific events.

The integration must be chain-preserving and scoped to the active Comfy root graph. It must reject a marked Cube before `LGraph.add()` mutates node id, graph membership, widgets, execution order, history, selection, or canvas state. It must allow adds performed during root workflow configuration, then inventory the hydrated graph and expose violations. Picker and Sugar-owned import/authoring paths still perform an earlier application-level preflight so no Cube definition is registered or constructed before rejection.

Clipboard deserialization requires an additional preflight because Comfy registers copied Subgraph definitions before adding the copied wrapper. The preflight must inspect the serialized node and referenced copied definition for durable SugarCube identity before `_deserializeItems()` mutates the definition registry. It must not reject ordinary Subgraphs, including ordinary Subgraphs whose implementations contain other ordinary Subgraphs.

### Required ownership model

1. A focused, host-neutral root-placement policy owns the single rule and its typed error.
2. A Comfy graph-scope adapter validates the root and current target graph objects and calls that policy.
3. A focused host mutation guard owns stable `LGraph.add`/configuration/clipboard integration, clean installation and disposal, chain preservation, and Sugar feedback. It does not own catalog behavior or persistence rules.
4. A focused graph inventory owner classifies root SugarCubes separately from invalid nested SugarCubes. `ComfyCubeNodeLifecycleAdapter` consumes only root results and remains responsible only for root identity reconciliation and catalog synchronization.
5. Persistence and execution preflight consume the same inventory result. They reject invalid nested state before network or backend work and explain that the user must remove the nested SugarCube wrapper. They preserve the workflow data in memory and on ordinary workflow serialization so the user can recover it.
6. Sugar-owned picker, import, and authoring use cases invoke the root precondition before any definition registration, construction, graph/history/catalog mutation, or UI state that assumes success.

### Proof obligations

- Root picker placement, root import, root empty/selection authoring, and root promotion continue to work.
- The same commands from a Cube implementation or ordinary nested Subgraph fail without new nodes, definitions, history entries, catalog entries, or selection residue.
- Paste, clone, duplicate, link-release creation, workflow insertion, public creation APIs, undo, and redo cannot introduce a new nested SugarCube.
- Loading historical invalid nesting preserves the wrapper and its definition for recovery, excludes it from the root catalog, and blocks Cube save/export and prompt execution with Sugar feedback.
- Ordinary Subgraph nodes can still be created, nested inside Cube implementations, copied, pasted, serialized, loaded, edited, and executed.
- Root Cube copy/paste still receives a fresh instance identity after configuration.

### Implemented ownership and behavior

The root-only rule now has one host-neutral owner, `CubeRootPlacementPolicy`. `ComfyCubeGraphScope` adapts Comfy's root and active graph objects to that policy without relying on URLs, renderer state, node titles, or picker presentation. `ComfyCubePlacementRuntime` composes the policy-facing adapters so `ComfyCubeRuntime` remains an orchestration façade rather than absorbing another host-integration responsibility.

`ComfyCubeHostMutationGuard` owns the stable defensive boundary. It installs a chain-preserving wrapper on the shared `LGraph.add()` prototype and scopes enforcement to the runtime's actual root graph. A marked Cube is rejected before native add side effects whenever the receiver is a descendant graph. The guard begins in hydration mode so existing invalid workflows can load for recovery, and `afterConfigureGraph` explicitly completes hydration before ordinary user mutations resume. It also preflights `LGraphCanvas._deserializeItems()` before Comfy registers copied definitions, closing paste, duplicate, clone, and copied-subgraph bypasses without changing ordinary Subgraph behavior.

`CubeGraphInventory` is the authoritative persisted-graph classifier. It returns root Cube wrappers separately from nested violations. Lifecycle reconciliation now consumes only the root set, so nested historical wrappers retain their serialized identities but never enter the root Cube catalog. `CubeSavePreflightService` and `ComfyPromptQueueBridge` consume the same inventory rule and reject Cube persistence or execution before their underlying workflows mutate state or contact the backend. Ordinary workflow serialization remains available for recovery.

Sugar-owned paths fail earlier than the defensive host guard. Picker placement validates the active graph before definition registration or construction; prepared import validates before runtime resolution, registration, or placement; and empty/selection authoring validates before conversion, native definition creation, node creation, graph insertion, or catalog mutation. Cube authoring commands are omitted entirely while the current canvas graph is nested. Promotion of an existing root Cube from its own editor remains valid because it does not place another wrapper.

No `.cube` format field was added. The durable Cube marker already answers what the node is, and the receiver graph's object identity answers where the operation is occurring.

### Verification

Automated regression coverage proves the policy and all mutation boundaries independently:

- exact root-object identity is required;
- nested inventory is non-mutating and excluded from lifecycle reconciliation;
- root `LGraph.add()` accepts Cubes, descendant `add()` rejects them before membership, and hydration preserves historical invalid data;
- clipboard preflight rejects a copied Cube and a Cube buried in a copied Subgraph before definition registration, while ordinary nested Subgraphs pass;
- nested picker, prepared import, empty authoring, and selection authoring reject before their first mutation;
- invalid historical nesting blocks Cube save and execution before the delegated operation;
- ordinary Subgraph conversion/nesting and root Cube copy identity behavior remain intact.

The full repository gate passes: strict TypeScript and mypy, formatting, linting, standards and DOM-safety audits, generated-web synchronization, 189 suites, and 1,016 tests.

Chrome live verification used isolated unsaved workflows at `http://localtest.me:8198` and exercised both renderers. In Nodes 2.0 and Nodes 1.0, placing `Anima/Prompt by Region` from the Node Library at the root succeeded. Repeating the same action from inside that Cube produced the SugarCubes `SugarCube placement failed` modal with the top-level-workflow explanation and did not add a nested Cube. Copying the root Cube, opening its implementation, and pressing Ctrl+V produced the same rejection. In both renderers, a normal `Conditioning (Average)` node could still be added inside the Cube and Comfy's native `Convert Selection to Subgraph` command remained available and completed without Sugar rejection.

An ordinary root Subgraph was then created through that same native conversion command and opened through Comfy's `Enter subgraph` action. SugarCube placement from the Node Library and the legacy Cube sidebar's `Place` action were both rejected there. Its native node context menu retained `Convert to Subgraph`, while `Create Empty SugarCube`, `Create SugarCube from Selection`, and `Convert Selected Subgraph to SugarCube` were all absent. This directly verifies the policy in an ordinary non-root Subgraph rather than only in a Cube implementation. Nodes 2.0 was restored, all isolated test workflows were discarded, and the original `Unsaved Workflow` tab was restored with its 24 Nodes 2.0 cards.
