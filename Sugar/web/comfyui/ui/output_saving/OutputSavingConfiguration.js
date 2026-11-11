/** Share one presentation snapshot across settings, source controls and mounted native fields. */
export class OutputSavingConfiguration {
    client;
    #listeners = new Set();
    #snapshot = null;
    #revision = 0;
    #loading = null;
    /** Bind validated transport without creating browser-owned saving defaults. */
    constructor(client) {
        this.client = client;
    }
    /** Expose only the last accepted server observation. */
    read() {
        return this.#snapshot;
    }
    /** Observe deliberate settings changes without observing DOM activity or rebuilding graph inventory. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Coalesce reads and prevent an older GET from replacing a successfully committed edit. */
    load() {
        if (this.#loading)
            return this.#loading;
        const revision = this.#revision;
        this.#loading = this.client
            .load()
            .then((snapshot) => {
            if (revision !== this.#revision && this.#snapshot)
                return this.#snapshot;
            this.#accept(snapshot);
            return snapshot;
        })
            .finally(() => {
            this.#loading = null;
        });
        return this.#loading;
    }
    /** Publish settings only after their authoritative write succeeds. */
    async save(preferences) {
        const snapshot = await this.client.save(preferences);
        this.#revision++;
        this.#accept(snapshot);
        return snapshot;
    }
    /** Publish an explicit import without treating it as workflow-manager opt-in. */
    async importSubstitute(value) {
        const snapshot = await this.client.importSubstitute(value);
        this.#revision++;
        this.#accept(snapshot);
        return snapshot;
    }
    /** Keep draft path examples out of the committed presentation snapshot. */
    preview(preferences, signal) {
        return this.client.preview(preferences, signal);
    }
    /** Notify consumers of accepted observations without owning their control lifetimes. */
    #accept(snapshot) {
        this.#snapshot = snapshot;
        for (const listener of this.#listeners)
            listener();
    }
}
