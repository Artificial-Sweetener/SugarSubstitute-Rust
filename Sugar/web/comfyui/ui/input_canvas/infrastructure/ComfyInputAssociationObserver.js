import { NativeSubgraphChangeObserver } from '../../surface/NativeSubgraphChangeObserver.js';
import { getGraphNodes } from '../../graph/GraphQuery.js';
/** Keep one graph's reversible native callback leases with its association lifecycle. */
export class ComfyInputAssociationObserver {
    graph;
    changed;
    #membership;
    #nodes = new Map();
    #stopConfigure;
    #active = true;
    /** Subscribe only to actual native topology operations and preserve their existing callbacks and receivers. */
    constructor(graph, changed) {
        this.graph = graph;
        this.changed = changed;
        this.#membership =
            typeof graph.add === 'function' && typeof graph.remove === 'function'
                ? new NativeSubgraphChangeObserver(graph, () => {
                    if (this.#active)
                        changed(true);
                }, (kind) => kind !== 'change')
                : null;
        const configure = graph.configure;
        if (typeof configure === 'function') {
            const configured = (...args) => {
                const result = configure.apply(graph, args);
                if (this.#active)
                    changed(true);
                return result;
            };
            graph.configure = configured;
            this.#stopConfigure = () => {
                if (graph.configure === configured)
                    graph.configure = configure;
            };
        }
        else
            this.#stopConfigure = () => { };
        this.reconcile();
    }
    /** Attach newly admitted nodes and retire removed handles after a real membership event. */
    reconcile() {
        const nodes = new Set(getGraphNodes(this.graph));
        for (const [node, stop] of this.#nodes)
            if (!nodes.has(node)) {
                stop();
                this.#nodes.delete(node);
            }
        for (const node of nodes) {
            if (this.#nodes.has(node))
                continue;
            const previous = node.onConnectionsChange;
            const changed = (...args) => {
                const result = typeof previous === 'function' ? previous.apply(node, args) : undefined;
                if (this.#active)
                    this.changed(false);
                return result;
            };
            node.onConnectionsChange = changed;
            this.#nodes.set(node, () => {
                if (node.onConnectionsChange === changed)
                    node.onConnectionsChange = previous;
            });
        }
    }
    /** Release all topology leases without disturbing callbacks installed by another native owner. */
    dispose() {
        this.#active = false;
        this.#membership?.dispose();
        this.#stopConfigure();
        for (const stop of this.#nodes.values())
            stop();
        this.#nodes.clear();
    }
}
