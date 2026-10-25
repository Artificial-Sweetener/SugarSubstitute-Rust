import { InputAssetAssociations } from '../application/InputAssetAssociations.js';
import { ComfyInputAssociationGraph } from './ComfyInputAssociationGraph.js';
import { ComfyInputAssociationObserver } from './ComfyInputAssociationObserver.js';
/** Own lifecycle and host translation while application policy owns every relationship decision. */
export class ComfyInputAssociations {
    bindings;
    root;
    report;
    relationships = new InputAssetAssociations();
    #graph;
    #observers = new Map();
    #facts = new Map();
    #dirty = new Set();
    #membership = true;
    #scheduled = false;
    #active = false;
    #generation = 0;
    #membershipReads = 0;
    #topologyReads = 0;
    /** Inject the root workflow and authoritative source-binding registry instead of reading rendered cards. */
    constructor(loaders, bindings, root, report, logger) {
        this.bindings = bindings;
        this.root = root;
        this.report = report;
        this.#graph = new ComfyInputAssociationGraph(loaders, logger);
    }
    /** Expose the same opaque field identity to future application consumers. */
    key(node, field) {
        return this.#graph.key(node, field);
    }
    /** Resolve any owned endpoint through the authoritative binding registry for reusable application use cases. */
    binding(endpoint) {
        const source = this.#graph.field(endpoint);
        return source ? this.bindings.get(source.node, source.loader) : null;
    }
    /** Adapt a relationship query to the existing source owner without introducing another source picker. */
    image(node, field) {
        const key = this.key(node, field);
        return {
            read: () => {
                const association = this.relationships.read(key);
                if (association.kind !== 'associated')
                    return null;
                const source = this.binding(association.image);
                return source?.loader.kind === 'image' ? source : null;
            },
            subscribe: (listener) => this.relationships.subscribe((changed) => {
                if (changed.has(key))
                    listener();
            }),
        };
    }
    /** Activate the current native workflow after setup or an explicitly completed configuration. */
    start() {
        this.#active = true;
        this.#membership = true;
        this.#flush();
    }
    /** Re-read membership only after Comfy refreshes its node-definition catalog. */
    definitionsChanged() {
        if (this.#active) {
            this.#membership = true;
            this.#schedule();
        }
    }
    /** Retire callbacks and relationship identities before another workflow replaces live native handles. */
    beforeConfigureGraph() {
        this.#active = false;
        this.#generation++;
        this.#scheduled = false;
        for (const observer of this.#observers.values())
            observer.dispose();
        this.#observers.clear();
        this.#facts.clear();
        this.#dirty.clear();
        this.#graph.reconcile(null);
        this.relationships.replace([]);
    }
    /** Release all graph ownership when the browser host is retired. */
    dispose() {
        this.beforeConfigureGraph();
    }
    /** Report native discovery work separately from source, value and popup activity. */
    diagnostics() {
        return {
            graphs: this.#observers.size,
            membershipReads: this.#membershipReads,
            topologyReads: this.#topologyReads,
        };
    }
    /** Coalesce synchronous native connection callbacks until their graph transaction is settled. */
    #schedule() {
        if (!this.#active || this.#scheduled)
            return;
        this.#scheduled = true;
        const generation = this.#generation;
        queueMicrotask(() => {
            if (this.#active && generation === this.#generation)
                this.#flush();
        });
    }
    /** Refresh changed scopes and their exposed ancestors while leaving unrelated graph branches cached. */
    #flush() {
        this.#scheduled = false;
        if (!this.#active)
            return;
        try {
            const dirty = new Set();
            if (this.#membership) {
                const scopes = this.#graph.reconcile(this.root());
                this.#membershipReads++;
                const graphs = new Set(scopes.map((scope) => scope.graph));
                for (const [graph, observer] of this.#observers)
                    if (!graphs.has(graph)) {
                        observer.dispose();
                        this.#observers.delete(graph);
                    }
                this.#facts.clear();
                for (const { graph } of scopes) {
                    if (!this.#observers.has(graph))
                        this.#observers.set(graph, new ComfyInputAssociationObserver(graph, (membership) => {
                            if (!this.#active)
                                return;
                            this.#membership ||= membership;
                            this.#dirty.add(graph);
                            this.#schedule();
                        }));
                    else
                        this.#observers.get(graph)?.reconcile();
                    dirty.add(graph);
                }
            }
            else
                for (const graph of this.#dirty) {
                    let scope = this.#graph.scope(graph);
                    while (scope && !dirty.has(scope.graph)) {
                        dirty.add(scope.graph);
                        scope = scope.parent ? this.#graph.scope(scope.parent) : null;
                    }
                }
            this.#membership = false;
            this.#dirty.clear();
            for (const graph of dirty) {
                this.#facts.set(graph, this.#graph.read(graph));
                this.#topologyReads++;
            }
            this.relationships.replace([...this.#facts.values()]);
        }
        catch (error) {
            this.relationships.replace([]);
            this.report(error);
        }
    }
}
