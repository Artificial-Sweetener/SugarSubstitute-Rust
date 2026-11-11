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
/**
 * Classify a refused viewport request.
 *
 * A stale answer while no preview transfer or pixels claim the surface means a newer camera, such
 * as a wheel gesture, replaced the destination: the pixels stay, the superseded command did not
 * apply, and one read reports the camera now shown. A host that could not apply a command is
 * recovered the same way. While a preview owns the camera, a stale answer is parked without a
 * failure because the preview's display observation settles it.
 */
export function viewportRejection(status, admitted, current, previewOwnsCamera) {
    const superseded = status === 'stale';
    const recover = admitted && (status === 'unavailable' || (superseded && !previewOwnsCamera));
    return Object.freeze({
        recover,
        retry: admitted && (!current || status === 'cancelled' || recover),
        settled: current && superseded && !previewOwnsCamera && !admitted,
        parked: current && superseded && previewOwnsCamera,
        failed: current && status !== 'cancelled' && !recover && !superseded,
    });
}
