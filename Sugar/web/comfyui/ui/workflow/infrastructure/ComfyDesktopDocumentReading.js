/**
 * Comfy restores saved tabs in the background and opens a chosen tab before its file arrives, so an open
 * workflow can briefly (or, until activated, indefinitely) hold no document. Comfy loads every tab it opens
 * itself, so only background tabs need this owner: it asks Comfy to read each one through its supported load,
 * one at a time, never overlapping a read Comfy has started, and remembers a failed read per occurrence so a
 * broken file is reported once rather than retried in a loop.
 */
export class ComfyDesktopDocumentReading {
    options;
    #failed = new Map();
    #running = false;
    #disposed = false;
    constructor(options) {
        this.options = options;
    }
    /** Start reading every unread workflow Comfy is not already reading, unless a pass is under way. */
    request() {
        if (this.#running || this.#disposed || !this.#next())
            return;
        this.#running = true;
        void this.#drain();
    }
    /** Whether any open document is still waiting for Comfy to read it. */
    pending() {
        return this.options.documents.unread().length > 0;
    }
    /** Retire the pass; a read already in flight finishes inside Comfy but nothing observes it. */
    dispose() {
        this.#disposed = true;
        this.#failed.clear();
    }
    /** Read workflows until none remain that have not already failed for their current occurrence. */
    async #drain() {
        try {
            for (let next = this.#next(); next && !this.#disposed; next = this.#next()) {
                try {
                    await this.options.documents.read(next);
                }
                catch (cause) {
                    this.#failed.set(next.sessionId, next.generation);
                    this.options.report(new Error(`Workflow "${next.path}" could not be read for the desktop session.`, {
                        cause,
                    }));
                }
            }
        }
        finally {
            this.#running = false;
            if (!this.#disposed)
                this.options.settled();
        }
    }
    /** The first unread workflow that Comfy is not reading and whose current occurrence has not failed. */
    #next() {
        const { documents } = this.options;
        const unread = documents.unread();
        for (const id of [...this.#failed.keys()])
            if (!unread.some((context) => context.sessionId === id))
                this.#failed.delete(id);
        return unread.find((context) => this.#failed.get(context.sessionId) !== context.generation &&
            !documents.isBeingRead(context));
    }
}
