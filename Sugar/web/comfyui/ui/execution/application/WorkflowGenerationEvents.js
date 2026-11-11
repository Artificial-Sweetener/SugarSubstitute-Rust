/** Isolate consumer failures so canvas delivery cannot interrupt queue or result ownership. */
export class WorkflowGenerationEvents {
    report;
    #listeners = new Set();
    /** Bind actionable failure reporting at the composition boundary. */
    constructor(report) {
        this.report = report;
    }
    /** Observe future transitions only; restored results are never replayed as new generations. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Notify each consumer independently while preserving publication order. */
    publish(event) {
        for (const listener of this.#listeners) {
            try {
                listener(event);
            }
            catch (error) {
                this.report(error);
            }
        }
    }
    /** Cancel live consumers before releasing subscriptions at integration teardown. */
    dispose() {
        this.publish({ type: 'disposed' });
        this.#listeners.clear();
    }
}
