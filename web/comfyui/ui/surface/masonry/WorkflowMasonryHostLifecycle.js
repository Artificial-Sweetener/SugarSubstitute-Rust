import { isRecord } from '../../types/common.js';
import { createComfyRendererModeChangeSource, resolveComfyRendererMode, } from '../../core/ComfyRendererMode.js';
import { CubeSurfaceRendererLifecycle } from '../CubeSurfaceRendererLifecycle.js';
import { NativeSubgraphChangeObserver, } from '../NativeSubgraphChangeObserver.js';
import { WorkflowMasonryController } from './WorkflowMasonryController.js';
import { projectWorkflowMasonry, } from './WorkflowMasonryProjection.js';
import { ensureWorkflowMasonryStyles } from './WorkflowMasonryStyles.js';
/** Compose focused owners and keep host registration free of layout behavior. */
export class WorkflowMasonryHostLifecycle {
    #options;
    #controller;
    #renderers;
    #button;
    #presence;
    #graphs = new Map();
    #unsubscribeRenderer = null;
    #unsubscribeCards = null;
    #restoreAfterConfigure = false;
    #disposed = false;
    /** Bind native renderer capabilities and the existing SugarCubes dialog service. */
    constructor(options) {
        this.#options = options;
        this.#renderers = new CubeSurfaceRendererLifecycle({
            document: options.document,
            logger: options.logger,
        });
        this.#button = options.document.createElement('button');
        this.#button.className = 'sugarcubes-masonry-toggle';
        this.#button.type = 'button';
        this.#button.setAttribute('aria-label', 'Toggle workflow masonry');
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
            alert: options.alert,
            reportError: options.reportError,
            onStateChange: () => this.#refreshButton(),
        });
        this.#button.addEventListener('click', () => this.toggle());
        this.#presence = new MutationObserver(() => this.#attachButton());
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
        this.#presence.observe(this.#options.document.body, { childList: true, subtree: true });
        this.#attachButton();
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
        this.#presence.disconnect();
        this.#unsubscribeRenderer?.();
        this.#unsubscribeRenderer = null;
        this.#releaseGraphObservers();
        this.#button.remove();
        this.#renderers.dispose();
        this.#options.document.defaultView?.removeEventListener('pagehide', this.#onPageHide);
    }
    /** Tear down before the browser releases the Comfy Vue application. */
    #onPageHide = () => this.dispose();
    /** Read the authoritative Comfy setting rather than inferring from stale graph DOM. */
    #isNodes2() {
        return resolveComfyRendererMode(this.#options.app, null, this.#options.document) === 'vue';
    }
    /** Reattach the same button after host DOM replacement without duplicating listeners. */
    #attachButton() {
        if (this.#disposed)
            return;
        const container = this.#options.document.querySelector('.graph-canvas-panel') ??
            this.#options.document.getElementById('graph-canvas-container');
        if (container && this.#button.parentElement !== container)
            container.append(this.#button);
    }
    /** Reflect transient presentation and subscribe only while masonry needs graph updates. */
    #refreshButton() {
        const active = this.#controller.active;
        const pressed = String(active);
        if (this.#button.getAttribute('aria-pressed') !== pressed)
            this.#button.setAttribute('aria-pressed', pressed);
        const label = active ? 'Graph view' : 'Masonry';
        if (this.#button.textContent !== label)
            this.#button.textContent = label;
        this.#button.disabled = !this.#isNodes2();
        this.#button.title = this.#button.disabled
            ? 'Masonry requires Nodes 2.0'
            : 'Toggle workflow masonry';
        if (active)
            this.#observeGraphs();
        else
            this.#releaseGraphObservers();
    }
    /** Observe topology through the existing reversible native graph adapter. */
    #observeGraphs() {
        this.#unsubscribeCards ??=
            this.#options.getCardState()?.subscribe(this.#onCardsChanged) ?? null;
        const graph = this.#options.app?.canvas?.graph ?? this.#options.app?.graph;
        const graphs = [graph, ...this.#options.getCubes().map((cube) => cube.subgraph)];
        for (const candidate of graphs) {
            if (!isObservableGraph(candidate) || this.#graphs.has(candidate))
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
