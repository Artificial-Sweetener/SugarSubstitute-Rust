/** Keep source discovery scoped to the live view and the current replacement batch. */
export class WorkflowMasonryRemountQueue {
    getSources;
    onRemounted;
    #pending = new Map();
    #disposed = false;
    /** Resolve current sources only after their owners finish replacing native controls. */
    constructor(getSources, onRemounted) {
        this.getSources = getSources;
        this.onRemounted = onRemounted;
    }
    /** Deduplicate repeated invalidations of a card within one native replacement batch. */
    request(id, rebind) {
        if (this.#disposed)
            return;
        const scheduled = this.#pending.size > 0;
        this.#pending.set(id, rebind);
        if (!scheduled)
            queueMicrotask(this.#flush);
    }
    /** Discard callbacks when the owning workflow leaves the presentation. */
    dispose() {
        this.#disposed = true;
        this.#pending.clear();
    }
    /** Share one ephemeral inventory without retaining replaced native DOM between batches. */
    #flush = () => {
        if (this.#disposed || this.#pending.size === 0)
            return;
        const batch = [...this.#pending];
        this.#pending.clear();
        const sources = this.getSources();
        let changed = false;
        for (const [id, rebind] of batch) {
            if (this.#disposed)
                return;
            changed = rebind(sources.get(id) ?? null) || changed;
        }
        if (changed && !this.#disposed)
            this.onRemounted();
    };
}
