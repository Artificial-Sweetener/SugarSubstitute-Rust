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
import { loadComfyNativeNodeComponent, loadComfyVueRuntime, } from './ComfyRuntimeModuleLoader.js';
import { ComfyVueNodeCardRenderer } from './ComfyVueNodeCardRenderer.js';
import { findComfyNativeNodeMount, findComfyVueAppContext } from './ComfyVueTree.js';
import { ComfyVueCubeNodeHost } from './ComfyVueCubeNodeHost.js';
import { ensureCubeSurfaceStyles } from './CubeSurfaceStyles.js';
import { CubeSurfaceView } from './CubeSurfaceView.js';
import { parseCubeSurfaceState, serializeCubeSurfaceState } from './CubeSurfaceState.js';
import { NativeSubgraphChangeObserver } from './NativeSubgraphChangeObserver.js';
import { ComfyLiteGraphCubeNodeHost, } from './ComfyLiteGraphCubeNodeHost.js';
import { CUBE_INPUT_GUTTER_WIDTH, CUBE_OUTPUT_GUTTER_WIDTH } from './CubePortGutterLayout.js';
import { ComfyRendererPresenceObserver } from './ComfyRendererPresenceObserver.js';
import { ComfyNativeSlotLayoutCoordinator } from './ComfyNativeSlotLayoutCoordinator.js';
import { CubeRendererTransitionStabilizer } from './CubeRendererTransitionStabilizer.js';
/** Own custom Cube-node face mounts across renderer and graph navigation changes. */
export class CubeSurfacePresenter {
    #document;
    #openEditor;
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
    #views = new Map();
    #observers = new Map();
    #renderer;
    #runtime = null;
    #unsubscribeNodes;
    #unsubscribePreviews;
    #unsubscribeGraphChanges;
    #nodePresenceObserver;
    #slotLayoutCoordinator;
    #rendererStabilizer;
    #mountGenerations = new Map();
    #presentedRendererMode = null;
    /** Bind native Cube nodes to their Nodes 2.0 custom-content seam. */
    constructor(options) {
        this.#document = options.document;
        this.#openEditor = options.openEditor;
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
            openEditor: (node) => this.#beginEditing(node),
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
        this.#renderer = options.renderer ? Promise.resolve(options.renderer) : null;
        this.#unsubscribeNodes = options.nodes.subscribe(() => this.#stabilizeSync());
        this.#unsubscribePreviews =
            options.previewChanges?.subscribe(() => this.#refreshPreviews()) ?? (() => undefined);
        this.#unsubscribeGraphChanges =
            options.graphChanges?.subscribe(() => this.#stabilizeSync()) ?? (() => undefined);
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
        this.#nodePresenceObserver.dispose();
        this.#slotLayoutCoordinator.dispose();
        this.#rendererStabilizer.dispose();
        this.#mountGenerations.clear();
        for (const surface of this.#views.values())
            surface.view.dispose();
        this.#views.clear();
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
        if (rendererMode !== this.#presentedRendererMode) {
            this.#presentedRendererMode = rendererMode;
            this.#logger.debug('SugarCubes reconciled Cube renderer presentation.', {
                rendererMode,
                atRoot,
                nodeCount: nodes.size,
            });
        }
        const shouldPresentVue = rendererMode === 'vue' && atRoot;
        const shouldPresentLegacy = rendererMode === 'litegraph' && atRoot;
        this.#legacyHost?.setEnabled(shouldPresentLegacy);
        this.#legacyHost?.sync();
        if (!shouldPresentVue) {
            for (const node of [...this.#views.keys()])
                this.#unmount(node);
            return;
        }
        for (const node of nodes) {
            const root = this.#host.mount(node);
            if (!root) {
                if (this.#views.has(node))
                    this.#unmount(node);
                continue;
            }
            const surface = this.#views.get(node);
            if (!surface || surface.root !== root) {
                void this.#mountView(node, root);
                continue;
            }
            const signature = buildTopologySignature(node);
            if (surface.topologySignature !== signature) {
                void this.#mountView(node, root);
                continue;
            }
            this.#layoutMountedView(node);
        }
    }
    /** Mount exact internal nodes using Comfy's active native component. */
    async #mountView(node, root) {
        const generation = (this.#mountGenerations.get(node) ?? 0) + 1;
        this.#mountGenerations.set(node, generation);
        try {
            const renderer = await this.#getRenderer();
            if (generation !== this.#mountGenerations.get(node) ||
                !this.#nodes.list().includes(node) ||
                this.#getCurrentGraph() !== this.#rootGraph) {
                return;
            }
            const surfaceState = requireCubeSurface(node);
            const state = parseCubeSurfaceState(surfaceState);
            const view = new CubeSurfaceView({
                document: this.#document,
                renderer,
                identity: resolveCubeIdentityPresentation({
                    metadata: requireCubeIdentity(node),
                    instanceTitle: node.title?.trim() || node.subgraph.name,
                    fallbackDefinitionTitle: node.subgraph.name,
                }),
                metadata: requireCubeIdentity(node),
                chromeActions: this.#chromeActions,
                nodes: node.subgraph._nodes,
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
            this.#views.get(node)?.view.dispose();
            root.replaceChildren(view.element);
            if (!this.#host.mountHeader(node, view.header)) {
                throw new Error('Comfy native Cube header is not mounted.');
            }
            this.#views.set(node, {
                root,
                topologySignature: buildTopologySignature(node),
                view,
                layoutWidth: Number.NaN,
            });
            this.#observers.get(node)?.dispose();
            this.#observers.set(node, new NativeSubgraphChangeObserver(node.subgraph, () => this.#sync()));
            this.#layoutMountedView(node);
            if (this.#previewCatalog)
                view.renderPreview(this.#previewCatalog.snapshot(node));
            this.#host.reconcileBoundary(node);
            this.#onBoundaryGeometryChange();
        }
        catch (error) {
            this.#logger.error('SugarCubes failed to mount a Cube node surface.', {
                nodeId: node.id,
                reason: error instanceof Error ? error.message : String(error),
                error,
            });
        }
    }
    /** Reflow one mounted Cube only when its usable width changes. */
    #layoutMountedView(node) {
        const surface = this.#views.get(node);
        if (!surface)
            return;
        surface.view.setPortGutterWidths(node.inputs.length > 0 ? CUBE_INPUT_GUTTER_WIDTH : 0, node.outputs.length > 0 ? CUBE_OUTPUT_GUTTER_WIDTH : 0);
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
            surface.view.renderPreview(this.#previewCatalog.snapshot(node));
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
        this.#renderer ??= this.#loadRenderer();
        return this.#renderer;
    }
    /** Resolve Comfy's slot-layout owner once for rendering and boundary remeasurement. */
    #getRuntime() {
        this.#runtime ??= loadComfyVueRuntime(this.#document);
        return this.#runtime;
    }
}
/** Describe exact internal node identity without projecting it onto the root graph. */
function buildTopologySignature(node) {
    return JSON.stringify(node.subgraph._nodes.map((innerNode) => [String(innerNode.id ?? ''), innerNode.type ?? '']));
}
/** Replace persisted surface state while retaining domain ownership of the record. */
function replaceRecord(target, source) {
    for (const key of Object.keys(target))
        Reflect.deleteProperty(target, key);
    Object.assign(target, source);
}
/** Exclude dedicated port gutters from responsive card and preview width. */
function resolveVueContentWidth(node) {
    const inputGutterWidth = node.inputs.length > 0 ? CUBE_INPUT_GUTTER_WIDTH : 0;
    const outputGutterWidth = node.outputs.length > 0 ? CUBE_OUTPUT_GUTTER_WIDTH : 0;
    return Math.max(1, Number(node.size[0]) - 16 - inputGutterWidth - outputGutterWidth);
}
