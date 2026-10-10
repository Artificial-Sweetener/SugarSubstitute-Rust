import { isRecord } from '../types/common.js';
import { HostSurfaceObserver } from '../core/HostSurfaceObserver.js';
import { NativeSubgraphChangeObserver } from '../surface/NativeSubgraphChangeObserver.js';
import { subscribeNativeNodeCardRefresh } from '../surface/NativeNodeCardRefresh.js';
import { WorkflowRelations } from './application/WorkflowRelations.js';
import { NativeRelationWorkflowPort } from './infrastructure/NativeRelationWorkflowPort.js';
import { NativeRelationExecution } from './infrastructure/NativeRelationExecution.js';
import { NativeRelationViews } from './infrastructure/NativeRelationViews.js';
import { NativeSharedSeedModes } from './infrastructure/NativeSharedSeedModes.js';
import { NativeRelationWidgetRenderer } from './infrastructure/NativeRelationWidgetRenderer.js';
import { NativeRelationVisibility } from './infrastructure/NativeRelationVisibility.js';
import { NativeOverrideToolbarHost } from './infrastructure/NativeOverrideToolbarHost.js';
import { createNativeOverrideToolbarState } from './infrastructure/NativeOverrideToolbarState.js';
import { GlobalOverrideToolbar } from './presentation/GlobalOverrideToolbar.js';
import { CubeRelationPresenter } from './presentation/CubeRelationPresenter.js';
import { ensureRelationStyles } from './presentation/RelationStyles.js';
/** Own relation lifecycle while delegating policy, native interaction, persistence, and rendering. */
export class RelationHostLifecycle {
    options;
    #port;
    relations;
    #execution;
    #toolbar;
    #cards;
    #views;
    #visibility = new NativeRelationVisibility();
    #seedModes = new NativeSharedSeedModes();
    #graphs = new Map();
    #presence = null;
    #chromePresence = null;
    #pending = false;
    #valuesPending = false;
    #active = false;
    #rendering = false;
    #unsubscribeChoices = null;
    /** Compose explicit collaborators against the root workflow rather than the navigated canvas graph. */
    constructor(options) {
        this.options = options;
        this.#port = new NativeRelationWorkflowPort(() => options.app?.graph, options.document, options.logger, options.getCardState, options.getConnections);
        this.relations = new WorkflowRelations(this.#port);
        this.#execution = new NativeRelationExecution(this.relations, this.#port.inventorySource, options.logger, () => options.app?.graph?.serialize?.());
        const widgets = new NativeRelationWidgetRenderer(options.document, options.reportError);
        this.#views = new NativeRelationViews(options.document);
        this.#toolbar = new GlobalOverrideToolbar(options.document, this.relations, options.reportError, widgets, new NativeOverrideToolbarHost(options.document), createNativeOverrideToolbarState(options.document, options.logger));
        this.#cards = new CubeRelationPresenter(options.document, this.relations, () => {
            const nodes = new Map([...this.#port.inventorySource.owners.values()].map((owner) => [owner.nodeId, owner]));
            return [...nodes].flatMap(([nodeId, owner]) => nodeId
                ? this.#views.read(owner.face, owner.cubeId).map((surface) => ({
                    nodeId,
                    surface,
                }))
                : []);
        }, options.reportError, widgets);
        this.relations.subscribe(() => this.#present());
    }
    /** Attach after Comfy initializes its root graph and shell. */
    setup() {
        if (this.#active)
            return;
        this.#active = true;
        ensureRelationStyles(this.options.document);
        const changed = (changes) => {
            this.#views.invalidate(changes.map((change) => change.element));
            if (!this.#rendering)
                this.#present();
        };
        this.#presence = new HostSurfaceObserver(this.options.document.body, '.sugarcubes-cube-face__node-card,.lg-node[data-node-id],[data-sugarcubes-cube-face-projection]', changed, { content: true, exclude: '.sc-field-relation,.sc-overrides-toolbar' });
        this.#chromePresence = new HostSurfaceObserver(this.options.document.body, '.actionbar-container', changed);
        this.options.document.addEventListener('input', this.#nativeEdit, true);
        this.options.document.addEventListener('change', this.#nativeEdit, true);
        this.#unsubscribeChoices = subscribeNativeNodeCardRefresh(this.options.document, [], () => this.#schedule());
        this.afterConfigureGraph();
    }
    /** Drop stale mounted views before another native workflow is configured. */
    beforeConfigureGraph() {
        for (const observer of this.#graphs.values())
            observer.dispose();
        this.#graphs.clear();
        this.#cards.dispose();
        this.#views.clear();
        this.#visibility.dispose();
        this.#seedModes.dispose();
    }
    /** Discover loaded instances without relying on whether their native cards have mounted. */
    afterConfigureGraph() {
        this.#schedule();
    }
    /** Reconcile reordered membership inside the graph mutation owner's native history transaction. */
    reconcileCubeOrder() {
        this.#port.joinTransaction(() => this.relations.reconcile());
    }
    /** Prepare either queue path through the same workflow relation owner. */
    prepare(payload) {
        return this.#execution.prepare(payload);
    }
    /** Release all reversible host hooks and presentation at application teardown. */
    dispose() {
        this.#active = false;
        this.#presence?.disconnect();
        this.#chromePresence?.disconnect();
        this.#unsubscribeChoices?.();
        this.#unsubscribeChoices = null;
        this.beforeConfigureGraph();
        this.#toolbar.dispose();
        this.options.document.removeEventListener('input', this.#nativeEdit, true);
        this.options.document.removeEventListener('change', this.#nativeEdit, true);
    }
    /** Coalesce native edits after their own promoted-widget forwarding callbacks. */
    #nativeEdit = (event) => {
        if (!(event.target instanceof Element))
            return;
        if (!event.target.closest('.lg-node,.dom-widget,[data-cube-node-id],[data-masonry-card]'))
            return;
        if (event.target.closest('[role="listbox"],.sc-field-relation,.sc-model-popup'))
            return;
        if (this.#valuesPending)
            return;
        this.#valuesPending = true;
        queueMicrotask(() => queueMicrotask(() => {
            this.#valuesPending = false;
            if (!this.#active)
                return;
            try {
                this.relations.reconcile(false);
            }
            catch (error) {
                this.options.reportError(error);
            }
        }));
    };
    /** Observe topology changes independently from the currently active graph or masonry surface. */
    #observeGraphs() {
        const active = new Set();
        const visit = (graph) => {
            if (!graph || active.has(graph))
                return;
            active.add(graph);
            for (const node of graph._nodes ?? graph.nodes ?? [])
                if (isRecord(node.subgraph))
                    visit(node.subgraph);
            if (!this.#graphs.has(graph) &&
                typeof graph.add === 'function' &&
                typeof graph.remove === 'function')
                this.#graphs.set(graph, new NativeSubgraphChangeObserver(graph, () => this.#schedule(), (kind) => kind !== 'change' || !this.#port.editing));
        };
        visit(this.options.app?.graph);
        for (const [graph, observer] of this.#graphs)
            if (!active.has(graph)) {
                observer.dispose();
                this.#graphs.delete(graph);
            }
    }
    /** Reconcile once per native event batch and report failures through SugarCubes feedback. */
    #schedule() {
        if (!this.#active || this.#pending)
            return;
        this.#pending = true;
        queueMicrotask(() => {
            this.#pending = false;
            if (!this.#active)
                return;
            try {
                this.#observeGraphs();
                this.relations.reconcile();
            }
            catch (error) {
                this.options.reportError(error);
            }
        });
    }
    /** Render from a completed application snapshot without re-entering candidate discovery. */
    #present() {
        if (!this.#active)
            return;
        this.#rendering = true;
        try {
            const snapshot = this.relations.snapshot();
            if (this.#seedModes.reconcile(snapshot, this.#port.inventorySource))
                this.#port.save(snapshot.state);
            const changed = this.#visibility.reconcile(snapshot, this.#port.inventorySource);
            if (changed.length)
                this.#port.refresh(changed);
            this.#toolbar.reconcile();
            this.#cards.reconcile();
        }
        finally {
            this.#presence?.discardPending();
            this.#chromePresence?.discardPending();
            this.#rendering = false;
        }
    }
}
