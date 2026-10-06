import { isRecord } from '../../types/common.js';
import { createComfyRendererModeChangeSource, resolveComfyRendererMode, } from '../../core/ComfyRendererMode.js';
import { CubeSurfaceRendererLifecycle } from '../CubeSurfaceRendererLifecycle.js';
import { NativeSubgraphChangeObserver, } from '../NativeSubgraphChangeObserver.js';
import { WorkflowMasonryController } from './WorkflowMasonryController.js';
import { projectWorkflowMasonry, } from './WorkflowMasonryProjection.js';
import { ensureWorkflowMasonryStyles } from './WorkflowMasonryStyles.js';
import { WorkflowMasonryToolbarButton } from './WorkflowMasonryToolbarButton.js';
/** Compose focused owners and keep host registration free of layout behavior. */
export class WorkflowMasonryHostLifecycle {
    #options;
    #controller;
    #renderers;
    #toolbar;
    #graphs = new Map();
    #unsubscribeRenderer = null;
    #unsubscribeCards = null;
    #unsubscribeNavigation = null;
    #restoreAfterConfigure = false;
    #disposed = false;
    /** Bind native renderer capabilities and non-blocking fallback feedback. */
    constructor(options) {
        this.#options = options;
        this.#renderers = new CubeSurfaceRendererLifecycle({
            document: options.document,
            logger: options.logger,
        });
        this.#controller = new WorkflowMasonryController({
            document: options.document,
            getContainer: () => options.document.querySelector('.graph-canvas-panel') ??
                options.document.getElementById('graph-canvas-container'),
            isNodes2: () => this.#isNodes2(),
            getSections: () => {
                const graph = options.app?.canvas?.graph ?? options.app?.graph;
                return graph
                    ? projectWorkflowMasonry(graph, options.getCubes(), options.getConnections(graph))
                    : [];
            },
            getRenderer: () => this.#renderers.getRenderer(),
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
        });
        this.#toolbar = new WorkflowMasonryToolbarButton(options.document, () => this.toggle());
    }
    /** Register the persistent affordance and renderer transition subscription after host setup. */
    setup() {
        if (this.#unsubscribeRenderer || this.#disposed)
            return;
        ensureWorkflowMasonryStyles(this.#options.document);
        this.#unsubscribeRenderer = createComfyRendererModeChangeSource(this.#options.app).subscribe(() => {
            this.#controller.rendererChanged();
            if (!this.#isNodes2())
                this.#renderers.release();
        });
        this.#toolbar.setup();
        this.#refreshButton();
        this.#options.document.defaultView?.addEventListener('pagehide', this.#onPageHide);
    }
    /** Serve both the button and the registered Comfy command through the same use case. */
    toggle() {
        void this.#controller.toggle().catch((error) => this.#options.reportError(error));
    }
    /** Release native mounts before Comfy replaces a workflow's graph objects. */
    beforeConfigureGraph() {
        this.#restoreAfterConfigure = this.#controller.active;
        this.#controller.leaveImmediately();
        this.#releaseGraphObservers();
    }
    /** Restore the selected presentation against the newly configured workflow. */
    afterConfigureGraph() {
        this.#refreshButton();
        if (this.#restoreAfterConfigure && this.#isNodes2())
            this.toggle();
        this.#restoreAfterConfigure = false;
    }
    /** Release all owned DOM, observers, subscriptions, renderer mounts, and animations. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#controller.leaveImmediately();
        this.#toolbar.dispose();
        this.#unsubscribeRenderer?.();
        this.#unsubscribeRenderer = null;
        this.#releaseGraphObservers();
        this.#renderers.dispose();
        this.#options.document.defaultView?.removeEventListener('pagehide', this.#onPageHide);
    }
    /** Tear down before the browser releases the Comfy Vue application. */
    #onPageHide = () => this.dispose();
    /** Read the authoritative Comfy setting rather than inferring from stale graph DOM. */
    #isNodes2() {
        return resolveComfyRendererMode(this.#options.app, null, this.#options.document) === 'vue';
    }
    /** Reflect transient presentation and subscribe only while masonry needs graph updates. */
    #refreshButton() {
        const active = this.#controller.active;
        this.#toolbar.update(active, this.#isNodes2());
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
