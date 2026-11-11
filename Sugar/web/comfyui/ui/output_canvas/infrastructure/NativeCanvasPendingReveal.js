/**
 * Hold only the latest reveal until the staged attachment accepts commands.
 *
 * A browser attachment awaits source preparation before applying a reveal; a native candidate
 * cannot accept commands until it commits, so it defers the same intent instead of refusing it.
 * A newer reveal, any newer view state (another selection, mode or comparison) and disposal
 * each settle the retained reveal as not applied.
 */
export class NativeCanvasPendingReveal {
    options;
    #pending = null;
    /** Bind the attachment's command port and view state authority. */
    constructor(options) {
        this.options = options;
    }
    /** Replace any older retained reveal and resolve once this one is applied or dropped. */
    retain(target) {
        this.drop();
        const revision = this.options.revision();
        return new Promise((settle) => {
            this.#pending = { target, revision, settle };
        });
    }
    /** Apply the retained reveal once the attachment accepts, unless the view changed since. */
    apply() {
        const pending = this.#pending;
        this.#pending = null;
        if (!pending)
            return;
        if (pending.revision !== this.options.revision()) {
            pending.settle(false);
            return;
        }
        void this.options.dispatch(pending.target).then(pending.settle, (error) => {
            this.options.report(error);
            pending.settle(false);
        });
    }
    /** Settle a retained reveal as not applied. */
    drop() {
        const pending = this.#pending;
        this.#pending = null;
        pending?.settle(false);
    }
}
