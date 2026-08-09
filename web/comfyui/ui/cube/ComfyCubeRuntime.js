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
/** Adapt Comfy's graph runtime to first-class native Cube nodes. */
import { isRecord } from '../types/common.js';
import { ComfyCubeAuthoringAdapter, } from './ComfyCubeAuthoringAdapter.js';
import { ComfyCubeGraphBuilder } from './ComfyCubeGraphBuilder.js';
import { CubePlacementService } from './CubePlacementService.js';
import { CubeConstructionService } from './CubeConstructionService.js';
import { resolveCubeInitialSurfaceSize } from '../surface/CubeInitialSurfaceSize.js';
import { CubeSubgraphRegistrar } from './CubeSubgraphRegistrar.js';
import { CubeOutputSurfaceSynchronizer } from './CubeOutputSurfaceSynchronizer.js';
import { LegacyCubeDefinitionSerializer } from './migration/LegacyCubeDefinitionSerializer.js';
import { LegacyCubeGraphBuilder } from './migration/LegacyCubeGraphBuilder.js';
import { LegacyCubeMigrationCoordinator, } from './migration/LegacyCubeMigrationCoordinator.js';
import { CubeSurfacePresenter } from '../surface/CubeSurfacePresenter.js';
import { ComfyCubePreviewCatalog } from '../surface/ComfyCubePreviewCatalog.js';
import { CubeEditorNavigationPresenter } from '../surface/CubeEditorNavigationPresenter.js';
import { CubeEditorMetadataHud, } from '../surface/CubeEditorMetadataHud.js';
import { ComfyCanvasViewStateAdapter } from '../surface/ComfyCanvasViewStateAdapter.js';
import { ComfyCanvasGraphFocusAdapter } from '../surface/ComfyCanvasGraphFocusAdapter.js';
import { ComfyCanvasSelectionStateAdapter } from '../surface/ComfyCanvasSelectionStateAdapter.js';
import { createComfyCubeHistoryAdapter } from './ComfyCubeHistoryAdapter.js';
import { ComfyCubeNodeFactory } from './node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from './node/CubeNodeCatalog.js';
import { CubeNodeSwapCoordinator } from './node/CubeNodeSwapCoordinator.js';
import { ComfyCubeNodeLifecycleAdapter } from './node/ComfyCubeNodeLifecycleAdapter.js';
import { createComfyCubePlacementRuntime, } from './placement/ComfyCubePlacementRuntime.js';
import { LegacyCubeContainerMigrationAdapter } from './migration/LegacyCubeContainerMigrationAdapter.js';
import { NativeCubeProximityEndpointSource } from './connection/NativeCubeProximityEndpointSource.js';
import { CubePortPresentationController } from './connection/CubePortPresentationController.js';
import { ComfyCanvasGraphChangeAdapter } from '../surface/ComfyCanvasGraphChangeAdapter.js';
import { createComfyRendererModeChangeSource, resolveComfyRendererMode, } from '../core/ComfyRendererMode.js';
import { NativeCubeGeometryCoordinator } from './geometry/NativeCubeGeometryCoordinator.js';
import { CubeEditorWorkspaceChromeAdapter } from '../surface/CubeEditorWorkspaceChromeAdapter.js';
import { createComfyCubePreviewActions } from '../surface/ComfyCubePreviewActions.js';
import { CubeEditorContextResolver } from '../surface/CubeEditorContextResolver.js';
import { CubeStructuralOperationGuard } from '../affordance/CubeStructuralOperationGuard.js';
import { CubeNodeProductIdentityPresenter } from '../affordance/CubeNodeProductIdentityPresenter.js';
import { CubeBlueprintMigrationDetector } from '../affordance/CubeBlueprintMigrationDetector.js';
import { CubeCardRevealService } from '../surface/CubeCardRevealService.js';
import { CubeVersionRepository } from './version/CubeVersionRepository.js';
import { CubeVersionAvailabilityService } from './version/CubeVersionAvailabilityService.js';
import { ComfyCubeVersionReplacementAdapter } from './version/ComfyCubeVersionReplacementAdapter.js';
import { ComfyCubeVersionSelectionAdapter } from './version/ComfyCubeVersionSelectionAdapter.js';
import { CubeVersionSwitchService } from './version/CubeVersionSwitchService.js';
import { CubeVersionDefinitionStager } from './version/CubeVersionDefinitionStager.js';
/** Construct the graph-bound SugarCubes integration around native SubgraphNodes. */
export function createComfyCubeRuntime(options) {
    const app = requireRecord(options.app, 'Comfy application');
    const api = requireRecord(options.api, 'Comfy API');
    const liteGraph = requireRecord(options.liteGraph, 'LiteGraph');
    const previewActions = createComfyCubePreviewActions({
        document: options.document,
        liteGraph,
        logger: options.logger,
    });
    const graph = requireRecord(app.graph, 'Comfy root graph');
    const canvas = requireRecord(app.canvas, 'Comfy canvas');
    const createNodeFunction = requireFunction(liteGraph.createNode, 'LiteGraph.createNode');
    const createSubgraphFunction = requireFunction(graph.createSubgraph, 'LGraph.createSubgraph');
    const addFunction = requireFunction(graph.add, 'LGraph.add');
    const removeFunction = requireFunction(graph.remove, 'LGraph.remove');
    const getLinkFunction = requireFunction(graph.getLink, 'LGraph.getLink');
    const getNodeByIdFunction = requireFunction(graph.getNodeById, 'LGraph.getNodeById');
    const selectedItems = requireSet(canvas.selectedItems, 'LGraphCanvas.selectedItems');
    const processSelectFunction = requireFunction(canvas.processSelect, 'LGraphCanvas.processSelect');
    const updateSelectedItemsFunction = typeof canvas.updateSelectedItems === 'function' ? canvas.updateSelectedItems : null;
    const apiUrlFunction = requireFunction(api.apiURL, 'Comfy API.apiURL');
    const openSubgraphFunction = typeof canvas.openSubgraph === 'function' ? canvas.openSubgraph : null;
    const runtimeGraph = requireRuntimeGraph(graph);
    const legacyCanvas = requireLiteGraphCubeNodeCanvas(canvas);
    const { graphScope, graphInventory, hostPlacementGuard } = createComfyCubePlacementRuntime({
        rootGraph: graph,
        canvas,
        logger: options.logger,
        ...(options.feedback ? { feedback: options.feedback } : {}),
    });
    const titleHeight = readPositiveNumber(liteGraph.NODE_TITLE_HEIGHT, 30);
    const previewEvents = readPreviewEventSource(api);
    const createNode = (type) => {
        const value = createNodeFunction.call(liteGraph, type);
        return isNativeGraphNode(value) ? value : null;
    };
    const discardSubgraph = (subgraph) => {
        runtimeGraph.subgraphs.delete(subgraph.id);
        if (typeof liteGraph.unregisterNodeType === 'function') {
            liteGraph.unregisterNodeType.call(liteGraph, subgraph.id);
        }
    };
    const graphBuilderHost = {
        rootGraph: {
            createSubgraph(data) {
                const value = createSubgraphFunction.call(graph, data);
                if (!isNativeCubeSubgraph(value)) {
                    throw new TypeError('Comfy returned an invalid native subgraph.');
                }
                return value;
            },
        },
        createNode,
        createUuid,
        discardSubgraph,
    };
    const history = createComfyCubeHistoryAdapter({ canvas, graph });
    const nodes = new CubeNodeCatalog();
    const cardReveal = new CubeCardRevealService({ nodes, history });
    const contexts = new CubeEditorContextResolver(nodes);
    const portPresentation = new CubePortPresentationController({
        requestFrame: (callback) => options.document.defaultView?.requestAnimationFrame(callback) ?? null,
        invalidate: () => history.setDirtyCanvas?.(true, true),
    });
    const proximityEndpoints = new NativeCubeProximityEndpointSource(options.logger, options.boundaryResolver, portPresentation);
    const nodeLifecycle = new ComfyCubeNodeLifecycleAdapter({
        graph: runtimeGraph,
        inventory: graphInventory,
        catalog: nodes,
        events: legacyCanvas.canvas,
        createInstanceId: createUuid,
        logger: options.logger,
    });
    const outputSurfaceSynchronizer = new CubeOutputSurfaceSynchronizer(nodes, legacyCanvas.canvas);
    const nodeFactory = new ComfyCubeNodeFactory({ createNode });
    const nodeSwap = new CubeNodeSwapCoordinator({
        graph,
        nodes,
        history,
        setDirtyCanvas: (foreground, background) => history.setDirtyCanvas?.(foreground, background),
    });
    const chromeActions = {
        onSwapLeft: (metadata) => nodeSwap.swap(metadata, 'left'),
        onSwapRight: (metadata) => nodeSwap.swap(metadata, 'right'),
        canSwap: (metadata, direction) => nodeSwap.canSwap(metadata, direction),
    };
    const legacyContainerMigration = new LegacyCubeContainerMigrationAdapter({
        graph: runtimeGraph,
        factory: nodeFactory,
        nodes,
        logger: options.logger,
        setDirtyCanvas: (foreground, background) => history.setDirtyCanvas?.(foreground, background),
    });
    const canvasView = new ComfyCanvasViewStateAdapter(canvas, (foreground, background) => history.setDirtyCanvas?.(foreground, background), options.logger);
    const canvasFocus = new ComfyCanvasGraphFocusAdapter(canvas, (foreground, background) => history.setDirtyCanvas?.(foreground, background));
    const canvasSelection = new ComfyCanvasSelectionStateAdapter(selectedItems, updateSelectedItemsFunction ? () => updateSelectedItemsFunction.call(canvas) : null, options.logger);
    const canvasGraphChanges = new ComfyCanvasGraphChangeAdapter(canvas);
    const rendererChanges = createComfyRendererModeChangeSource(app);
    const metadataHud = options.editorMetadata
        ? new CubeEditorMetadataHud(options.document, options.editorMetadata, {
            workspaceChrome: new CubeEditorWorkspaceChromeAdapter(options.document),
        })
        : null;
    const geometryWindow = options.document.defaultView;
    const nativeGeometry = new NativeCubeGeometryCoordinator({
        document: options.document,
        nodes,
        graphChanges: canvasGraphChanges,
        getCurrentGraph: () => (isRecord(canvas.graph) ? canvas.graph : null),
        getRenderer: () => resolveComfyRendererMode(app, liteGraph, options.document),
        scheduler: {
            schedule: (callback, delayMs) => geometryWindow?.setTimeout(callback, delayMs) ?? globalThis.setTimeout(callback, delayMs),
            cancel: (handle) => {
                const timer = Number(handle);
                if (Number.isFinite(timer)) {
                    if (geometryWindow)
                        geometryWindow.clearTimeout(timer);
                    else
                        globalThis.clearTimeout(timer);
                }
            },
        },
        logger: options.logger,
    });
    const editorNavigation = new CubeEditorNavigationPresenter({
        canvas: {
            setGraph(targetGraph) {
                canvasGraphChanges.setGraph(targetGraph);
            },
            ...(openSubgraphFunction
                ? {
                    openSubgraph(targetGraph, fromNode) {
                        openSubgraphFunction.call(canvas, targetGraph, fromNode);
                    },
                }
                : {}),
            captureView: () => canvasView.capture(),
            restoreView: (state) => canvasView.restore(state),
            focusBounds: (bounds) => canvasFocus.focus(bounds),
            captureSelection: () => canvasSelection.capture(),
            restoreSelection: (items) => canvasSelection.restore(items),
        },
        rootGraph: graph,
        getCurrentGraph: () => (isRecord(canvas.graph) ? canvas.graph : null),
        nodes,
        contexts,
        graphChanges: canvasGraphChanges,
        ...(metadataHud ? { metadataHud } : {}),
    });
    const presenter = new CubeSurfacePresenter({
        document: options.document,
        openEditor: (node) => editorNavigation.open(node),
        prepareEditor: (node) => editorNavigation.prepare(node),
        rootGraph: graph,
        getCurrentGraph: () => (isRecord(canvas.graph) ? canvas.graph : null),
        nodes,
        setDirtyCanvas: (foreground, background) => history.setDirtyCanvas?.(foreground, background),
        setDropTarget: (node) => {
            app.dragOverNode = node;
        },
        logger: options.logger,
        previewCatalog: new ComfyCubePreviewCatalog({
            getRootGraph: () => graph,
            ...(options.getCubeOutput ? { getCubeOutput: options.getCubeOutput } : {}),
            ...(options.previewRetention ? { retention: options.previewRetention } : {}),
            getEffectiveLinks: () => options.getPreviewLinks?.() ?? [],
            logger: options.logger,
            getNodeOutputs: () => (isRecord(app.nodeOutputs) ? app.nodeOutputs : {}),
            getNodePreviewImages: () => (isRecord(app.nodePreviewImages) ? app.nodePreviewImages : {}),
            buildOutputImageUrl(image) {
                const filename = readNonEmptyString(image.filename);
                if (!filename)
                    return null;
                const params = new URLSearchParams();
                params.set('filename', filename);
                const subfolder = readNonEmptyString(image.subfolder);
                const type = readNonEmptyString(image.type);
                if (subfolder)
                    params.set('subfolder', subfolder);
                if (type)
                    params.set('type', type);
                const preview = callOptionalString(app.getPreviewFormatParam, app);
                const random = callOptionalString(app.getRandParam, app);
                const path = `/view?${params.toString()}${preview}${random}`;
                const value = apiUrlFunction.call(api, path);
                return typeof value === 'string' ? value : null;
            },
        }),
        previewActions,
        getRendererMode: () => resolveComfyRendererMode(app, liteGraph, options.document),
        legacyCanvas,
        titleHeight,
        history,
        chromeActions,
        portPresentation,
        graphChanges: canvasGraphChanges,
        rendererChanges,
        ...(previewEvents ? { previewEvents } : {}),
        ...(options.onBoundaryGeometryChange
            ? { onBoundaryGeometryChange: options.onBoundaryGeometryChange }
            : {}),
        ...(options.subscribePreviewChanges
            ? {
                previewChanges: {
                    subscribe: options.subscribePreviewChanges,
                },
            }
            : {}),
    });
    const graphBuilder = new ComfyCubeGraphBuilder(graphBuilderHost);
    const subgraphRegistrar = new CubeSubgraphRegistrar({
        getSubgraph(id) {
            return runtimeGraph.subgraphs.get(id) ?? null;
        },
        createSubgraph(data) {
            return graphBuilderHost.rootGraph.createSubgraph(data);
        },
        createNode,
        discardSubgraph(id) {
            const subgraph = runtimeGraph.subgraphs.get(id);
            if (subgraph)
                discardSubgraph(subgraph);
        },
    });
    const construction = new CubeConstructionService({
        graphBuilder,
        nodeFactory,
        resolveInitialSize: resolveCubeInitialSurfaceSize,
        createInstanceId: createUuid,
        definitions: {
            discard: discardSubgraph,
        },
    });
    const versionRepository = new CubeVersionRepository(options.cubeApi, options.logger);
    const versionAvailability = new CubeVersionAvailabilityService(versionRepository);
    const versionSelection = new ComfyCubeVersionSelectionAdapter({
        selectedItems,
        processSelect: (node) => void processSelectFunction.call(canvas, node),
    });
    const versionReplacement = new ComfyCubeVersionReplacementAdapter({
        graph: {
            add(node) {
                addFunction.call(graph, node);
            },
            remove(node) {
                removeFunction.call(graph, node);
            },
            getNodeById(id) {
                return getNodeByIdFunction.call(graph, id);
            },
            getLink(id) {
                const value = getLinkFunction.call(graph, id);
                return isRecord(value) ? value : null;
            },
        },
        catalog: nodes,
        history,
        selection: versionSelection,
        logger: options.logger,
    });
    const versionSwitch = new CubeVersionSwitchService({
        availability: versionAvailability,
        repository: versionRepository,
        construction,
        replacement: versionReplacement,
        definitions: new CubeVersionDefinitionStager({
            registrar: subgraphRegistrar,
            createId: createUuid,
        }),
    });
    const placement = new CubePlacementService({
        construction,
        graph: {
            add(node) {
                addFunction.call(graph, node);
            },
        },
        catalog: nodes,
        history,
    });
    const authoring = new ComfyCubeAuthoringAdapter({
        graph,
        subgraphs: runtimeGraph.subgraphs,
        convertToSubgraph(items) {
            const activeGraph = readActiveGraph(canvas, graph);
            const convert = requireFunction(activeGraph.convertToSubgraph, 'LGraph.convertToSubgraph');
            return convert.call(activeGraph, items);
        },
        canvas: {
            selectedItems,
            ...(updateSelectedItemsFunction
                ? { updateSelectedItems: () => updateSelectedItemsFunction.call(canvas) }
                : {}),
        },
        nodeFactory,
        catalog: nodes,
        graphScope,
        createEmptySubgraph: (title, description) => graphBuilder.createEmptyDraft(title, description),
        getDraftPosition: () => readGraphPoint(legacyCanvas.graph_mouse, [0, 0]),
    });
    const legacyMigration = new LegacyCubeMigrationCoordinator({
        graphBuilder: new LegacyCubeGraphBuilder(graphBuilderHost, new LegacyCubeDefinitionSerializer(createUuid)),
        placement,
        graph: {
            getNodeById(id) {
                const value = getNodeByIdFunction.call(graph, id);
                return isConnectableNode(value) ? value : null;
            },
            ...(history.setDirtyCanvas
                ? {
                    setDirtyCanvas: (foreground, background) => history.setDirtyCanvas?.(foreground, background),
                }
                : {}),
        },
        logger: options.logger,
    });
    const structuralGuard = new CubeStructuralOperationGuard({
        rootGraph: graph,
        nodes,
        ...(options.feedback ? { feedback: options.feedback } : {}),
    });
    const productIdentity = new CubeNodeProductIdentityPresenter(nodes);
    const blueprintMigration = new CubeBlueprintMigrationDetector({
        logger: options.logger,
        ...(options.feedback ? { feedback: options.feedback } : {}),
    });
    return {
        construction,
        placement,
        authoring,
        nodes,
        cardReveal,
        versionAvailability,
        versionSwitch,
        graphScope,
        graphInventory,
        hostPlacementGuard,
        contexts,
        metadataHud,
        proximityEndpoints,
        proximityPresentation: portPresentation,
        registerSubgraphs: (payload) => subgraphRegistrar.register(payload),
        restoreLegacy: (batch) => legacyMigration.restore(batch),
        detectLegacyBlueprints: () => blueprintMigration.scan(graph),
        dispose: () => {
            outputSurfaceSynchronizer.dispose();
            editorNavigation.dispose();
            metadataHud?.dispose();
            presenter.dispose();
            legacyContainerMigration.dispose();
            nodeLifecycle.dispose();
            nativeGeometry.dispose();
            canvasGraphChanges.dispose();
            structuralGuard.dispose();
            productIdentity.dispose();
            hostPlacementGuard.dispose();
        },
    };
}
/** Create a stable graph UUID without depending on Comfy's private helper. */
function createUuid() {
    if (typeof globalThis.crypto?.randomUUID === 'function') {
        return globalThis.crypto.randomUUID();
    }
    return `sugarcube-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}
/** Read the graph currently displayed by Comfy's canvas, falling back to the workflow root. */
function readActiveGraph(canvas, fallback) {
    return isRecord(canvas.graph) ? canvas.graph : fallback;
}
/** Validate the node surface required by native Cube graph construction. */
function isNativeGraphNode(value) {
    if (!isRecord(value))
        return false;
    value.properties = isRecord(value.properties) ? value.properties : {};
    return (isNumericVector(value.pos) &&
        isNumericVector(value.size) &&
        Array.isArray(value.inputs) &&
        Array.isArray(value.outputs) &&
        typeof value.connect === 'function');
}
/** Validate the Comfy subgraph API used by Cube assembly and serialization. */
function isNativeCubeSubgraph(value) {
    return (isRecord(value) &&
        typeof value.id === 'string' &&
        typeof value.name === 'string' &&
        Array.isArray(value._nodes) &&
        Array.isArray(value.inputs) &&
        Array.isArray(value.outputs) &&
        isRecord(value.inputNode) &&
        isRecord(value.outputNode) &&
        typeof value.add === 'function' &&
        typeof value.remove === 'function' &&
        typeof value.addInput === 'function' &&
        typeof value.addOutput === 'function' &&
        typeof value.configure === 'function');
}
/** Require Comfy's native definition map and lifecycle hook surface. */
function requireRuntimeGraph(graph) {
    if (!(graph.subgraphs instanceof Map)) {
        throw new TypeError('LGraph.subgraphs is unavailable.');
    }
    return graph;
}
/** Require the exact host capabilities used by the Nodes 1.0 Cube node face. */
function requireLiteGraphCubeNodeCanvas(canvas) {
    if (!(canvas.canvas instanceof HTMLCanvasElement) ||
        typeof canvas.drawNode !== 'function' ||
        typeof canvas.processWidgetClick !== 'function' ||
        !isNumericVector(canvas.graph_mouse)) {
        throw new TypeError('Comfy Nodes 1.0 Cube-node integration is unavailable.');
    }
    return canvas;
}
/** Validate a native node capable of owning one restored graph connection. */
function isConnectableNode(value) {
    return (isRecord(value) &&
        Array.isArray(value.inputs) &&
        Array.isArray(value.outputs) &&
        typeof value.connect === 'function');
}
/** Accept LiteGraph's array and typed-array geometry vectors. */
function isNumericVector(value) {
    return Array.isArray(value) || value instanceof Float32Array || value instanceof Float64Array;
}
/** Read one finite graph-space point from Comfy's mutable coordinate vector. */
function readGraphPoint(value, fallback) {
    const x = Number(value[0]);
    const y = Number(value[1]);
    return [Number.isFinite(x) ? x : fallback[0], Number.isFinite(y) ? y : fallback[1]];
}
/** Require one record-valued host capability. */
function requireRecord(value, name) {
    if (!isRecord(value))
        throw new TypeError(`${name} is unavailable.`);
    return value;
}
/** Require one callable host capability. */
function requireFunction(value, name) {
    if (typeof value !== 'function')
        throw new TypeError(`${name} is unavailable.`);
    return value;
}
/** Adapt Comfy's EventTarget-shaped API without leaking its dynamic receiver inward. */
function readPreviewEventSource(api) {
    if (typeof api.addEventListener !== 'function' || typeof api.removeEventListener !== 'function') {
        return null;
    }
    const addEventListener = requireFunction(api.addEventListener, 'Comfy API.addEventListener');
    const removeEventListener = requireFunction(api.removeEventListener, 'Comfy API.removeEventListener');
    return {
        addEventListener(type, listener) {
            addEventListener.call(api, type, listener);
        },
        removeEventListener(type, listener) {
            removeEventListener.call(api, type, listener);
        },
    };
}
/** Require the host-owned selection set used by native graph conversion. */
function requireSet(value, name) {
    if (!(value instanceof Set))
        throw new TypeError(`${name} is unavailable.`);
    return value;
}
/** Read one finite positive host setting. */
function readPositiveNumber(value, fallback) {
    const number = Number(value);
    return Number.isFinite(number) && number > 0 ? number : fallback;
}
/** Read one non-empty external string. */
function readNonEmptyString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
/** Invoke one optional Comfy URL suffix provider. */
function callOptionalString(value, receiver) {
    if (typeof value !== 'function')
        return '';
    const result = value.call(receiver);
    return typeof result === 'string' ? result : '';
}
