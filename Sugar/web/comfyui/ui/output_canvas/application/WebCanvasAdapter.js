/** Keep web view projection separate from execution attribution and document-layer insertion. */
export class WebCanvasAdapter {
    adapter;
    #results = new Map();
    #listeners = new Set();
    /** Supply existing preview presentation while the common coordinator owns event routing. */
    constructor(previews) {
        this.adapter = {
            id: 'web',
            capabilities: {
                output: {
                    ordering: 'immediate',
                    bind: () => ({
                        receive: (output) => {
                            if (output.state)
                                this.#update(output.state);
                        },
                    }),
                },
                outputView: {
                    update: (state) => this.#update(state),
                    restore: (state) => this.#update(state),
                },
                preview: previews,
            },
        };
    }
    /** Read only metadata delivered to this canvas through its declared capabilities. */
    read(sessionId) {
        return this.#results.get(sessionId) ?? null;
    }
    /** Refresh existing tabs and batches after a scoped canvas delivery. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Release lightweight projection state with the web canvas lifetime. */
    dispose() {
        this.#results.clear();
        this.#listeners.clear();
    }
    /** Apply an authoritative view snapshot without recreating output or merge policies. */
    #update(state) {
        if (state.result === this.read(state.sessionId))
            return;
        if (state.result)
            this.#results.set(state.sessionId, state.result);
        else
            this.#results.delete(state.sessionId);
        for (const listener of this.#listeners)
            listener(state.sessionId);
    }
}
