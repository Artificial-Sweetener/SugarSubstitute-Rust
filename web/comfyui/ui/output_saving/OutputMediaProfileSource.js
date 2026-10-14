/** Own profile revision, observation and teardown independently of individual byte transfers. */
export class OutputMediaProfileSource {
    load;
    #listeners = new Set();
    #profile = Object.freeze({
        preferredCopyFormat: 'png',
        fileAction: 'download',
    });
    #revision = 0;
    #disposed = false;
    /** Inject the validated capability boundary instead of reading host or saving globals. */
    constructor(load) {
        this.load = load;
    }
    /** Read the last accepted metadata with a conservative download endpoint before initialization. */
    read() {
        return this.#profile;
    }
    /** Observe semantic capability changes without installing timers or native mount observers. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Refuse superseded responses and notify only when the accepted profile actually changes. */
    async refresh() {
        if (this.#disposed)
            return;
        const revision = ++this.#revision;
        const profile = await this.load();
        if (this.#disposed || revision !== this.#revision)
            return;
        if (profile.preferredCopyFormat === this.#profile.preferredCopyFormat &&
            profile.fileAction === this.#profile.fileAction)
            return;
        this.#profile = profile;
        for (const listener of [...this.#listeners])
            listener();
    }
    /** Invalidate late observations and release every consumer on host teardown. */
    dispose() {
        this.#disposed = true;
        this.#revision++;
        this.#listeners.clear();
    }
}
