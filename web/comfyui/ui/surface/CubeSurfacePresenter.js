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
/** Present Cube faces inside their real native graph nodes. */
import { requireCubeIdentity, requireCubeSurface, } from '../cube/node/ComfyCubeNodeFactory.js';
import { resolveCubeIdentityPresentation } from '../cube/CubeIdentityPresentation.js';
import { buildCubeFaceChromeMetadata } from '../cube/node/CubeNodeAuthoringCandidate.js';
import { loadComfyNativeNodeComponent, loadComfyVueRuntime, } from './ComfyRuntimeModuleLoader.js';
import { ComfyVueNodeCardRenderer } from './ComfyVueNodeCardRenderer.js';
import { findComfyNativeNodeMount, findComfyVueAppContext } from './ComfyVueTree.js';
import { ComfyVueCubeNodeHost } from './ComfyVueCubeNodeHost.js';
import { ensureCubeSurfaceStyles } from './CubeSurfaceStyles.js';
import { CubeSurfaceView } from './CubeSurfaceView.js';
import { parseCubeSurfaceState, serializeCubeSurfaceState } from './CubeSurfaceState.js';
import { NativeSubgraphChangeObserver } from './NativeSubgraphChangeObserver.js';
import { ComfyLiteGraphCubeNodeHost, } from './ComfyLiteGraphCubeNodeHost.js';
import { CUBE_INPUT_GUTTER_WIDTH } from './CubePortGutterLayout.js';
import { ComfyRendererPresenceObserver } from './ComfyRendererPresenceObserver.js';
import { ComfyNativeSlotLayoutCoordinator } from './ComfyNativeSlotLayoutCoordinator.js';
import { CubeRendererTransitionStabilizer } from './CubeRendererTransitionStabilizer.js';
import { resolveCubeExternalInterface } from '../cube/graph/CubeExternalInterface.js';
import { filterCubePreviewOutputs } from './CubePreviewModel.js';
/** Own custom Cube-node face mounts across renderer and graph navigation changes. */
export class CubeSurfacePresenter {
    #document;
    #openEditor;
    #prepareEditor;
    #rootGraph;
    #getCurrentGraph;
    #nodes;
    #setDirtyCanvas;
    #logger;
    #previewCatalog;
    #getRendererMode;
    #host;
    #legacyHost;
    #chromeActions;
    #onBoundaryGeometryChange;
    #createRenderer;
    #views = new Map();
    #observers = new Map();
    #renderer;
    #runtime = null;
    #unsubscribeNodes;
    #unsubscribePreviews;
    #unsubscribeGraphChanges;
    #unsubscribeRendererChanges;
    #nodePresenceObserver;
    #slotLayoutCoordinator;
    #rendererStabilizer;
    #mountGenerations = new Map();
    #mountsInFlight = new Set();
    #mountFailureSignatures = new Map();
    #presentedRendererMode = null;
    /** Bind native Cube nodes to their Nodes 2.0 custom-content seam. */
    constructor(options) {
        this.#document = options.document;
        this.#openEditor = options.openEditor;
        this.#prepareEditor = options.prepareEditor ?? (() => undefined);
        this.#rootGraph = options.rootGraph;
        this.#getCurrentGraph = options.getCurrentGraph;
        this.#nodes = options.nodes;
        this.#setDirtyCanvas = options.setDirtyCanvas ?? (() => undefined);
        this.#logger = options.logger;
        this.#previewCatalog = options.previewCatalog ?? null;
        this.#getRendererMode = options.getRendererMode ?? (() => 'vue');
        this.#chromeActions = options.chromeActions ?? null;
        this.#onBoundaryGeometryChange = options.onBoundaryGeometryChange ?? (() => undefined);
        const windowRef = options.document.defaultView;
        this.#rendererStabilizer = new CubeRendererTransitionStabilizer({
            requestFrame: (callback) => windowRef?.requestAnimationFrame(callback) ?? null,
            cancelFrame: (handle) => windowRef?.cancelAnimationFrame(handle),
        });
        this.#slotLayoutCoordinator = new ComfyNativeSlotLayoutCoordinator({
            getRuntime: () => this.#getRuntime(),
            requestFrame: (callback) => windowRef?.requestAnimationFrame(callback) ?? null,
            cancelFrame: (handle) => windowRef?.cancelAnimationFrame(handle),
            logger: options.logger,
        });
        this.#host = new ComfyVueCubeNodeHost({
            document: options.document,
            titleHeight: options.titleHeight ?? 30,
            history: options.history ?? {},
            getScale: () => Number(options.legacyCanvas?.ds?.scale) || 1,
            prepareEditor: (node) => this.#prepareEditor(node),
            onGeometryChange: (node) => this.#handleNodeGeometryChange(node),
            requestSlotLayoutSync: options.requestSlotLayoutSync ?? (() => this.#slotLayoutCoordinator.request()),
            ...(options.portPresentation ? { portPresentation: options.portPresentation } : {}),
        });
        this.#legacyHost = options.legacyCanvas
            ? new ComfyLiteGraphCubeNodeHost({
                canvas: options.legacyCanvas,
                document: options.document,
                rootGraph: options.rootGraph,
                nodes: options.nodes,
                history: options.history ?? {},
                titleHeight: options.titleHeight ?? 30,
                openEditor: (node) => this.#beginEditing(node),
                logger: options.logger,
                ...(options.previewCatalog ? { previewCatalog: options.previewCatalog } : {}),
                ...(options.chromeActions ? { chromeActions: options.chromeActions } : {}),
                ...(options.portPresentation ? { portPresentation: options.portPresentation } : {}),
            })
            : null;
        const providedRenderer = options.renderer;
        this.#createRenderer =
            options.createRenderer ??
                (providedRenderer ? async () => providedRenderer : () => this.#loadRenderer());
        this.#renderer = null;
        this.#unsubscribeNodes = options.nodes.subscribe(() => this.#stabilizeSync());
        this.#unsubscribePreviews =
            options.previewChanges?.subscribe(() => this.#refreshPreviews()) ?? (() => undefined);
        this.#unsubscribeGraphChanges =
            options.graphChanges?.subscribe(() => this.#stabilizeSync()) ?? (() => undefined);
        this.#unsubscribeRendererChanges =
            options.rendererChanges?.subscribe(() => this.#stabilizeSync()) ?? (() => undefined);
        this.#nodePresenceObserver = new ComfyRendererPresenceObserver({
            document: options.document,
            ownsNodeId: (nodeId) => this.#nodes.list().some((node) => String(node.id) === nodeId),
            onPresenceChange: () => this.#stabilizeSync(),
        });
        ensureCubeSurfaceStyles(options.document);
        this.#stabilizeSync();
    }
    /** Dispose all native card mounts and host observers. */
    dispose() {
        this.#unsubscribeNodes();
        this.#unsubscribePreviews();
        this.#unsubscribeGraphChanges();
        this.#unsubscribeRendererChanges();
        this.#nodePresenceObserver.dispose();
        this.#slotLayoutCoordinator.dispose();
        this.#rendererStabilizer.dispose();
        this.#mountGenerations.clear();
        this.#mountsInFlight.clear();
        this.#mountFailureSignatures.clear();
        for (const surface of this.#views.values())
            surface.view.dispose();
        this.#views.clear();
        this.#releaseRenderer();
        for (const observer of this.#observers.values())
            observer.dispose();
        this.#observers.clear();
        this.#legacyHost?.dispose();
        this.#host.dispose();
    }
    /** Reconcile presentation after an explicit host geometry or lifecycle event. */
    refresh() {
        this.#stabilizeSync();
    }
    /** Reconcile now and across Comfy's short renderer hook replacement window. */
    #stabilizeSync() {
        this.#sync();
        this.#rendererStabilizer.run(() => this.#sync());
    }
    /** Keep presentation aligned with renderer mode, graph navigation, and collection state. */
    #sync() {
        const nodes = new Set(this.#nodes.list());
        for (const node of [...this.#views.keys()]) {
            if (!nodes.has(node))
                this.#unmount(node);
        }
        const atRoot = this.#getCurrentGraph() === this.#rootGraph;
        const rendererMode = this.#getRendererMode();
        const previousRendererMode = this.#presentedRendererMode;
        const rendererChanged = rendererMode !== previousRendererMode;
        if (rendererChanged) {
            this.#presentedRendererMode = rendererMode;
            this.#logger.debug('SugarCubes reconciled Cube renderer presentation.', {
                rendererMode,
                atRoot,
                nodeCount: nodes.size,
            });
        }
        const shouldPresentVue = rendererMode === 'vue' && atRoot;
        const shouldPresentLegacy = rendererMode === 'litegraph' && atRoot;
        if (!shouldPresentVue) {
            for (const node of [...this.#views.keys()])
                this.#unmount(node);
        }
        if (rendererChanged && previousRendererMode === 'vue')
            this.#releaseRenderer();
        this.#legacyHost?.setEnabled(shouldPresentLegacy);
        this.#legacyHost?.sync();
        if (!shouldPresentVue)
            return;
        for (const node of nodes) {
            const surface = this.#views.get(node);
            const root = this.#host.getRoot(node);
            const failureSignature = buildMountAttemptSignature(node);
            if (!surface || !root || surface.root !== root) {
                if (surface)
                    this.#unmount(node);
                if (this.#mountFailureSignatures.get(node) !== failureSignature) {
                    void this.#mountView(node);
                }
                continue;
            }
            const signature = buildTopologySignature(node);
            const presentationSignature = buildPresentationSignature(node);
            if (surface.topologySignature !== signature ||
                surface.presentationSignature !== presentationSignature) {
                if (this.#mountFailureSignatures.get(node) !== failureSignature) {
                    void this.#mountView(node);
                }
                continue;
            }
            this.#layoutMountedView(node);
            this.#host.reconcileBoundary(node);
        }
    }
    /** Mount exact internal nodes using Comfy's active native component. */
    async #mountView(node) {
        if (this.#mountsInFlight.has(node))
            return;
        this.#mountsInFlight.add(node);
        const generation = (this.#mountGenerations.get(node) ?? 0) + 1;
        this.#mountGenerations.set(node, generation);
        let pendingView = null;
        try {
            const renderer = await this.#getRenderer();
            if (generation !== this.#mountGenerations.get(node) ||
                !this.#nodes.list().includes(node) ||
                this.#getCurrentGraph() !== this.#rootGraph ||
                this.#getRendererMode() !== 'vue') {
                return;
            }
            const surfaceState = requireCubeSurface(node);
            const state = parseCubeSurfaceState(surfaceState);
            pendingView = new CubeSurfaceView({
                document: this.#document,
                renderer,
                identity: resolveCubeIdentityPresentation({
                    metadata: requireCubeIdentity(node),
                    instanceTitle: node.title?.trim() || node.subgraph.name,
                    fallbackDefinitionTitle: node.subgraph.name,
                }),
                metadata: buildCubeFaceChromeMetadata(node),
                chromeActions: this.#chromeActions,
                nodes: node.subgraph._nodes,
                graph: node.subgraph,
                state,
                onStateChange: (nextState) => {
                    replaceRecord(surfaceState, serializeCubeSurfaceState(nextState));
                    this.#nodes.changed(node);
                    this.#setDirtyCanvas(true, true);
                },
                onMinimumHeightChange: (minimumHeight) => {
                    if (!this.#host.reconcileMinimumSize(node, minimumHeight))
                        return;
                    this.#nodes.changed(node);
                    this.#setDirtyCanvas(true, true);
                },
            });
            const root = this.#host.getRoot(node) ?? this.#host.mount(node);
            if (!root) {
                pendingView.dispose();
                pendingView = null;
                return;
            }
            this.#views.get(node)?.view.dispose();
            root.replaceChildren(pendingView.element);
            if (!this.#host.mountHeader(node, pendingView.header)) {
                throw new Error('Comfy native Cube header is not mounted.');
            }
            this.#views.set(node, {
                root,
                topologySignature: buildTopologySignature(node),
                presentationSignature: buildPresentationSignature(node),
                view: pendingView,
                layoutWidth: Number.NaN,
            });
            this.#observers.get(node)?.dispose();
            this.#observers.set(node, new NativeSubgraphChangeObserver(node.subgraph, () => this.#sync()));
            this.#layoutMountedView(node);
            if (this.#previewCatalog) {
                const externalInterface = resolveCubeExternalInterface(node);
                pendingView.renderPreview(filterCubePreviewOutputs(this.#previewCatalog.snapshot(node), externalInterface.outputSlots));
            }
            this.#host.reconcileBoundary(node);
            this.#onBoundaryGeometryChange();
            this.#mountFailureSignatures.delete(node);
            pendingView = null;
        }
        catch (error) {
            const mounted = this.#views.get(node);
            if (pendingView && mounted?.view === pendingView) {
                this.#views.delete(node);
                this.#observers.get(node)?.dispose();
                this.#observers.delete(node);
            }
            pendingView?.dispose();
            this.#host.unmount(node);
            const failureSignature = buildMountAttemptSignature(node);
            if (this.#mountFailureSignatures.get(node) !== failureSignature) {
                this.#mountFailureSignatures.set(node, failureSignature);
                const reason = error instanceof Error ? error.message : String(error);
                this.#logger.error(`SugarCubes failed to mount a Cube node surface: ${reason}`, {
                    nodeId: node.id,
                    reason,
                    error,
                });
            }
        }
        finally {
            this.#mountsInFlight.delete(node);
        }
    }
    /** Reflow one mounted Cube only when its usable width changes. */
    #layoutMountedView(node) {
        const surface = this.#views.get(node);
        if (!surface)
            return;
        const externalInterface = resolveCubeExternalInterface(node);
        surface.view.setPortGutterWidths(externalInterface.inputSlots.length > 0 ? CUBE_INPUT_GUTTER_WIDTH : 0, 0);
        surface.view.setPreviewAvailable(externalInterface.outputSlots.length > 0);
        const width = resolveVueContentWidth(node);
        if (Number.isFinite(surface.layoutWidth) && Math.abs(surface.layoutWidth - width) < 0.5) {
            return;
        }
        surface.layoutWidth = width;
        surface.view.layout(width);
    }
    /** Reconcile responsive face content and its dependent boundary anchors together. */
    #handleNodeGeometryChange(node) {
        this.#layoutMountedView(node);
        this.#host.reconcileBoundary(node);
        this.#onBoundaryGeometryChange();
    }
    /** Refresh media only when the execution-output owner reports a change. */
    #refreshPreviews() {
        if (!this.#previewCatalog)
            return;
        for (const [node, surface] of this.#views) {
            const externalInterface = resolveCubeExternalInterface(node);
            surface.view.renderPreview(filterCubePreviewOutputs(this.#previewCatalog.snapshot(node), externalInterface.outputSlots));
            this.#host.reconcileBoundary(node);
            this.#onBoundaryGeometryChange();
        }
    }
    /** Remove one mounted face without touching its native definition. */
    #unmount(node) {
        this.#views.get(node)?.view.dispose();
        this.#views.delete(node);
        this.#observers.get(node)?.dispose();
        this.#observers.delete(node);
        this.#mountGenerations.delete(node);
        this.#mountFailureSignatures.delete(node);
        this.#host.unmount(node);
    }
    /** Remove every native face mount before Comfy activates the same internal nodes. */
    #beginEditing(node) {
        this.#legacyHost?.setEnabled(false);
        for (const mountedNode of [...this.#views.keys()])
            this.#unmount(mountedNode);
        try {
            this.#openEditor(node);
        }
        catch (error) {
            this.#sync();
            throw error;
        }
    }
    /** Resolve the actual Comfy Nodes 2.0 component and Vue runtime. */
    async #loadRenderer() {
        const vueRoot = this.#document.querySelector('#vue-app');
        if (!vueRoot)
            throw new Error('Comfy Vue application root is not mounted.');
        const appContext = findComfyVueAppContext(vueRoot);
        if (!appContext)
            throw new Error('Comfy Vue application context is not mounted.');
        const mountedComponent = findComfyNativeNodeMount(vueRoot)?.component;
        const [component, runtime] = await Promise.all([
            mountedComponent
                ? Promise.resolve(mountedComponent)
                : loadComfyNativeNodeComponent(this.#document),
            this.#getRuntime(),
        ]);
        return new ComfyVueNodeCardRenderer({
            component,
            appContext,
            runtime,
        });
    }
    /** Lazily resolve Comfy's Vue renderer only while Nodes 2.0 is active. */
    #getRenderer() {
        this.#renderer ??= this.#createRenderer();
        return this.#renderer;
    }
    /** Resolve Comfy's slot-layout owner once for rendering and boundary remeasurement. */
    #getRuntime() {
        this.#runtime ??= loadComfyVueRuntime(this.#document);
        return this.#runtime;
    }
    /** Drop renderer state tied to the Vue graph application that Comfy replaced. */
    #releaseRenderer() {
        const renderer = this.#renderer;
        this.#renderer = null;
        this.#runtime = null;
        this.#mountFailureSignatures.clear();
        if (!renderer)
            return;
        void renderer
            .then((resolved) => resolved.dispose())
            .catch((error) => {
            const reason = error instanceof Error ? error.message : String(error);
            this.#logger.warn(`SugarCubes failed to dispose a replaced Nodes 2 renderer: ${reason}`, {
                reason,
                error,
            });
        });
    }
}
/** Describe exact internal node identity without projecting it onto the root graph. */
function buildTopologySignature(node) {
    return JSON.stringify(node.subgraph._nodes.map((innerNode) => [String(innerNode.id ?? ''), innerNode.type ?? '']));
}
/** Detect titlebar identity and persistence changes without remounting for unrelated graph state. */
function buildPresentationSignature(node) {
    const identity = requireCubeIdentity(node);
    return JSON.stringify([
        node.title ?? '',
        node.subgraph.name,
        identity.cube_id ?? '',
        identity.default_alias ?? '',
        identity.cube_version ?? '',
        identity.has_saveable_changes ?? false,
        identity.icon ?? null,
    ]);
}
/** Retry a failed mount only after its renderer-relevant node contract changes. */
function buildMountAttemptSignature(node) {
    return `${buildTopologySignature(node)}|${buildPresentationSignature(node)}`;
}
/** Replace persisted surface state while retaining domain ownership of the record. */
function replaceRecord(target, source) {
    for (const key of Object.keys(target))
        Reflect.deleteProperty(target, key);
    Object.assign(target, source);
}
/** Exclude only input labels because output slots overlay the preview's right rail. */
function resolveVueContentWidth(node) {
    const inputGutterWidth = node.inputs.length > 0 ? CUBE_INPUT_GUTTER_WIDTH : 0;
    return Math.max(1, Number(node.size[0]) - 16 - inputGutterWidth);
}
