//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/**
 * Own the SugarCubes host integration layer in `frontend/comfyui/ui.js`.
 */

import { app } from '/scripts/app.js';
import type { ComfyApp, ComfyExtension, ComfyNodeDefinition } from '/scripts/app.js';
import { api } from '/scripts/api.js';
import { createComfyCubeRuntime, type ComfyCubeRuntime } from './ui/cube/ComfyCubeRuntime.js';
import {
  isDraftCubeNode,
  requireCubeIdentity,
  type CubeNode,
} from './ui/cube/node/ComfyCubeNodeFactory.js';
import { readCubeNodeAuthoringCandidate } from './ui/cube/node/CubeNodeAuthoringCandidate.js';
import { ComfyCubeRuntimeLifecycle } from './ui/cube/ComfyCubeRuntimeLifecycle.js';
import { CubePreviewRetentionStore } from './ui/surface/CubePreviewRetentionStore.js';
import { CubeWorkflowPreconfiguration } from './ui/cube/CubeWorkflowPreconfiguration.js';
import { CubeAuthoringHostCommands } from './ui/cube/CubeAuthoringHostCommands.js';
import {
  ComfyPromptQueueBridge,
  type ComfyPromptQueueApi,
} from './ui/cube/execution/ComfyPromptQueueBridge.js';
import { CubePromptPipeline } from './ui/cube/execution/CubePromptPipeline.js';
import { CubeOutputPromptAdapter } from './ui/cube/execution/CubeOutputPromptAdapter.js';
import {
  ComfyCubeOutputEventBridge,
  type ComfyExecutionEventApi,
} from './ui/cube/execution/ComfyCubeOutputEventBridge.js';
import {
  ComfyCubeOutputHistoryAdapter,
  type ComfyCubeOutputHistoryApi,
} from './ui/cube/execution/ComfyCubeOutputHistoryAdapter.js';
import { CubeOutputExecutionStore } from './ui/cube/execution/CubeOutputExecutionStore.js';
import { NativeSubgraphBoundaryResolver } from './ui/cube/graph/NativeSubgraphBoundaryResolver.js';
import { getGroupSugarcubes } from './ui/graph/GroupMetadata.js';
import { coerceVec2, readVector2 } from './ui/graph/VectorUtils.js';
import {
  computePayloadBounds,
  drawGhostRect,
  getPlacementGroupLabel,
  resolvePreviewRect,
} from './ui/overlays/PlacementHelpers.js';
import { buildShiftedPlacementPayload } from './ui/import/PlacementPayload.js';
import { CubeImportCommandService } from './ui/import/CubeImportCommandService.js';
import type { ImportOptions, ImportResult } from './ui/import/CubeImportTypes.js';
import { CubeImportOutcomeReporter } from './ui/import/CubeImportOutcomeReporter.js';
import { CubePreparedImportService } from './ui/import/CubePreparedImportService.js';
import {
  ComfyCanvasDropOriginAdapter,
  type CubePlacementCanvas,
} from './ui/import/ComfyCanvasDropOriginAdapter.js';
import {
  SugarCubesSidebarHostAdapter,
  type SugarCubesSidebarManager,
} from './ui/sidebar/SugarCubesSidebarHostAdapter.js';
import { createPublicApi, getSugarCubesUI } from './ui/index.js';
import { computeInnerBounds } from './ui/graph/CubeBounds.js';
import { createHostSettingsController } from './ui/settings/HostSettingsController.js';
import type { SettingsManager } from './ui/settings/HostSettingsController.js';
import type { UnknownRecord, Vec2 } from './ui/types/common.js';
import type { ComfyApplication } from './ui/types/graph.js';
import { CubeBlueprintPersistenceGuard } from './ui/affordance/CubeBlueprintPersistenceGuard.js';
import { CubeHostAffordanceController } from './ui/affordance/CubeHostAffordanceController.js';
import { CubeAffordanceHostLifecycle } from './ui/affordance/CubeAffordanceHostLifecycle.js';
import { notifyComfyGraphCleared } from './ui/affordance/ComfyGraphClearNotifier.js';
import { createComfyCubePickerIntegration } from './ui/picker/ComfyCubePickerComposition.js';
import { CubeCatalogInvalidationCoordinator } from './ui/core/CubeCatalogInvalidationCoordinator.js';
import { CubeGraphInventory, type CubeInventoryGraph } from './ui/cube/node/CubeGraphInventory.js';

export { buildShiftedPlacementPayload };

type ToastSeverity = 'success' | 'info' | 'warn' | 'error';

interface SugarCubesHostApp extends ComfyApplication {
  ui?: { settings?: SettingsManager };
  extensionManager?: SugarCubesSidebarManager;
  clean?(): void;
  registerExtension?(extension: UnknownRecord): void;
}

export interface SugarCubesExtension extends ComfyExtension, UnknownRecord {
  addCustomNodeDefs(definitions: Record<string, ComfyNodeDefinition>): Promise<void>;
  beforeRegisterVueAppNodeDefs(definitions: ComfyNodeDefinition[]): void;
  registerCustomNodes(): void;
  setup(): Promise<void>;
  beforeConfigureGraph(graphData?: unknown): void;
  afterConfigureGraph(missingNodeTypes: unknown[], comfyApp: ComfyApp): void;
  getCanvasMenuItems(canvas: unknown): unknown[];
  getNodeMenuItems(node: unknown): unknown[];
}

const EXTENSION_NAME = 'SugarCubes.UI';
const IMPORT_STORAGE_KEY = 'SugarCubes.Import.LastCube';

/** Provide the authoritative UI service graph for this host extension instance. */
export const sugarCubesUI = getSugarCubesUI({
  forceNew: true,
  adapterOptions: { app, api },
  applyPreparedImport,
  reportImportOutcome,
  buildShiftedPlacementPayload,
  getCubeAuthoring: () => requireCubeRuntime().authoring,
  getCubeNodeCatalog: () => cubeRuntimeLifecycle.current()?.nodes ?? null,
  validateCubePersistence: () => assertNoNestedCubes('save SugarCubes'),
});
const ui = sugarCubesUI;
const cubeAuthoringCommands = new CubeAuthoringHostCommands({
  canAuthor: () => requireCubeRuntime().graphScope.isCurrentRoot(),
  createCubeFromSelection: () => ui.cubeCreation.startCreateCubeFromSelection(),
  createCubeFromSubgraph: () => ui.cubeCreation.startCreateCubeFromSelectedSubgraph(),
  createEmptyCube: () => ui.cubeCreation.startCreateEmptyCube(),
});
const adapter = ui.adapter;
const storage = ui.storage;
const toastService = ui.toast;
const cubeApi = ui.api;
const overlayManager = ui.overlayManager;
const appRef = adapter.getApp() as SugarCubesHostApp | null;
const windowRef = adapter.getWindow();
const documentRef = adapter.getDocument();
const consoleRef = adapter.getConsole();
const logger = consoleRef || {
  log() {},
  warn() {},
  error() {},
  info() {},
  debug() {},
};

const cubePreconfiguration = new CubeWorkflowPreconfiguration();
const cubePreviewRetention = new CubePreviewRetentionStore();
const cubeRuntimeLifecycle = new ComfyCubeRuntimeLifecycle(createCubeRuntime);
const cubePicker = createComfyCubePickerIntegration({
  api: cubeApi,
  app,
  document: documentRef ?? document,
  getLiteGraph: () => adapter.getLiteGraph?.(),
  getNodeRenderer: () => adapter.getNodeRenderer?.(),
  getRuntime: requireCubeRuntime,
  logger,
  reportError: (summary, detail) => pushToastMessage('error', summary, detail),
});
const cubeCatalogInvalidation = new CubeCatalogInvalidationCoordinator({
  refreshPicker: () => cubePicker.refresh(),
  refreshBrowser: () => ui.cubeBrowser.refresh({ force: true }),
  logger,
});
const nativeBoundaryResolver = new NativeSubgraphBoundaryResolver(logger);
const cubeOutputExecutionStore = new CubeOutputExecutionStore();
const cubeOutputEventBridge = new ComfyCubeOutputEventBridge({
  api: api as unknown as ComfyExecutionEventApi,
  store: cubeOutputExecutionStore,
  logger,
});
const cubeOutputHistoryAdapter = new ComfyCubeOutputHistoryAdapter({
  api: api as unknown as ComfyCubeOutputHistoryApi,
  store: cubeOutputExecutionStore,
  logger,
});
const cubeOutputPromptAdapter = new CubeOutputPromptAdapter({
  getCubes: () => cubeRuntimeLifecycle.current()?.nodes.list() ?? [],
  boundaryResolver: nativeBoundaryResolver,
});
let blueprintPersistenceGuard: CubeBlueprintPersistenceGuard | null = null;
const cubePromptPipeline = new CubePromptPipeline({
  applyProximity: (payload) => overlayManager.proximity.applyProximityToPrompt(payload),
  adaptCubeOutputs: (payload) => cubeOutputPromptAdapter.apply(payload),
});
const promptQueueBridge = new ComfyPromptQueueBridge({
  api: api as unknown as ComfyPromptQueueApi,
  preflight: () => {
    try {
      assertNoNestedCubes('run this workflow');
    } catch (error: unknown) {
      pushToastMessage('error', 'SugarCube execution blocked', readErrorMessage(error));
      throw error;
    }
  },
  transform: (payload) => cubePromptPipeline.transform(payload),
});

/** Validate persisted nesting without requiring renderer or node-factory capabilities. */
function assertNoNestedCubes(action: string): void {
  const rootGraph = appRef?.graph;
  if (!rootGraph) return;
  new CubeGraphInventory(rootGraph as CubeInventoryGraph).assertNoNestedCubes(action);
}
const cubeAffordanceController = new CubeHostAffordanceController({
  getRuntime: () => cubeRuntimeLifecycle.current(),
  cubeCreation: ui.cubeCreation,
  cubeSave: ui.cubeSave,
  confirm: ui.confirmDialog,
  feedback: toastService,
  prepareGraphClear: () => appRef?.clean?.(),
  markGraphDirty: () => {
    appRef?.canvas?.setDirty?.(true, true);
    const graph = appRef?.canvas?.graph ?? appRef?.graph;
    ui.dirtyManager.requestRefresh({ ...(graph ? { graph } : {}), reason: 'cube-affordance' });
  },
  announceGraphCleared: () => notifyComfyGraphCleared(api, logger),
});
const cubeAffordances = new CubeAffordanceHostLifecycle({
  getDocument: () => documentRef,
  getCanvas: () => appRef?.canvas ?? null,
  controller: cubeAffordanceController,
  logger,
});

/** Construct graph-bound Cube collaborators only after Comfy initializes its graph. */
function createCubeRuntime(): ComfyCubeRuntime {
  const liteGraph = adapter.getLiteGraph?.();
  if (!appRef || !liteGraph || !documentRef) {
    throw new Error('SugarCubes native Cube runtime dependencies are unavailable.');
  }
  let runtime: ComfyCubeRuntime | null = null;
  runtime = createComfyCubeRuntime({
    app: appRef,
    api,
    liteGraph,
    document: documentRef,
    logger,
    editorMetadata: {
      canEdit: async (node) => {
        if (isDraftCubeNode(node)) return true;
        const cubeId = readCubeMetadataString(node, 'cube_id');
        return cubeId ? ui.packService.canWriteCube(cubeId) : false;
      },
      save: async (node, values) => ui.cubeEditorSave.save(node, values),
      modelSuggestions: () => ui.cubeBrowser.getModelSuggestions(),
    },
    feedback: toastService,
    boundaryResolver: nativeBoundaryResolver,
    previewRetention: cubePreviewRetention,
    openCubeMenu: (metadata, event) => {
      const instanceId =
        typeof metadata.instance_id === 'string' ? metadata.instance_id.trim() : '';
      const node = instanceId ? runtime?.nodes.get(instanceId) : null;
      overlayManager.openCubeMenu(
        node
          ? {
              ...metadata,
              graphSummary: readCubeNodeAuthoringCandidate(node),
            }
          : metadata,
        event,
      );
    },
    getCubeOutput: (executionId) => cubeOutputExecutionStore.read(executionId),
    subscribePreviewChanges: (listener) => cubeOutputExecutionStore.subscribe(listener),
    onBoundaryGeometryChange: () =>
      overlayManager.proximity.schedulePreview({
        graph: appRef.canvas?.graph ?? appRef.graph,
      }),
    getPreviewLinks: () =>
      overlayManager.proximity.settings.enabled
        ? overlayManager.proximity.promptMatches.flatMap((match) =>
            match.outputId === undefined || match.inputId === undefined
              ? []
              : [
                  {
                    originId: match.outputId,
                    originSlot: match.outputSlot,
                    targetId: match.inputId,
                  },
                ],
          )
        : [],
  });
  overlayManager.proximity.setEndpointSource(runtime.proximityEndpoints);
  overlayManager.proximity.setMatchSink(runtime.proximityPresentation);
  return runtime;
}

/** Read one graph-owned Cube metadata field without leaking dynamic values upward. */
function readCubeMetadataString(node: CubeNode, key: string): string {
  const value = requireCubeIdentity(node)[key];
  return typeof value === 'string' ? value.trim() : '';
}

/** Resolve the first-class Cube runtime only when the active graph requires it. */
function requireCubeRuntime(): ComfyCubeRuntime {
  return cubeRuntimeLifecycle.require();
}

const LAST_CUBE_STORAGE_KEYS = Object.freeze([IMPORT_STORAGE_KEY]);

function readErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function persistLastCubeId(value: unknown): void {
  if (value == null) {
    return;
  }
  const trimmed = typeof value === 'string' ? value.trim() : '';
  if (!trimmed) {
    return;
  }
  try {
    if (!storage) {
      return;
    }
    const seen = new Set();
    for (const key of LAST_CUBE_STORAGE_KEYS) {
      if (!key || seen.has(key)) {
        continue;
      }
      storage.writeValue(key, trimmed);
      seen.add(key);
    }
  } catch (error: unknown) {
    logger.warn('SugarCubes: failed to persist the last imported Cube id.', error);
  }
}

function pushToastMessage(severity: ToastSeverity, summary: string, detail: string): void {
  toastService?.push?.(severity, summary, detail);
}

const hostSettingsController = createHostSettingsController({
  adapter,
  appRef,
  cubeApi,
  ui,
  logger,
  pushToast: pushToastMessage,
  readErrorMessage,
  invalidateDependentCatalogs: () => {
    void cubeCatalogInvalidation.invalidate();
  },
});

const sidebarHost = new SugarCubesSidebarHostAdapter({
  document: documentRef ?? document,
  getManager: () => appRef?.extensionManager,
  browser: ui.cubeBrowser,
  logger,
});
const dropOrigin = new ComfyCanvasDropOriginAdapter({
  getCanvas: () => adapter.getCanvas() as CubePlacementCanvas | null,
  logger,
});
const computeDropOrigin = (): Vec2 => dropOrigin.compute();

function reportImportOutcome(
  defaultAlias: string,
  backendWarnings: unknown[],
  importResult: Partial<ImportResult> | null,
  payloadValue: unknown,
  options: { focus?: boolean } = {},
): void {
  importOutcomeReporter.report(defaultAlias, backendWarnings, importResult, payloadValue, options);
}

async function applyPreparedImport(
  payloadValue: unknown,
  options: ImportOptions & { instanceAlias?: string } = {},
): Promise<ImportResult> {
  return preparedImportService.apply(payloadValue, options);
}

const preparedImportService = new CubePreparedImportService({
  getGraph: () => appRef?.graph,
  getLiteGraph: () => adapter.getLiteGraph?.(),
  getNodeRenderer: () => adapter.getNodeRenderer?.(),
  getRuntime: requireCubeRuntime,
  assertRootPlacement: () => requireCubeRuntime().graphScope.assertCurrentRoot('imported'),
  readErrorMessage,
});

const importOutcomeReporter = new CubeImportOutcomeReporter({
  focusImportedCube: (result) => {
    const canvas = adapter.getCanvas() as CubePlacementCanvas | null;
    const bounds = result.bounds;
    if (!canvas || !bounds) return;
    const rectangle: [number, number, number, number] = [
      bounds.minX,
      bounds.minY,
      bounds.maxX - bounds.minX,
      bounds.maxY - bounds.minY,
    ];
    const redraw = (): void => canvas.setDirty?.(true, true);
    if (typeof canvas.ds?.animateToBounds === 'function') {
      canvas.ds.animateToBounds(rectangle, redraw);
    } else {
      canvas.ds?.fitToBounds?.(rectangle);
      redraw();
    }
  },
  pushToast: pushToastMessage,
});

const importCommandService = new CubeImportCommandService({
  api: cubeApi,
  applyPreparedImport,
  computeDropOrigin,
  reportOutcome: (defaultAlias, backendWarnings, result, payload) =>
    importOutcomeReporter.report(defaultAlias, backendWarnings, result, payload),
  persistLastCubeId: (cubeId) => persistLastCubeId(cubeId),
  pushToast: pushToastMessage,
  readErrorMessage,
});
ui.cubeBrowser.configure({
  actions: {
    computeDropOrigin,
    importCubeByName: (cubeId, options) => importCommandService.importCurrent(cubeId, options),
    importCubeRevision: (cubeId, revisionRef, options) =>
      importCommandService.importRevision(cubeId, revisionRef, options),
    onCubesUpdated: (cubes) => ui.dirtyManager.updateKnownCubes(cubes),
    openConfirmDialog: (options) => ui.confirmDialog.open(options),
    promoteCube: (cube) => ui.promotionService.promote(cube),
    reconcileCubeIdentity: (identity) => ui.identityReconciler.reconcile(identity),
    startCubePlacement: (cubeId, options) => overlayManager.placement.start(cubeId, options),
  },
  helpers: {
    coerceVec2,
    computePayloadBounds: (entries, ctx) =>
      computePayloadBounds(entries, ctx, adapter.getLiteGraph?.()),
    drawGhostRect,
    getPlacementGroupLabel: (defaultAlias, group) =>
      getPlacementGroupLabel(defaultAlias, group, getGroupSugarcubes),
    readVector2,
    resolvePreviewRect: (entry, pos, size, ctx) =>
      resolvePreviewRect(entry, pos, size, ctx, adapter.getLiteGraph?.()),
  },
  placement: {
    commit: () => overlayManager.placement.commit(),
    computeOriginFromEvent: (event) => overlayManager.placement.computeOriginFromEvent(event),
    getState: () => overlayManager.placement.getState(),
    isPointerOverCanvas: (event) => overlayManager.placement.isPointerOverCanvas(event),
    setCommitInProgress: (value) => overlayManager.placement.setCommitInProgress(value),
    setDirty: () => overlayManager.placement.setDirty(),
    setOrigin: (origin) => overlayManager.placement.setOrigin(origin),
    start: (cubeId, options) =>
      overlayManager.placement.start(cubeId, {
        closeBrowser: options.closeBrowser,
        ...(options.defaultAlias ? { defaultAlias: options.defaultAlias } : {}),
      }),
    stop: (reason) => overlayManager.placement.stop(reason),
  },
});

/** Define the ComfyUI extension lifecycle owned by SugarCubes. */
export const sugarCubesExtension: SugarCubesExtension = {
  name: EXTENSION_NAME,
  addCustomNodeDefs(definitions) {
    return cubePicker.contribute(definitions);
  },
  beforeRegisterVueAppNodeDefs(definitions) {
    cubePicker.orderForVue(definitions);
  },
  registerCustomNodes() {
    cubePicker.activate();
  },
  getCanvasMenuItems(canvas) {
    return cubeAuthoringCommands.getCanvasMenuItems(canvas);
  },
  getNodeMenuItems(node) {
    return cubeAffordances.getNodeMenuItems(node);
  },
  async setup() {
    try {
      sidebarHost.register();
      hostSettingsController.register();
      await ui.setup();
      if (typeof Reflect.get(api as object, 'storeUserData') === 'function') {
        blueprintPersistenceGuard ??= new CubeBlueprintPersistenceGuard(api);
        blueprintPersistenceGuard.install();
      } else {
        logger.warn('SugarCubes: Comfy Blueprint persistence guard is unavailable.');
      }
      promptQueueBridge.install();
      cubeOutputEventBridge.install();
      cubeOutputHistoryAdapter.install();
      await cubeOutputHistoryAdapter.hydrateRecent();
      await hostSettingsController.refresh({ checkForUpdates: false });
      const graph = appRef?.canvas?.graph ?? appRef?.graph;
      overlayManager.proximity.refreshOverlayState({
        recompute: true,
        ...(graph ? { graph } : {}),
      });
      hostSettingsController.refreshUi();
      ui.instanceManager.scheduleRefresh({ ...(graph ? { graph } : {}), reason: 'setup' });
      ui.dirtyManager.requestRefresh({ ...(graph ? { graph } : {}), reason: 'setup' });
    } catch (error: unknown) {
      logger.error('SugarCubes: setup failed', error);
      throw error;
    }
  },
  beforeConfigureGraph(graphData?: unknown) {
    try {
      cubeRuntimeLifecycle.reset();
      cubeOutputExecutionStore.clear();
      cubePreconfiguration.prepare(graphData);
      cubeAffordances.attach(requireCubeRuntime());
      overlayManager.proximity.resetOverlayState();
    } catch (error: unknown) {
      logger.error(`SugarCubes: Cube preconfiguration failed: ${readErrorMessage(error)}`, error);
      throw error;
    }
  },
  afterConfigureGraph(missingNodeTypes: unknown[], comfyApp) {
    try {
      const runtime = requireCubeRuntime();
      runtime.hostPlacementGuard.completeHydration();
      runtime.restoreLegacy(cubePreconfiguration.takeLegacyBatch());
      runtime.detectLegacyBlueprints();
      const nestedCubes = runtime.graphInventory.snapshot().nestedCubes;
      if (nestedCubes.length) {
        pushToastMessage(
          'error',
          'Nested SugarCubes need attention',
          `${String(nestedCubes.length)} SugarCube${nestedCubes.length === 1 ? '' : 's'} ` +
            `${nestedCubes.length === 1 ? 'is' : 'are'} inside a Subgraph. ` +
            'The workflow was preserved, but execution and Cube saving are blocked until the nested wrapper is removed.',
        );
      }
      cubeAffordances.adaptMissingNodes(missingNodeTypes);
      void cubeOutputHistoryAdapter.hydrateRecent();
      const graph = appRef?.canvas?.graph ?? comfyApp.graph ?? appRef?.graph;
      overlayManager.proximity.refreshOverlayState({
        recompute: true,
        ...(graph ? { graph } : {}),
      });
      ui.instanceManager.scheduleRefresh({ ...(graph ? { graph } : {}), reason: 'configure' });
      ui.dirtyManager.requestRefresh({ ...(graph ? { graph } : {}), reason: 'configure' });
    } catch (error: unknown) {
      logger.error(`SugarCubes: Cube postconfiguration failed: ${readErrorMessage(error)}`, error);
      throw error;
    }
  },
};

app.registerExtension(sugarCubesExtension);

const debugApi = {
  getDirtyState(instanceId: unknown) {
    return ui.dirtyManager.getDebugState(instanceId);
  },
  bounds: {
    get(instanceId: unknown) {
      if (!instanceId) {
        return null;
      }
      const graph = appRef?.graph || null;
      const index = ui.containmentService?.buildIndex?.(graph) || null;
      const entry = index?.instanceById?.get?.(String(instanceId)) || null;
      if (!entry?.metadata?.bounds) {
        return null;
      }
      return {
        bounds: entry.metadata.bounds,
        inner: computeInnerBounds(entry.metadata.bounds),
      };
    },
    reconcile(instanceId: unknown) {
      const graph = appRef?.graph || null;
      if (!graph || !ui.boundsReconciler) {
        return { changed: [] };
      }
      const result = ui.boundsReconciler.reconcileAll({ graph });
      const changed = Array.from(result.changed || []);
      if (instanceId && !changed.includes(String(instanceId))) {
        return { changed: [] };
      }
      return { changed };
    },
    resolveCollisions(instanceId: unknown) {
      const graph = appRef?.graph || null;
      if (!graph || !ui.collisionService || !instanceId) {
        return { moved: false };
      }
      const index = ui.containmentService?.buildIndex?.(graph) || null;
      return ui.collisionService.resolveCollisions({
        graph,
        activeInstanceId: String(instanceId),
        index,
      });
    },
  },
  layout: {
    service: ui.layoutService,
    appendCube: (options: Parameters<typeof ui.layoutService.appendCube>[0]) =>
      ui.layoutService.appendCube(options),
    insertBetween: (options: Parameters<typeof ui.layoutService.insertBetween>[0]) =>
      ui.layoutService.insertBetween(options),
    insertBefore: (options: Parameters<typeof ui.layoutService.insertBefore>[0]) =>
      ui.layoutService.insertBefore(options),
    swapOrder: (options: Parameters<typeof ui.layoutService.swapOrder>[0]) =>
      ui.layoutService.swapOrder(options),
    replaceCube: (options: Parameters<typeof ui.layoutService.replaceCube>[0]) =>
      ui.layoutService.replaceCube(options),
  },
};

if (windowRef) {
  Object.assign(windowRef, {
    SugarCubes: createPublicApi(ui),
    SugarCubesDebug: debugApi,
  });
}
