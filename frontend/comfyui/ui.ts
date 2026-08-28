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
import { ComfyCubeRuntimeLifecycle } from './ui/cube/ComfyCubeRuntimeLifecycle.js';
import { CubePreviewRetentionStore } from './ui/surface/CubePreviewRetentionStore.js';
import { CubeAuthoringHostCommands } from './ui/cube/CubeAuthoringHostCommands.js';
import {
  ComfyPromptQueueBridge,
  type ComfyPromptQueueApi,
} from './ui/cube/execution/ComfyPromptQueueBridge.js';
import { CubePromptPipeline } from './ui/cube/execution/CubePromptPipeline.js';
import { CubeOutputPromptAdapter } from './ui/cube/execution/CubeOutputPromptAdapter.js';
import { createCubeDirectExecution } from './ui/cube/execution/CubeDirectExecutionService.js';
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
import { buildShiftedPlacementPayload } from './ui/import/PlacementPayload.js';
import type { ImportOptions, ImportResult } from './ui/import/CubeImportTypes.js';
import { CubeImportHostCoordinator } from './ui/import/CubeImportHostCoordinator.js';
import {
  SugarCubesSidebarHostAdapter,
  type SugarCubesSidebarManager,
} from './ui/sidebar/SugarCubesSidebarHostAdapter.js';
import { createPublicApi, getSugarCubesUI } from './ui/index.js';
import { createHostSettingsController } from './ui/settings/HostSettingsController.js';
import type { SettingsManager } from './ui/settings/HostSettingsController.js';
import type { UnknownRecord } from './ui/types/common.js';
import type { ComfyApplication } from './ui/types/graph.js';
import { CubeHostAffordanceController } from './ui/affordance/CubeHostAffordanceController.js';
import { CubeAffordanceHostLifecycle } from './ui/affordance/CubeAffordanceHostLifecycle.js';
import { notifyComfyGraphCleared } from './ui/affordance/ComfyGraphClearNotifier.js';
import { createComfyCubePickerIntegration } from './ui/picker/ComfyCubePickerComposition.js';
import { CubeCatalogInvalidationCoordinator } from './ui/core/CubeCatalogInvalidationCoordinator.js';
import { CubeGraphInventory, type CubeInventoryGraph } from './ui/cube/node/CubeGraphInventory.js';
import { CubeGraphConfigurationLifecycle } from './ui/cube/CubeGraphConfigurationLifecycle.js';
import { CubeExtensionSetupLifecycle } from './ui/cube/CubeExtensionSetupLifecycle.js';
import { CubeHostFeedback, type ToastSeverity } from './ui/core/CubeHostFeedback.js';
import { createSugarCubesDebugApi } from './ui/debug/SugarCubesDebugApi.js';

export { buildShiftedPlacementPayload };

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
let invalidateCubeCatalogs: () => Promise<void> = async () => undefined;

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
  invalidateCubeCatalogs: () => invalidateCubeCatalogs(),
});
const ui = sugarCubesUI;
const cubeAuthoringCommands = new CubeAuthoringHostCommands({
  canAuthor: () => requireCubeRuntime().graphScope.isCurrentRoot(),
  createCubeFromSelection: () => ui.cubeCreation.startCreateCubeFromSelection(),
  createCubeFromSubgraph: () => ui.cubeCreation.startCreateCubeFromSelectedSubgraph(),
  createEmptyCube: () => ui.cubeCreation.startCreateEmptyCube(),
});
const adapter = ui.adapter;
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
const hostFeedback = new CubeHostFeedback({
  storage: ui.storage,
  toast: toastService,
  logger,
  lastCubeStorageKeys: [IMPORT_STORAGE_KEY],
});

const cubePreconfiguration = ui.workflowPreconfiguration;
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
  isProximityStrict: () => Boolean(overlayManager.proximity.settings.strict),
});
const cubeCatalogInvalidation = new CubeCatalogInvalidationCoordinator({
  refreshPicker: () => cubePicker.refresh(),
  refreshBrowser: () => ui.cubeBrowser.refresh({ force: true }),
  logger,
});
invalidateCubeCatalogs = () => cubeCatalogInvalidation.invalidate();
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
const cubePromptPipeline = new CubePromptPipeline({
  applyProximity: (payload) => overlayManager.proximity.applyProximityToPrompt(payload),
  adaptCubeOutputs: (payload) => cubeOutputPromptAdapter.apply(payload),
});
const directExecution = createCubeDirectExecution({
  api,
  logger,
  getGraph: () => appRef?.graph,
  getProximityMatches: () =>
    overlayManager.proximity.resolveExecutionMatches(appRef?.canvas?.graph ?? appRef?.graph),
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
  executeDirect: (position, payload, options) => directExecution.queue(position, payload, options),
  shouldExecuteDirect: (payload) => directExecution.owns(payload),
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
  libraryActions: ui.workflowLibraryActions,
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
    cubeApi,
    liteGraph,
    document: documentRef,
    logger,
    nodePackMetadata: ui.workflowNodePackMetadata,
    openAddCubeMenu: (metadata, anchor) => cubePicker.openAddMenu(metadata, anchor),
    editorMetadata: {
      canEdit: (node) => ui.workflowLibraryPresentation.canEdit(node),
      save: async (node, values) => ui.cubeEditorSave.save(node, values),
      modelSuggestions: () => ui.cubeBrowser.getModelSuggestions(),
    },
    feedback: toastService,
    boundaryResolver: nativeBoundaryResolver,
    previewRetention: cubePreviewRetention,
    getCubeOutput: (executionId) => cubeOutputExecutionStore.read(executionId),
    subscribePreviewChanges: (listener) => cubeOutputExecutionStore.subscribe(listener),
    presentationChanges: ui.workflowLibraryState,
    resolveIdentitySource: (metadata) => ui.workflowLibraryPresentation.resolveSource(metadata),
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

/** Resolve the first-class Cube runtime only when the active graph requires it. */
function requireCubeRuntime(): ComfyCubeRuntime {
  return cubeRuntimeLifecycle.require();
}

const readErrorMessage = (error: unknown): string => hostFeedback.readErrorMessage(error);

function pushToastMessage(severity: ToastSeverity, summary: string, detail: string): void {
  hostFeedback.pushToast(severity, summary, detail);
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
const importHostCoordinator = new CubeImportHostCoordinator({
  ui,
  app: appRef,
  feedback: hostFeedback,
  getRuntime: requireCubeRuntime,
  logger,
});
function reportImportOutcome(
  defaultAlias: string,
  backendWarnings: unknown[],
  importResult: Partial<ImportResult> | null,
  payloadValue: unknown,
  options: { focus?: boolean } = {},
): void {
  importHostCoordinator.report(defaultAlias, backendWarnings, importResult, payloadValue, options);
}

async function applyPreparedImport(
  payloadValue: unknown,
  options: ImportOptions & { instanceAlias?: string } = {},
): Promise<ImportResult> {
  return importHostCoordinator.apply(payloadValue, options);
}

const graphConfigurationLifecycle = new CubeGraphConfigurationLifecycle({
  app: appRef,
  ui,
  runtimeLifecycle: cubeRuntimeLifecycle,
  requireRuntime: requireCubeRuntime,
  preconfiguration: cubePreconfiguration,
  affordances: cubeAffordances,
  overlays: overlayManager,
  outputStore: cubeOutputExecutionStore,
  outputHistory: cubeOutputHistoryAdapter,
  feedback: hostFeedback,
  logger,
});
const extensionSetupLifecycle = new CubeExtensionSetupLifecycle({
  api,
  app: appRef,
  ui,
  sidebar: sidebarHost,
  settings: hostSettingsController,
  promptQueue: promptQueueBridge,
  outputEvents: cubeOutputEventBridge,
  outputHistory: cubeOutputHistoryAdapter,
  logger,
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
    await extensionSetupLifecycle.setup();
  },
  beforeConfigureGraph(graphData?: unknown) {
    graphConfigurationLifecycle.beforeConfigureGraph(graphData);
  },
  afterConfigureGraph(missingNodeTypes: unknown[], comfyApp) {
    graphConfigurationLifecycle.afterConfigureGraph(missingNodeTypes, comfyApp);
  },
};

app.registerExtension(sugarCubesExtension);

const debugApi = createSugarCubesDebugApi({ ui, app: appRef });

if (windowRef) {
  Object.assign(windowRef, {
    SugarCubes: createPublicApi(ui, {
      importSugarScript: (source) => importHostCoordinator.importSugarScript(source),
    }),
    SugarCubesDebug: debugApi,
  });
}
