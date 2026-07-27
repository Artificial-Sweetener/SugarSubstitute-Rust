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
import type { UnknownRecord } from '../types/common.js';
import {
  ComfyCubeAuthoringAdapter,
  type CubeAuthoringCanvas,
} from './ComfyCubeAuthoringAdapter.js';
import { ComfyCubeGraphBuilder } from './ComfyCubeGraphBuilder.js';
import type {
  CubeGraphBuilderHost,
  NativeCubeSubgraph,
  NativeGraphNode,
} from './ComfyCubeGraphBuilder.js';
import { CubePlacementService } from './CubePlacementService.js';
import { CubeSubgraphRegistrar } from './CubeSubgraphRegistrar.js';
import { CubeOutputSurfaceSynchronizer } from './CubeOutputSurfaceSynchronizer.js';
import type { ImportPayload } from '../import/PlacementPayload.js';
import { LegacyCubeDefinitionSerializer } from './migration/LegacyCubeDefinitionSerializer.js';
import { LegacyCubeGraphBuilder } from './migration/LegacyCubeGraphBuilder.js';
import {
  LegacyCubeMigrationCoordinator,
  type ConnectableNode,
} from './migration/LegacyCubeMigrationCoordinator.js';
import type { LegacyCubeMigrationResult } from './migration/LegacyCubeMigrationCoordinator.js';
import type { LegacyCubeMigrationBatch } from './migration/LegacyCubeWorkflowExtractor.js';
import { CubeSurfacePresenter } from '../surface/CubeSurfacePresenter.js';
import { ComfyCubePreviewCatalog } from '../surface/ComfyCubePreviewCatalog.js';
import type { CubePreviewLink } from '../surface/ComfyCubePreviewCatalog.js';
import type { CubePreviewRetentionStore } from '../surface/CubePreviewRetentionStore.js';
import { CubeEditorNavigationPresenter } from '../surface/CubeEditorNavigationPresenter.js';
import { ComfyCanvasViewStateAdapter } from '../surface/ComfyCanvasViewStateAdapter.js';
import { ComfyCanvasSelectionStateAdapter } from '../surface/ComfyCanvasSelectionStateAdapter.js';
import { createComfyCubeHistoryAdapter } from './ComfyCubeHistoryAdapter.js';
import { ComfyCubeNodeFactory } from './node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from './node/CubeNodeCatalog.js';
import { CubeNodeSwapCoordinator } from './node/CubeNodeSwapCoordinator.js';
import { ComfyCubeNodeLifecycleAdapter } from './node/ComfyCubeNodeLifecycleAdapter.js';
import type { LiteGraphCubeNodeCanvas } from '../surface/ComfyLiteGraphCubeNodeHost.js';
import { LegacyCubeContainerMigrationAdapter } from './migration/LegacyCubeContainerMigrationAdapter.js';
import { NativeCubeProximityEndpointSource } from './connection/NativeCubeProximityEndpointSource.js';
import type { ProximityEndpointSource } from '../overlays/proximity/ProximityModel.js';
import type { NativeSubgraphBoundaryResolver } from './graph/NativeSubgraphBoundaryResolver.js';
import type {
  CubeFaceChromeActions,
  CubeFaceChromeMetadata,
} from '../surface/CubeFaceChromeActions.js';
import { CubePortPresentationController } from './connection/CubePortPresentationController.js';
import type { ProximityMatchSink } from '../overlays/proximity/ProximityModel.js';
import { ComfyCanvasGraphChangeAdapter } from '../surface/ComfyCanvasGraphChangeAdapter.js';
import { resolveComfyRendererMode } from '../core/ComfyRendererMode.js';

export interface ComfyCubeRuntimeOptions {
  app: unknown;
  api: unknown;
  liteGraph: unknown;
  document: Document;
  logger: Pick<Console, 'debug' | 'error' | 'warn'>;
  getPreviewLinks?(): readonly CubePreviewLink[];
  getCubeOutput?(executionId: string): unknown;
  previewRetention?: CubePreviewRetentionStore;
  boundaryResolver: NativeSubgraphBoundaryResolver;
  openCubeMenu?(metadata: CubeFaceChromeMetadata, event: MouseEvent): void;
  subscribePreviewChanges?(listener: () => void): () => void;
  onBoundaryGeometryChange?(): void;
}

export interface ComfyCubeRuntime {
  placement: CubePlacementService;
  authoring: ComfyCubeAuthoringAdapter;
  nodes: CubeNodeCatalog;
  proximityEndpoints: ProximityEndpointSource;
  proximityPresentation: ProximityMatchSink;
  registerSubgraphs(payload: ImportPayload): string[];
  restoreLegacy(batch: LegacyCubeMigrationBatch | null): LegacyCubeMigrationResult;
  dispose(): void;
}

interface CubeRuntimeGraph {
  subgraphs: Map<string, NativeCubeSubgraph>;
  _nodes?: unknown[];
  onConfigure?: ((data: UnknownRecord) => void) | null;
  onSerialize?: ((data: UnknownRecord) => void) | null;
  onNodeAdded?: ((node: unknown) => void) | null;
  onNodeRemoved?: ((node: unknown) => void) | null;
  getNodeById(id: string | number): unknown;
}

/** Construct the graph-bound SugarCubes integration around native SubgraphNodes. */
export function createComfyCubeRuntime(options: ComfyCubeRuntimeOptions): ComfyCubeRuntime {
  const app = requireRecord(options.app, 'Comfy application');
  const api = requireRecord(options.api, 'Comfy API');
  const liteGraph = requireRecord(options.liteGraph, 'LiteGraph');
  const graph = requireRecord(app.graph, 'Comfy root graph');
  const canvas = requireRecord(app.canvas, 'Comfy canvas');
  const createNodeFunction = requireFunction(liteGraph.createNode, 'LiteGraph.createNode');
  const createSubgraphFunction = requireFunction(graph.createSubgraph, 'LGraph.createSubgraph');
  const convertToSubgraphFunction = requireFunction(
    graph.convertToSubgraph,
    'LGraph.convertToSubgraph',
  );
  const addFunction = requireFunction(graph.add, 'LGraph.add');
  const getNodeByIdFunction = requireFunction(graph.getNodeById, 'LGraph.getNodeById');
  const selectedItems = requireSet(canvas.selectedItems, 'LGraphCanvas.selectedItems');
  const updateSelectedItemsFunction =
    typeof canvas.updateSelectedItems === 'function' ? canvas.updateSelectedItems : null;
  const apiUrlFunction = requireFunction(api.apiURL, 'Comfy API.apiURL');
  const runtimeGraph = requireRuntimeGraph(graph);
  const legacyCanvas = requireLiteGraphCubeNodeCanvas(canvas);
  const titleHeight = readPositiveNumber(liteGraph.NODE_TITLE_HEIGHT, 30);

  const createNode = (type: string): NativeGraphNode | null => {
    const value = createNodeFunction.call(liteGraph, type);
    return isNativeGraphNode(value) ? value : null;
  };
  const graphBuilderHost: CubeGraphBuilderHost = {
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
  };

  const history = createComfyCubeHistoryAdapter({ canvas, graph });
  const nodes = new CubeNodeCatalog();
  const portPresentation = new CubePortPresentationController({
    requestFrame: (callback) =>
      options.document.defaultView?.requestAnimationFrame(callback) ?? null,
    invalidate: () => history.setDirtyCanvas?.(true, true),
  });
  const proximityEndpoints = new NativeCubeProximityEndpointSource(
    options.logger,
    options.boundaryResolver,
    portPresentation,
  );
  const nodeLifecycle = new ComfyCubeNodeLifecycleAdapter({
    graph: runtimeGraph,
    catalog: nodes,
    events: legacyCanvas.canvas,
    createInstanceId: createUuid,
    logger: options.logger,
  });
  const outputSurfaceSynchronizer = new CubeOutputSurfaceSynchronizer(nodes, legacyCanvas.canvas);
  const nodeFactory = new ComfyCubeNodeFactory({
    graph: {
      add(node) {
        addFunction.call(graph, node);
      },
    },
    createNode,
  });
  const nodeSwap = new CubeNodeSwapCoordinator({
    graph,
    nodes,
    history,
    setDirtyCanvas: (foreground, background) => history.setDirtyCanvas?.(foreground, background),
  });
  const chromeActions: CubeFaceChromeActions = {
    onSwapLeft: (metadata) => nodeSwap.swap(metadata, 'left'),
    onSwapRight: (metadata) => nodeSwap.swap(metadata, 'right'),
    canSwap: (metadata, direction) => nodeSwap.canSwap(metadata, direction),
    ...(options.openCubeMenu ? { onOpenMenu: options.openCubeMenu } : {}),
  };
  const legacyContainerMigration = new LegacyCubeContainerMigrationAdapter({
    graph: runtimeGraph,
    factory: nodeFactory,
    nodes,
    logger: options.logger,
    setDirtyCanvas: (foreground, background) => history.setDirtyCanvas?.(foreground, background),
  });
  const canvasView = new ComfyCanvasViewStateAdapter(
    canvas,
    (foreground, background) => history.setDirtyCanvas?.(foreground, background),
    options.logger,
  );
  const canvasSelection = new ComfyCanvasSelectionStateAdapter(
    selectedItems,
    updateSelectedItemsFunction ? () => updateSelectedItemsFunction.call(canvas) : null,
    options.logger,
  );
  const canvasGraphChanges = new ComfyCanvasGraphChangeAdapter(canvas);
  const editorNavigation = new CubeEditorNavigationPresenter({
    document: options.document,
    canvas: {
      setGraph(targetGraph) {
        canvasGraphChanges.setGraph(targetGraph);
      },
      captureView: () => canvasView.capture(),
      restoreView: (state) => canvasView.restore(state),
      captureSelection: () => canvasSelection.capture(),
      restoreSelection: (items) => canvasSelection.restore(items),
    },
    rootGraph: graph,
    getCurrentGraph: () => (isRecord(canvas.graph) ? canvas.graph : null),
    nodes,
    logger: options.logger,
    graphChanges: canvasGraphChanges,
  });
  const presenter = new CubeSurfacePresenter({
    document: options.document,
    openEditor: (node) => editorNavigation.open(node),
    rootGraph: graph,
    getCurrentGraph: () => (isRecord(canvas.graph) ? canvas.graph : null),
    nodes,
    setDirtyCanvas: (foreground, background) => history.setDirtyCanvas?.(foreground, background),
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
        if (!filename) return null;
        const params = new URLSearchParams();
        params.set('filename', filename);
        const subfolder = readNonEmptyString(image.subfolder);
        const type = readNonEmptyString(image.type);
        if (subfolder) params.set('subfolder', subfolder);
        if (type) params.set('type', type);
        const preview = callOptionalString(app.getPreviewFormatParam, app);
        const random = callOptionalString(app.getRandParam, app);
        const path = `/view?${params.toString()}${preview}${random}`;
        const value = apiUrlFunction.call(api, path);
        return typeof value === 'string' ? value : null;
      },
    }),
    getRendererMode: () => resolveComfyRendererMode(app, liteGraph, options.document),
    legacyCanvas,
    titleHeight,
    history,
    chromeActions,
    portPresentation,
    graphChanges: canvasGraphChanges,
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
  const placement = new CubePlacementService({
    graphBuilder,
    nodeFactory,
    catalog: nodes,
    history,
  });
  const authoring = new ComfyCubeAuthoringAdapter({
    graph,
    subgraphs: runtimeGraph.subgraphs,
    convertToSubgraph(items) {
      return convertToSubgraphFunction.call(graph, items);
    },
    canvas: {
      selectedItems,
      ...(updateSelectedItemsFunction
        ? { updateSelectedItems: () => updateSelectedItemsFunction.call(canvas) }
        : {}),
    } satisfies CubeAuthoringCanvas,
    nodeFactory,
    catalog: nodes,
  });
  const subgraphRegistrar = new CubeSubgraphRegistrar({
    hasSubgraph(id) {
      return runtimeGraph.subgraphs.has(id);
    },
    createSubgraph(data) {
      return graphBuilderHost.rootGraph.createSubgraph(data);
    },
    createNode,
  });
  const legacyMigration = new LegacyCubeMigrationCoordinator({
    graphBuilder: new LegacyCubeGraphBuilder(
      graphBuilderHost,
      new LegacyCubeDefinitionSerializer(createUuid),
    ),
    placement,
    graph: {
      getNodeById(id) {
        const value = getNodeByIdFunction.call(graph, id);
        return isConnectableNode(value) ? value : null;
      },
      ...(history.setDirtyCanvas
        ? {
            setDirtyCanvas: (foreground?: boolean, background?: boolean) =>
              history.setDirtyCanvas?.(foreground, background),
          }
        : {}),
    },
    logger: options.logger,
  });

  return {
    placement,
    authoring,
    nodes,
    proximityEndpoints,
    proximityPresentation: portPresentation,
    registerSubgraphs: (payload) => subgraphRegistrar.register(payload),
    restoreLegacy: (batch) => legacyMigration.restore(batch),
    dispose: () => {
      outputSurfaceSynchronizer.dispose();
      editorNavigation.dispose();
      presenter.dispose();
      legacyContainerMigration.dispose();
      nodeLifecycle.dispose();
      canvasGraphChanges.dispose();
    },
  };
}

/** Create a stable graph UUID without depending on Comfy's private helper. */
function createUuid(): string {
  if (typeof globalThis.crypto?.randomUUID === 'function') {
    return globalThis.crypto.randomUUID();
  }
  return `sugarcube-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}

/** Validate the node surface required by native Cube graph construction. */
function isNativeGraphNode(value: unknown): value is NativeGraphNode {
  if (!isRecord(value)) return false;
  value.properties = isRecord(value.properties) ? value.properties : {};
  return (
    isNumericVector(value.pos) &&
    isNumericVector(value.size) &&
    Array.isArray(value.inputs) &&
    Array.isArray(value.outputs) &&
    typeof value.connect === 'function'
  );
}

/** Validate the Comfy subgraph API used by Cube assembly and serialization. */
function isNativeCubeSubgraph(value: unknown): value is NativeCubeSubgraph {
  return (
    isRecord(value) &&
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
    typeof value.configure === 'function'
  );
}

/** Require Comfy's native definition map and lifecycle hook surface. */
function requireRuntimeGraph(graph: UnknownRecord): CubeRuntimeGraph {
  if (!(graph.subgraphs instanceof Map)) {
    throw new TypeError('LGraph.subgraphs is unavailable.');
  }
  return graph as unknown as CubeRuntimeGraph;
}

/** Require the exact host capabilities used by the Nodes 1.0 Cube node face. */
function requireLiteGraphCubeNodeCanvas(canvas: UnknownRecord): LiteGraphCubeNodeCanvas {
  if (
    !(canvas.canvas instanceof HTMLCanvasElement) ||
    typeof canvas.drawNode !== 'function' ||
    typeof canvas.processWidgetClick !== 'function' ||
    !isNumericVector(canvas.graph_mouse)
  ) {
    throw new TypeError('Comfy Nodes 1.0 Cube-node integration is unavailable.');
  }
  return canvas as unknown as LiteGraphCubeNodeCanvas;
}

/** Validate a native node capable of owning one restored graph connection. */
function isConnectableNode(value: unknown): value is ConnectableNode {
  return (
    isRecord(value) &&
    Array.isArray(value.inputs) &&
    Array.isArray(value.outputs) &&
    typeof value.connect === 'function'
  );
}

/** Accept LiteGraph's array and typed-array geometry vectors. */
function isNumericVector(value: unknown): value is number[] | Float32Array | Float64Array {
  return Array.isArray(value) || value instanceof Float32Array || value instanceof Float64Array;
}

/** Require one record-valued host capability. */
function requireRecord(value: unknown, name: string): UnknownRecord {
  if (!isRecord(value)) throw new TypeError(`${name} is unavailable.`);
  return value;
}

/** Require one callable host capability. */
function requireFunction(value: unknown, name: string): (...args: unknown[]) => unknown {
  if (typeof value !== 'function') throw new TypeError(`${name} is unavailable.`);
  return value as (...args: unknown[]) => unknown;
}

/** Require the host-owned selection set used by native graph conversion. */
function requireSet(value: unknown, name: string): Set<unknown> {
  if (!(value instanceof Set)) throw new TypeError(`${name} is unavailable.`);
  return value;
}

/** Read one finite positive host setting. */
function readPositiveNumber(value: unknown, fallback: number): number {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : fallback;
}

/** Read one non-empty external string. */
function readNonEmptyString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

/** Invoke one optional Comfy URL suffix provider. */
function callOptionalString(value: unknown, receiver: UnknownRecord): string {
  if (typeof value !== 'function') return '';
  const result = value.call(receiver);
  return typeof result === 'string' ? result : '';
}
