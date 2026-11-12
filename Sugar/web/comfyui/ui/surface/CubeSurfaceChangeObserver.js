import { NativeSubgraphChangeObserver } from './NativeSubgraphChangeObserver.js';
import { subscribeNativeNodeCardRefresh } from './NativeNodeCardRefresh.js';
/** Keep card visibility responsive to native field leases without rediscovering unrelated graph nodes. */
export class CubeSurfaceChangeObserver {
    #topology;
    #stop;
    /** Watch exact graph and field owners for this mounted Cube, including its exposed boundary controls. */
    constructor(document, node, changed) {
        this.#topology = new NativeSubgraphChangeObserver(node.subgraph, changed);
        this.#stop = subscribeNativeNodeCardRefresh(document, [node, ...node.subgraph._nodes], changed);
    }
    /** Release both subscriptions before the native face is replaced or unmounted. */
    dispose() {
        this.#topology.dispose();
        this.#stop();
    }
}
