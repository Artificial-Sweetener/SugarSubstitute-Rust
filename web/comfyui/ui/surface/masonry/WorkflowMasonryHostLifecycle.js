import { isRecord } from '../../types/common.js';
import { createComfyAppModeSource } from '../../core/ComfyAppMode.js';
import { createComfyRendererModeChangeSource, resolveComfyRendererMode, } from '../../core/ComfyRendererMode.js';
import { NativeSubgraphChangeObserver, } from '../NativeSubgraphChangeObserver.js';
import { WorkflowMasonryController } from './WorkflowMasonryController.js';
import { projectWorkflowMasonry, } from './WorkflowMasonryProjection.js';
import { ensureWorkflowMasonryStyles } from './WorkflowMasonryStyles.js';
import { WorkflowMasonryToolbarButton } from './WorkflowMasonryToolbarButton.js';
import { WorkflowMasonryPropertiesPanel } from './WorkflowMasonryPropertiesPanel.js';
import { captureMasonryLinkPaths } from './ComfyMasonryLinkAdapter.js';
import { WorkflowMasonryLinkMotion } from './WorkflowMasonryLinkMotion.js';
import { MasonrySearchHost } from '../../search/infrastructure/MasonrySearchHost.js';
import { CubeStackHost } from '../stack/CubeStackHost.js';
/** Compose focused owners and keep host registration free of layout behavior. */
export class WorkflowMasonryHostLifecycle {
    #options;
    #controller;
    #toolbar;
    #properties;
    #search;
    #stack;
    #graphs = new Map();
    #unsubscribeRenderer = null;
    #unsubscribeCards = null;
    #unsubscribeNavigation = null;
    #unsubscribeAppMode = null;
    #unsubscribePresentation = null;
    #appMode = false;
    #disposed = false;
    #presentationRevision = 0;
    /** Bind native renderer capabilities and non-blocking fallback feedback. */
    constructor(options) {
        this.#options = options;
        this.#search = new MasonrySearchHost({
            document: options.document,
            app: options.app,
            getGraph: () => options.app?.canvas?.graph ?? options.app?.graph ?? null,
            reportError: options.reportError,
        });
        this.#stack = new CubeStackHost({
            ...options.stack,
            document: options.document,
            app: options.app,
            refreshSurface: () => this.#controller.graphChanged(),
            revealSearchResults: () => this.#search.revealAll(),
            reportError: options.reportError,
        });
        this.#controller = new WorkflowMasonryController({
            document: options.document,
            getContainer: options.getContainer,
            getScope: () => {
                const manager = options.app?.extensionManager;
                const store = isRecord(manager) && isRecord(manager.workflow) ? manager.workflow : null;
                return store && isRecord(store.activeWorkflow) ? store.activeWorkflow : null;
            },
            isNodes2: () => this.#isNodes2(),
            getSections: () => {
                const graph = options.app?.canvas?.graph ?? options.app?.graph;
                return graph
                    ? projectWorkflowMasonry(graph, options.getCubes(), options.getConnections(graph), options.stack.getReorder()?.analysis ?? null)
                    : [];
            },
            getRenderer: options.getRenderer,
            createLinkMotion: (container) => {
                const layer = container.closest('#graph-canvas-container') ?? container;
                const snapshot = captureMasonryLinkPaths(options.app?.canvas, layer);
                if (!snapshot)
                    return null;
                try {
                    return new WorkflowMasonryLinkMotion(layer, snapshot);
                }
                catch (error) {
                    snapshot.restore();
                    throw error;
                }
            },
            getCardState: () => {
                const state = options.getCardState();
                if (!state)
                    throw new Error('Cube card presentation is unavailable.');
                return state;
            },
            openCube: (cube) => this.#openCube(cube),
            notifyRendererFallback: options.notifyRendererFallback,
            reportError: options.reportError,
            onStateChange: () => this.#refreshButton(),
            onSurfaceReady: (view, preparing) => {
                this.#search.ready(view);
                this.#stack.ready(view, preparing);
            },
            onSurfaceTransition: (entering) => this.#stack.transition(entering),
            onSurfaceLeaving: () => {
                this.#properties.update(this.#controller.active);
                this.#stack.release();
                this.#search.release();
            },
            onSurfaceValuesChanged: () => {
                this.#search.valuesChanged();
                this.#stack.valuesChanged();
            },
        });
        this.#toolbar = new WorkflowMasonryToolbarButton(options.document, () => this.toggle());
        this.#properties = new WorkflowMasonryPropertiesPanel(options.document, options.app, options.reportError);
    }
    /** Register the persistent affordance and renderer transition subscription after host setup. */
    setup() {
        if (this.#unsubscribeRenderer || this.#disposed)
            return;
        ensureWorkflowMasonryStyles(this.#options.document);
        const appMode = createComfyAppModeSource(this.#options.app);
        this.#onAppModeChanged(appMode.active);
        this.#unsubscribeAppMode = appMode.subscribe(this.#onAppModeChanged);
        this.#unsubscribeRenderer = createComfyRendererModeChangeSource(this.#options.app).subscribe(() => {
            this.#controller.rendererChanged();
            this.#restorePresentation();
        });
        this.#toolbar.setup();
        this.#properties.setup();
        this.#search.setup();
        this.#stack.setup();
        this.#unsubscribePresentation = this.#options.presentation.subscribe(this.#restorePresentation);
        this.#restorePresentation();
        this.#refreshButton();
        this.#options.document.defaultView?.addEventListener('pagehide', this.#onPageHide);
    }
    /** Serve both the button and the registered Comfy command through the same use case. */
    toggle() {
        if (this.#disposed)
            return;
        if (!this.#isNodes2() || !this.#options.presentation.ready || this.#appMode)
            return;
        const desired = !this.#options.presentation.snapshot.masonry;
        const transition = this.#controller.toggle();
        this.#options.presentation.change({ masonry: desired });
        void transition.catch((error) => this.#options.reportError(error));
    }
    /** Refresh an accepted canonical stack mutation through the existing projected layout owner. */
    refresh() {
        return this.#controller.graphChanged();
    }
    /** Release native mounts before Comfy replaces a workflow's graph objects. */
    beforeConfigureGraph() {
        void this.#controller
            .setSuspended('configure', true)
            .catch((error) => this.#options.reportError(error));
        this.#releaseGraphObservers();
    }
    /** Restore the selected presentation against the newly configured workflow. */
    afterConfigureGraph() {
        if (!this.#options.presentation.ready)
            return;
        void this.#controller
            .setSuspended('configure', false)
            .catch((error) => this.#options.reportError(error));
    }
    /** Release all owned DOM, observers, subscriptions, renderer mounts, and animations. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#controller.leaveImmediately();
        this.#toolbar.dispose();
        this.#properties.dispose();
        this.#search.dispose();
        this.#stack.dispose();
        this.#unsubscribeRenderer?.();
        this.#unsubscribeRenderer = null;
        this.#unsubscribeAppMode?.();
        this.#unsubscribeAppMode = null;
        this.#unsubscribePresentation?.();
        this.#unsubscribePresentation = null;
        this.#releaseGraphObservers();
        this.#options.document.defaultView?.removeEventListener('pagehide', this.#onPageHide);
    }
    /** Tear down before the browser releases the Comfy Vue application. */
    #onPageHide = () => this.dispose();
    /** Resume once at committed activation, restoring choices without entrance motion. */
    #restorePresentation = () => {
        const presentation = this.#options.presentation;
        if (this.#disposed)
            return;
        const revision = ++this.#presentationRevision;
        void (async () => {
            if (!presentation.ready) {
                await this.#controller.setSuspended('configure', true);
                return;
            }
            await this.#controller.restore(presentation.snapshot.masonry && this.#isNodes2());
            if (this.#disposed || revision !== this.#presentationRevision)
                return;
            await this.#controller.setSuspended('configure', false);
        })().catch(this.#options.reportError);
    };
    /** Suspend borrowed native controls before Comfy hides the graph, preserving the view choice. */
    #onAppModeChanged = (active) => {
        if (this.#disposed)
            return;
        this.#appMode = active;
        void this.#controller
            .setSuspended('app', active)
            .catch((error) => this.#options.reportError(error));
        this.#refreshButton();
    };
    /** Read the authoritative Comfy setting rather than inferring from stale graph DOM. */
    #isNodes2() {
        return resolveComfyRendererMode(this.#options.app, null, this.#options.document) === 'vue';
    }
    /** Reflect transient presentation and subscribe only while masonry needs graph updates. */
    #refreshButton() {
        const active = this.#controller.active;
        this.#toolbar.update(active, this.#isNodes2() && !this.#appMode);
        if (!this.#properties.update(this.#controller.presenting)) {
            this.#controller.leaveImmediately();
            return;
        }
        if (active)
            this.#observeGraphs();
        else
            this.#releaseGraphObservers();
    }
    /** Observe topology through the existing reversible native graph adapter. */
    #observeGraphs() {
        this.#unsubscribeCards ??=
            this.#options.getCardState()?.subscribe(this.#onCardsChanged) ?? null;
        this.#unsubscribeNavigation ??=
            this.#options.getGraphChanges()?.subscribe(this.#onGraphReplaced) ?? null;
        const graph = this.#options.app?.canvas?.graph ?? this.#options.app?.graph;
        const candidates = [graph, ...this.#options.getCubes().map((cube) => cube.subgraph)];
        const graphs = new Set(candidates.filter(isObservableGraph));
        for (const [observed, observer] of this.#graphs) {
            if (graphs.has(observed))
                continue;
            observer.dispose();
            this.#graphs.delete(observed);
        }
        for (const candidate of graphs) {
            if (this.#graphs.has(candidate))
                continue;
            this.#graphs.set(candidate, new NativeSubgraphChangeObserver(candidate, () => {
                if (this.#disposed)
                    return;
                void this.#controller
                    .graphChanged()
                    .catch((error) => this.#options.reportError(error));
            }));
        }
    }
    /** Restore every host callback before changing renderer or graph ownership. */
    #releaseGraphObservers() {
        this.#unsubscribeNavigation?.();
        this.#unsubscribeNavigation = null;
        this.#unsubscribeCards?.();
        this.#unsubscribeCards = null;
        for (const observer of this.#graphs.values())
            observer.dispose();
        this.#graphs.clear();
    }
    /** Refresh through the controller when shared card state changes in either presentation. */
    #onCardsChanged = () => {
        if (this.#disposed)
            return;
        void this.#controller
            .graphChanged()
            .catch((error) => this.#options.reportError(error));
    };
    /** Follow the shared native navigation boundary rather than retaining a previous graph's cards. */
    #onGraphReplaced = () => {
        if (this.#disposed)
            return;
        void this.#controller
            .graphReplaced()
            .catch((error) => this.#options.reportError(error));
    };
    /** Enter the native Cube editor through Comfy's validated host API. */
    #openCube(cube) {
        const canvas = this.#options.app?.canvas;
        const openSubgraph = canvas?.openSubgraph;
        if (typeof openSubgraph !== 'function') {
            this.#options.reportError(new Error('Comfy Cube editor navigation is unavailable.'));
            return;
        }
        this.#options.presentation.change({ masonry: false });
        openSubgraph.call(canvas, cube.subgraph, cube);
    }
}
/** Narrow dynamic host graph methods before passing them to the native observer adapter. */
function isObservableGraph(value) {
    return (isRecord(value) &&
        typeof value.add === 'function' &&
        typeof value.remove === 'function' &&
        (value.onAfterChange === undefined ||
            value.onAfterChange === null ||
            typeof value.onAfterChange === 'function'));
}
