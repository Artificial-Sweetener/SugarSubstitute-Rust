/** Retain session-scoped activity until committed viewing or explicit session retirement. */
export class WorkflowOutputActivity {
    #unread = new Set();
    #listeners = new Set();
    #active = null;
    #open = new Set();
    /** Read activity without manufacturing notifications from restored result snapshots. */
    unread(id) {
        return this.#unread.has(id);
    }
    /** Accept only inactive, still-open workflow results and publish the first unread transition. */
    record(id) {
        if (id === this.#active || !this.#open.has(id) || this.#unread.has(id))
            return;
        this.#unread.add(id);
        this.#publish();
    }
    /** Clear viewing activity only after session activation commits and retire closed identities. */
    synchronize(snapshot) {
        this.#open = new Set(snapshot.open.map((session) => session.sessionId));
        this.#active = snapshot.active?.sessionId ?? null;
        let changed = false;
        for (const id of this.#unread)
            if (!this.#open.has(id) || id === this.#active) {
                this.#unread.delete(id);
                changed = true;
            }
        if (changed)
            this.#publish();
    }
    /** Bind one presentation observer with explicit teardown. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Release activity with the execution runtime rather than persisting it in workflow files. */
    dispose() {
        this.#listeners.clear();
        this.#unread.clear();
        this.#open.clear();
    }
    /** Publish one accepted state transition to mounted views. */
    #publish() {
        for (const listener of this.#listeners)
            listener();
    }
}
