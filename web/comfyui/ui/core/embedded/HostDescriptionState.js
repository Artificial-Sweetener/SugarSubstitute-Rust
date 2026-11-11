/** Publish the declaration once it settles for this frontend lifetime. */
export class HostDescriptionState {
    #value = null;
    #listeners = new Set();
    /** Read the settled declaration, or null while it is pending. */
    read() {
        return this.#value;
    }
    /** Observe the declaration settling. */
    subscribe(changed) {
        this.#listeners.add(changed);
        return () => this.#listeners.delete(changed);
    }
    /**
     * Settle on a declaration; a later, different one is refused.
     *
     * @returns Whether the declaration is the settled one, so a contradicting host can be reported.
     */
    settle(value) {
        if (this.#value)
            return this.#value.same(value);
        this.#value = value;
        for (const changed of this.#listeners)
            changed();
        return true;
    }
}
