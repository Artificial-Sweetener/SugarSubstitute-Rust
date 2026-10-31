/** Keep promise settlement and abort-listener custody independent of native stream sequencing. */
export class NativeCanvasViewportIntent {
    command;
    scope;
    cancel;
    finished;
    result;
    running = false;
    #settled = false;
    #resolve;
    /** Retain exactly one caller scope while the presentation owner decides when to forward it. */
    constructor(command, scope, cancel, finished) {
        this.command = command;
        this.scope = scope;
        this.cancel = cancel;
        this.finished = finished;
        this.result = new Promise((resolve) => {
            this.#resolve = resolve;
        });
    }
    /** Observe cancellation only after the presentation owner has installed this intent. */
    observe() {
        this.scope.signal.addEventListener('abort', this.#cancel, { once: true });
        if (!this.scope.current())
            this.complete(false);
    }
    /** Release the caller listener and report one terminal result without releasing native credit. */
    complete(applied) {
        if (this.#settled)
            return;
        this.#settled = true;
        this.scope.signal.removeEventListener('abort', this.#cancel);
        this.finished(this);
        this.#resolve(applied);
    }
    /** Delegate cancellation policy to the sole native operation owner. */
    #cancel = () => {
        this.cancel(this);
    };
}
