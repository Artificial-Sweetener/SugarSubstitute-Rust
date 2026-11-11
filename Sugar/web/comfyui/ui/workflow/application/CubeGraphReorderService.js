import { cubeGraphOrder } from '../domain/CubeGraphOrder.js';
/** Share one canonical placement snapshot and mutation lifecycle across every reorder affordance. */
export class CubeGraphReorderService {
    #gateway;
    #host;
    #listeners = new Set();
    #analysis = null;
    #analyzedSnapshot = null;
    #generation = 0;
    #pending = false;
    #refreshing = null;
    /** Bind graph authority and atomic native commit through separate typed ports. */
    constructor(options) {
        this.#gateway = options.gateway;
        this.#host = options.host;
    }
    /** Allow pointer intent whenever the native root owns mutation, including analysis refreshes. */
    get available() {
        return this.#host.available;
    }
    /** Expose the mutation lifetime so presentation cannot initiate overlapping graph writes. */
    get pending() {
        return this.#pending;
    }
    /** Keep drag permissions through presentation resizes while requiring current native Cube order. */
    get analysis() {
        return this.#analyzedSnapshot && this.#host.isOrderCurrent(this.#analyzedSnapshot)
            ? this.#analysis
            : null;
    }
    /** Observe accepted analysis and busy-state changes without discovering host nodes. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Retire stale requests and preserve permissions whose native placement inputs remain current. */
    invalidate() {
        this.#generation += 1;
        if (!this.analysis) {
            this.#analysis = null;
            this.#analyzedSnapshot = null;
        }
        this.#refreshing = null;
        this.#notify();
    }
    /** Coalesce exact-snapshot analysis while discarding responses from obsolete graph lifetimes. */
    refresh() {
        if (!this.#host.available)
            return Promise.resolve(null);
        const accepted = this.#analyzedSnapshot && this.#host.isPlacementCurrent(this.#analyzedSnapshot)
            ? this.#analysis
            : null;
        if (accepted)
            return Promise.resolve(accepted);
        const snapshot = this.#host.snapshot();
        if (this.#refreshing?.revision === snapshot.revision)
            return this.#refreshing.promise;
        const generation = ++this.#generation;
        const promise = this.#analyzeCurrent(snapshot, generation).finally(() => {
            if (this.#refreshing?.promise === promise)
                this.#refreshing = null;
        });
        this.#refreshing = { revision: snapshot.revision, promise };
        return promise;
    }
    /** Reanalyze settling native geometry while accepting only the exact current workflow. */
    async #analyzeCurrent(initial, generation) {
        let snapshot = initial;
        for (let attempt = 0; attempt < 4; attempt += 1) {
            const analysis = await this.#gateway.analyze(snapshot);
            if (generation !== this.#generation || !this.#host.available)
                return null;
            const current = this.#host.snapshot();
            if (this.#host.isPlacementCurrent(snapshot)) {
                this.#analysis = analysis;
                this.#analyzedSnapshot = current;
                this.#notify();
                return analysis;
            }
            snapshot = current;
            if (this.#refreshing)
                this.#refreshing.revision = snapshot.revision;
        }
        throw new Error('Workflow kept changing during Cube analysis. Try again after it settles.');
    }
    /** Resolve keyboard and arrow movement from accepted native position order. */
    canMove(instanceId, delta) {
        const analysis = this.analysis;
        if (this.#pending || !analysis)
            return false;
        const order = cubeGraphOrder(analysis);
        const index = order.indexOf(instanceId);
        return index >= 0 && index + delta >= 0 && index + delta < order.length;
    }
    /** Submit one adjacent movement through the same transaction used for drag permutations. */
    move(instanceId, delta) {
        return this.#run(instanceId, (current) => {
            const order = [...current];
            const index = order.indexOf(instanceId);
            const neighbor = order[index + delta];
            if (!neighbor)
                throw new Error('Cube movement crosses a graph boundary.');
            order[index] = neighbor;
            order[index + delta] = instanceId;
            return order;
        });
    }
    /** Submit a drag permutation without turning its temporary visual order into workflow state. */
    reorder(instanceId, orderedInstanceIds) {
        return this.#run(instanceId, () => orderedInstanceIds);
    }
    /** Rebase placement conflicts while keeping intent bound to the original native workflow. */
    async #run(instanceId, orderFor) {
        if (this.#pending)
            throw new Error('A Cube reorder is already in progress.');
        this.#pending = true;
        this.#notify();
        try {
            const origin = this.#host.snapshot();
            let ordered = null;
            for (let attempt = 0; attempt < 8; attempt += 1) {
                this.#requireCurrent(origin);
                const analysis = await this.refresh();
                this.#requireCurrent(origin);
                if (!analysis)
                    continue;
                const snapshot = this.#host.snapshot();
                if (!this.#analyzedSnapshot || !this.#host.isPlacementCurrent(this.#analyzedSnapshot))
                    continue;
                const current = cubeGraphOrder(analysis);
                if (!current.includes(instanceId))
                    throw new Error('Cube instance is unavailable.');
                ordered ??= [...orderFor(current)];
                if (ordered.length !== current.length ||
                    new Set(ordered).size !== ordered.length ||
                    ordered.some((id) => !current.includes(id)))
                    throw new Error('Cube reorder must be an exact Cube permutation.');
                if (ordered.every((id, index) => id === current[index]))
                    return;
                const result = await this.#gateway.reorder(snapshot, current, ordered);
                this.#assertResult(analysis, result, ordered);
                this.#requireCurrent(origin);
                if (!this.#host.commit(snapshot, result.placements)) {
                    this.invalidate();
                    continue;
                }
                // The reorder response already contains canonical analysis for the committed placement.
                // Retire earlier requests and bind those facts to the native post-transaction lifetime.
                this.#generation++;
                this.#refreshing = null;
                this.#analysis = this.#host.isCurrent(origin) ? result : null;
                this.#analyzedSnapshot = this.#analysis ? this.#host.snapshot() : null;
                return;
            }
            throw new Error('Cube positions could not settle during reorder. Please retry the move.');
        }
        finally {
            this.#pending = false;
            this.#notify();
        }
    }
    /** Refuse to carry an old gesture into another workflow, navigation scope or undo lifetime. */
    #requireCurrent(snapshot) {
        if (!this.#host.isCurrent(snapshot))
            throw new Error('The original Cube workflow is no longer active.');
    }
    /** Reject foreign instances and unfulfilled permutations even after transport validation. */
    #assertResult(before, after, order) {
        const identities = new Map(before.instances.map((instance) => [instance.instanceId, instance]));
        if (after.instances.length !== before.instances.length ||
            after.instances.some((instance) => {
                const original = identities.get(instance.instanceId);
                return (!original ||
                    original.nodeId !== instance.nodeId ||
                    original.definitionId !== instance.definitionId ||
                    original.cubeId !== instance.cubeId ||
                    original.version !== instance.version ||
                    original.alias !== instance.alias ||
                    original.executionMode !== instance.executionMode);
            }))
            throw new Error('Cube reorder response changed workflow instance identity.');
        const placements = new Map(before.placements.map((placement) => [placement.instanceId, placement]));
        if (after.placements.length !== before.placements.length ||
            new Set(after.placements.map((placement) => placement.instanceId)).size !==
                after.placements.length ||
            after.placements.some((placement) => {
                const original = placements.get(placement.instanceId);
                return (!original ||
                    original.nodeId !== placement.nodeId ||
                    (!order.includes(placement.instanceId) &&
                        (original.position[0] !== placement.position[0] ||
                            original.position[1] !== placement.position[1])));
            }))
            throw new Error('Cube reorder response changed an unrelated placement.');
        const accepted = cubeGraphOrder(after);
        if (accepted.length !== order.length || accepted.some((id, index) => id !== order[index]))
            throw new Error('Cube reorder response did not fulfill the requested permutation.');
    }
    /** Publish coherent application state after each accepted transition. */
    #notify() {
        for (const listener of this.#listeners)
            listener();
    }
}
