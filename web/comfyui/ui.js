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
import { api } from '/scripts/api.js';
import { createEditorActionsRuntime } from './ui/editor_actions/EditorActionsComposition.js';
import { createComfyCubeRuntime } from './ui/cube/ComfyCubeRuntime.js';
import { CubeGraphApi } from './ui/workflow/infrastructure/CubeGraphApi.js';
import { ComfyCubeRuntimeLifecycle } from './ui/cube/ComfyCubeRuntimeLifecycle.js';
import { createComfyWorkflowExecutionRuntime } from './ui/execution/infrastructure/ComfyWorkflowExecutionRuntime.js';
import { projectCubeProximityPreviewLinks } from './ui/surface/CubeProximityPreviewLinks.js';
import { CubeAuthoringHostCommands } from './ui/cube/CubeAuthoringHostCommands.js';
import { createCubeHostExecution } from './ui/cube/execution/CubeHostExecutionComposition.js';
import { createWorkflowFieldHost } from './ui/relations/WorkflowFieldComposition.js';
import { ComfyCanvasHistoryAdapter } from './ui/surface/ComfyCanvasHistoryAdapter.js';
import { proximityWorkflowConnections } from './ui/overlays/proximity/ProximityWorkflowConnections.js';
import { NativeSubgraphBoundaryResolver } from './ui/cube/graph/NativeSubgraphBoundaryResolver.js';
import { buildShiftedPlacementPayload } from './ui/import/PlacementPayload.js';
import { CubeImportHostCoordinator } from './ui/import/CubeImportHostCoordinator.js';
import { createPublicApi, getSugarCubesUI } from './ui/index.js';
import { createHostSettingsComposition } from './ui/settings/HostSettingsComposition.js';
import { attachSavingRunContext } from './ui/output_saving/ComfySavingRunContext.js';
import { CubeHostAffordanceController } from './ui/affordance/CubeHostAffordanceController.js';
import { CubeAffordanceHostLifecycle } from './ui/affordance/CubeAffordanceHostLifecycle.js';
import { notifyComfyGraphCleared } from './ui/affordance/ComfyGraphClearNotifier.js';
import { createComfyCubePickerIntegration } from './ui/picker/ComfyCubePickerComposition.js';
import { CubeCatalogInvalidationCoordinator } from './ui/core/CubeCatalogInvalidationCoordinator.js';
import { CubeGraphInventory } from './ui/cube/node/CubeGraphInventory.js';
import { CubeGraphConfigurationLifecycle } from './ui/cube/CubeGraphConfigurationLifecycle.js';
import { CubeExtensionSetupLifecycle } from './ui/cube/CubeExtensionSetupLifecycle.js';
import { CubeHostFeedback } from './ui/core/CubeHostFeedback.js';
import { createSugarCubesDebugApi } from './ui/debug/SugarCubesDebugApi.js';
import { ComfyWorkflowWorkspace } from './ui/surface/ComfyWorkflowWorkspace.js';
import { createCubeStackCommands } from './ui/surface/stack/CubeStackCommands.js';
import { ScrollSurfaceWheelHost } from './ui/surface/ScrollSurfaceWheelHost.js';
import { ModelPickerHost } from './ui/model_library/ModelPickerHost.js';
import { ComfyOutputThumbnailSource } from './ui/model_library/ComfyOutputThumbnailSource.js';
import { modelBackgroundReporter } from './ui/model_library/ModelBackgroundDiagnostics.js';
import { ModelUpdateClient } from './ui/model_library/ModelUpdateClient.js';
import { ModelUpdateLifecycle } from './ui/model_library/ModelUpdateLifecycle.js';
import { ModelUpdateSettings } from './ui/model_library/ModelUpdateSettings.js';
import { ModelProviderSettings } from './ui/model_library/ModelProviderSettings.js';
import { ModelProviderCacheSettings } from './ui/model_library/ModelProviderCacheSettings.js';
import { ModelProviderCacheClient } from './ui/model_library/ModelProviderCacheClient.js';
import { ModelProviderPreferenceClient } from './ui/model_library/ModelProviderPreferences.js';
import { ModelCatalogClient } from './ui/model_library/ModelCatalogClient.js';
import { CubePromptEditorHost } from './ui/prompt_editor/CubePromptEditorHost.js';
import { OwnedSurfaceAppearance } from './ui/surface/appearance/OwnedSurfaceAppearance.js';
import { ModelDetectorClient } from './ui/model_library/ModelDetectorClient.js';
import { refreshComfyModelChoices } from './ui/model_library/ComfyModelChoices.js';
import { refreshNativeNodeCards } from './ui/surface/NativeNodeCardRefresh.js';
import { ModelDiscoveryClient } from './ui/model_library/ModelDiscoveryClient.js';
import { CivitaiCredentialClient } from './ui/model_library/CivitaiCredentialClient.js';
import { ModelRootClient } from './ui/model_library/ModelRootClient.js';
import { ModelLibrarySettings } from './ui/model_library/ModelLibrarySettings.js';
export { buildShiftedPlacementPayload };
const EXTENSION_NAME = 'SugarCubes.UI';
let invalidateCubeCatalogs = async () => undefined;
/** Provide the authoritative UI service graph for this host extension instance. */
export const sugarCubesUI = getSugarCubesUI({
    forceNew: true,
    adapterOptions: { app, api },
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
const appRef = adapter.getApp();
const windowRef = adapter.getWindow();
const documentRef = adapter.getDocument();
const logger = adapter.getConsole() ?? console;
const hostFeedback = new CubeHostFeedback({
    toast: toastService,
});
const cubePreconfiguration = ui.workflowPreconfiguration;
const workflowExecution = createComfyWorkflowExecutionRuntime({ app: appRef, api, logger });
const cubeRuntimeLifecycle = new ComfyCubeRuntimeLifecycle(createCubeRuntime);
const { appearance, relations, frame, setupPresentation } = createWorkflowFieldHost({
    app: appRef,
    document: documentRef ?? document,
    seeds: workflowExecution.seeds,
    activity: workflowExecution.activity,
    progress: workflowExecution.progress,
    stackControls: () => workflowWorkspace.masonry.stackControls(),
    sessions: workflowExecution.sessions,
    getCardState: () => cubeRuntimeLifecycle.current()?.cardState ?? null,
    getConnections: (graph) => proximityWorkflowConnections(overlayManager.proximity, graph),
    logger,
    reportPresentationError: (error) => {
        logger.error('SugarCubes presentation failed', { error });
        pushToastMessage('error', 'Interface unavailable', readErrorMessage(error));
    },
    reportRelationError: (error) => {
        logger.error('SugarCubes workflow relations failed', { error });
        pushToastMessage('error', 'Shared field unavailable', readErrorMessage(error));
    },
});
const cubePicker = createComfyCubePickerIntegration({
    appearance: appearance.state,
    onStackApplied: () => workflowWorkspace.masonry.refresh(),
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
    refreshLibrary: () => ui.libraryCatalog.refresh({ force: true }),
    logger,
});
invalidateCubeCatalogs = () => cubeCatalogInvalidation.invalidate();
const nativeBoundaryResolver = new NativeSubgraphBoundaryResolver(logger);
const promptQueueBridge = createCubeHostExecution({
    api,
    observer: workflowExecution.submissionObserver,
    logger,
    boundaryResolver: nativeBoundaryResolver,
    getGraph: () => appRef?.graph,
    getCubes: () => cubeRuntimeLifecycle.current()?.nodes.list() ?? [],
    getProximityMatches: () => overlayManager.proximity.resolveExecutionMatches(appRef?.canvas?.graph ?? appRef?.graph),
    applyProximity: (payload) => overlayManager.proximity.applyProximityToPrompt(payload),
    prepareRelations: (payload) => {
        try {
            workflowExecution.assertCurrentPrompt(payload);
            return attachSavingRunContext(relations.prepare(payload), workflowExecution.submissionContext(payload));
        }
        catch (error) {
            pushToastMessage('error', 'Workflow run not queued', readErrorMessage(error));
            throw error;
        }
    },
    preflight: () => {
        try {
            assertNoNestedCubes('run this workflow');
        }
        catch (error) {
            pushToastMessage('error', 'SugarCube execution blocked', readErrorMessage(error));
            throw error;
        }
    },
});
/** Validate persisted nesting without requiring renderer or node-factory capabilities. */
function assertNoNestedCubes(action) {
    const rootGraph = appRef?.graph;
    if (!rootGraph)
        return;
    new CubeGraphInventory(rootGraph).assertNoNestedCubes(action);
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
function createCubeRuntime() {
    const liteGraph = adapter.getLiteGraph?.();
    if (!appRef || !liteGraph || !documentRef) {
        throw new Error('SugarCubes native Cube runtime dependencies are unavailable.');
    }
    const runtime = createComfyCubeRuntime({
        app: appRef,
        api,
        cubeApi,
        graphGateway: new CubeGraphApi(cubeApi),
        reconcileCubeOrder: () => relations.reconcileCubeOrder(),
        liteGraph,
        document: documentRef,
        logger,
        nodePackMetadata: ui.workflowNodePackMetadata,
        openAddCubeMenu: (metadata, anchor) => cubePicker.openAddMenu(metadata, anchor),
        editorMetadata: {
            canEdit: (node) => ui.workflowLibraryPresentation.canEdit(node),
            save: async (node, values) => ui.cubeEditorSave.save(node, values),
            modelSuggestions: () => ui.modelSuggestions.getModelSuggestions(),
        },
        feedback: toastService,
        boundaryResolver: nativeBoundaryResolver,
        getWorkflowOutput: (path) => workflowExecution.outputs.output(path),
        getWorkflowPreview: (path) => workflowExecution.outputs.livePreview(path),
        subscribePreviewChanges: (listener) => workflowExecution.outputs.subscribe(listener),
        revealImage: (target) => workflowWorkspace.canvas.reveal(target),
        presentationChanges: ui.workflowLibraryState,
        resolveIdentitySource: (metadata) => ui.workflowLibraryPresentation.resolveSource(metadata),
        onBoundaryGeometryChange: () => overlayManager.proximity.schedulePreview({
            graph: appRef.canvas?.graph ?? appRef.graph,
        }),
        getPreviewLinks: () => projectCubeProximityPreviewLinks(overlayManager.proximity.settings.enabled, overlayManager.proximity.promptMatches),
    });
    overlayManager.proximity.setEndpointSource(runtime.proximityEndpoints);
    overlayManager.proximity.setMatchSink(runtime.proximityPresentation);
    return runtime;
}
/** Resolve the first-class Cube runtime only when the active graph requires it. */
function requireCubeRuntime() {
    return cubeRuntimeLifecycle.require();
}
const readErrorMessage = (error) => hostFeedback.readErrorMessage(error);
const pushToastMessage = hostFeedback.pushToast.bind(hostFeedback);
const hostSettingsController = createHostSettingsComposition({
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
const importHostCoordinator = new CubeImportHostCoordinator({
    ui,
    app: appRef,
    feedback: hostFeedback,
    getRuntime: requireCubeRuntime,
    logger,
});
const graphConfigurationLifecycle = new CubeGraphConfigurationLifecycle({
    app: appRef,
    ui,
    runtimeLifecycle: cubeRuntimeLifecycle,
    requireRuntime: requireCubeRuntime,
    preconfiguration: cubePreconfiguration,
    affordances: cubeAffordances,
    overlays: overlayManager,
    feedback: hostFeedback,
    logger,
});
const extensionSetupLifecycle = new CubeExtensionSetupLifecycle({
    api,
    app: appRef,
    ui,
    settings: hostSettingsController,
    promptQueue: promptQueueBridge,
    execution: workflowExecution,
    logger,
});
const workflowWorkspace = new ComfyWorkflowWorkspace({
    saving: hostSettingsController.outputSaving,
    app: appRef,
    document: documentRef ?? document,
    execution: workflowExecution,
    logger,
    report: (error) => {
        logger.error('SugarCubes output canvas failed', { error });
        pushToastMessage('error', 'Output canvas unavailable', readErrorMessage(error));
    },
}, {
    stack: createCubeStackCommands({
        document: documentRef ?? document,
        appearance: appearance.state,
        frame: frame.mode,
        picker: cubePicker,
        library: ui.workflowLibraryActions,
        dialogs: ui.dialogs,
        getRuntime: () => cubeRuntimeLifecycle.current(),
        resolveSource: (metadata) => ui.workflowLibraryPresentation.resolveSource(metadata),
        reportError: (error) => {
            logger.error('SugarCubes stack action failed.', { error });
            pushToastMessage('error', 'Cube stack unavailable', readErrorMessage(error));
        },
    }),
    app: appRef,
    document: documentRef ?? document,
    getCubes: () => cubeRuntimeLifecycle.current()?.nodes.list() ?? [],
    getCardState: () => cubeRuntimeLifecycle.current()?.cardState ?? null,
    getGraphChanges: () => cubeRuntimeLifecycle.current()?.graphChanges ?? null,
    getConnections: (graph) => proximityWorkflowConnections(overlayManager.proximity, graph),
    getRenderer: () => {
        const runtime = cubeRuntimeLifecycle.current();
        if (!runtime)
            throw new Error('SugarCubes native renderer is unavailable.');
        return runtime.getNativeRenderer();
    },
    notifyRendererFallback: () => pushToastMessage('info', 'Switched to graph view', 'Masonry requires Nodes 2.0.'),
    reportError: (error) => {
        logger.error('SugarCubes workflow masonry failed', { error });
        pushToastMessage('error', 'Masonry unavailable', readErrorMessage(error));
    },
}, logger);
const workflowHistory = new ComfyCanvasHistoryAdapter(appRef, documentRef ?? document, relations.commitNativeTransaction);
/** Compose theme-independent native scroll input once for the host document. */
const scrollSurfaceInput = new ScrollSurfaceWheelHost(documentRef ?? document);
/** Compose shared model presentation while preserving the native widget owner. */
const modelCatalog = new ModelCatalogClient((path, options) => api.fetchApi(path, options), () => refreshComfyModelChoices(appRef));
/** Bind prompt editing to the same catalog used by native model controls. */
const promptEditors = new CubePromptEditorHost(documentRef ?? document, modelCatalog, modelBackgroundReporter(logger, 'prompt editor'), new OwnedSurfaceAppearance(document, appearance.state));
const modelUpdates = new ModelUpdateClient((path, options) => api.fetchApi(path, options));
const modelDetectors = new ModelDetectorClient((path, options) => api.fetchApi(path, options));
/** Route update failures through the established SugarCubes feedback owner. */
const reportModelUpdateError = (error) => {
    logger.error('SugarCubes model update failed', { error });
    pushToastMessage('error', 'Model updates unavailable', readErrorMessage(error));
};
const modelUpdateLifecycle = new ModelUpdateLifecycle(window, modelUpdates, modelBackgroundReporter(logger, 'model update lifecycle'));
const modelUpdateSettings = new ModelUpdateSettings(documentRef ?? document, modelUpdates, reportModelUpdateError);
const modelProviderSettings = new ModelProviderSettings(documentRef ?? document, new ModelProviderPreferenceClient((path, options) => api.fetchApi(path, options)), (error) => {
    logger.error('SugarCubes model provider settings failed', { error });
    pushToastMessage('error', 'Model provider settings', readErrorMessage(error));
});
const modelProviderCacheSettings = new ModelProviderCacheSettings(documentRef ?? document, new ModelProviderCacheClient((path, options) => api.fetchApi(path, options)), (error) => {
    logger.error('SugarCubes model cache settings failed', { error });
    pushToastMessage('error', 'Model cache', readErrorMessage(error));
});
const modelPickers = new ModelPickerHost({
    app: appRef,
    document: documentRef ?? document,
    catalog: modelCatalog,
    feedback: (severity, message) => pushToastMessage(severity, 'Model library', message),
    updates: modelUpdates,
    detectors: modelDetectors,
    outputs: new ComfyOutputThumbnailSource(appRef, documentRef ?? document, (executionId) => workflowExecution.outputs.output(executionId)),
    reportError: modelBackgroundReporter(logger, 'model picker'),
    discovery: {
        discovery: new ModelDiscoveryClient((path, options) => api.fetchApi(path, options), async () => {
            await refreshComfyModelChoices(appRef);
            await modelCatalog.load(true);
        }),
        credentials: new CivitaiCredentialClient((path, options) => api.fetchApi(path, options)),
        cubes: () => cubeRuntimeLifecycle.current()?.nodes.list() ?? [],
    },
});
const editorActions = createEditorActionsRuntime(adapter, modelCatalog, logger, hostFeedback, modelPickers);
/** Compose shared model-root settings without duplicating persistence or dialog policy. */
const modelLibrarySettings = new ModelLibrarySettings({
    app: appRef,
    document: documentRef ?? document,
    catalog: modelCatalog,
    roots: new ModelRootClient((path, options) => api.fetchApi(path, options)),
    dialogs: ui.dialogs,
    reportError: (error) => {
        logger.error('SugarCubes model library settings failed', { error });
        pushToastMessage('error', 'Model library unavailable', readErrorMessage(error));
    },
});
/** Define the ComfyUI extension lifecycle owned by SugarCubes. */
export const sugarCubesExtension = {
    name: EXTENSION_NAME,
    commands: [
        {
            id: 'SugarCubes.CubeStackCart',
            label: 'Add cubes / edit Cube stack',
            function: () => cubePicker.openCart(),
        },
        {
            id: 'SugarCubes.ToggleMasonry',
            label: 'Toggle workflow masonry',
            function: () => workflowWorkspace.masonry.toggle(),
        },
        {
            id: 'SugarCubes.ToggleOutputCanvas',
            label: 'Toggle output canvas',
            function: () => workflowWorkspace.canvas.toggle(),
        },
    ],
    addCustomNodeDefs(definitions) {
        return cubePicker.contribute(definitions);
    },
    beforeRegisterVueAppNodeDefs(definitions) {
        cubePicker.orderForVue(definitions);
    },
    registerCustomNodes() {
        cubePicker.activate();
    },
    refreshComboInNodes() {
        refreshNativeNodeCards(documentRef ?? document, { kind: 'reset', reason: 'native-choices' });
    },
    getCanvasMenuItems(canvas) {
        return cubeAuthoringCommands.getCanvasMenuItems(canvas);
    },
    getNodeMenuItems(node) {
        return cubeAffordances.getNodeMenuItems(node);
    },
    async setup() {
        promptEditors.setup();
        await extensionSetupLifecycle.setup();
        workflowHistory.install();
        relations.setup();
        workflowWorkspace.setup();
        scrollSurfaceInput.setup();
        setupPresentation();
        modelPickers.setup();
        await editorActions.setup();
        modelLibrarySettings.setup();
        modelUpdateSettings.setup(appRef);
        modelProviderSettings.setup(appRef);
        modelProviderCacheSettings.setup(appRef);
        modelUpdateLifecycle.start();
    },
    beforeConfigureGraph(graphData) {
        promptEditors.beforeConfigureGraph();
        workflowHistory.setConfiguring(true);
        relations.beforeConfigureGraph();
        workflowWorkspace.masonry.beforeConfigureGraph();
        workflowWorkspace.canvas.beforeConfigureGraph();
        cubePicker.closeAddMenu();
        graphConfigurationLifecycle.beforeConfigureGraph(graphData);
    },
    afterConfigureGraph(missingNodeTypes, comfyApp) {
        graphConfigurationLifecycle.afterConfigureGraph(missingNodeTypes, comfyApp);
        workflowWorkspace.canvas.afterConfigureGraph();
        workflowWorkspace.masonry.afterConfigureGraph();
        relations.afterConfigureGraph();
        workflowHistory.setConfiguring(false);
    },
};
app.registerExtension(sugarCubesExtension);
const debugApi = createSugarCubesDebugApi({
    ui,
    app: appRef,
    canvas: () => workflowWorkspace.canvas.diagnostics(),
});
if (windowRef) {
    Object.assign(windowRef, {
        SugarCubes: createPublicApi(ui, {
            setSubstituteFrame: (enabled) => frame.mode.setEnabled(enabled),
            desktopFrame: frame,
            openLibrary: () => cubePicker.openCart(),
            importSugarScript: (source) => importHostCoordinator.importSugarScript(source),
        }),
        SugarCubesDebug: debugApi,
    });
}
