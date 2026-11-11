/**
 * Keep one command in flight so the host observes user intent in order.
 *
 * Slider drags and amount previews pass a coalescing key: a queued, unsent command with the same
 * key is replaced in place and settles with null, so the host receives only the latest value
 * without reordering it around commands such as Apply.
 */
export class CanvasEditCommandQueue {
    host;
    outcomes;
    #queued = [];
    #running = false;
    #disposed = false;
    /** Borrow the host's correlated transport; the queue owns no request identities. */
    constructor(host, outcomes) {
        this.host = host;
        this.outcomes = outcomes;
    }
    /** Queue a command; resolves with its reply, or null when superseded, closed or failed. */
    send(command, key = null) {
        if (this.#disposed)
            return Promise.resolve(null);
        return new Promise((settle) => {
            const entry = { command, key, settle };
            const replaced = key === null ? -1 : this.#queued.findIndex((queued) => queued.key === key);
            if (replaced >= 0) {
                this.#queued[replaced]?.settle(null);
                this.#queued[replaced] = entry;
            }
            else
                this.#queued.push(entry);
            void this.#pump();
        });
    }
    /** Settle every unsent command without sending it; an in-flight reply is still adopted. */
    dispose() {
        this.#disposed = true;
        for (const entry of this.#queued.splice(0))
            entry.settle(null);
    }
    async #pump() {
        if (this.#running)
            return;
        this.#running = true;
        try {
            for (let entry = this.#queued.shift(); entry; entry = this.#queued.shift())
                entry.settle(await this.#execute(entry.command));
        }
        finally {
            this.#running = false;
        }
    }
    async #execute(command) {
        try {
            const result = await this.host.execute(command);
            if (result.kind === 'state')
                this.outcomes.adopt(result.state);
            else if (result.status !== 'cancelled')
                this.outcomes.report(new Error(result.message), command);
            return result;
        }
        catch (error) {
            this.outcomes.report(error, command);
            return null;
        }
    }
}
