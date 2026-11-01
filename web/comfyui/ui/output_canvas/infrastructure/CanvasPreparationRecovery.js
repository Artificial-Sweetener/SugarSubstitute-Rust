/** Delays before each fresh attempt; after the last, the target shows its error state. */
export const CANVAS_PREPARATION_RETRY_DELAYS = Object.freeze([1000, 2000, 4000]);
/**
 * Own the attempt count and the single backoff deadline for one attachment target.
 *
 * Each preparation timeout of the same target schedules the next fresh attempt after a longer
 * delay through the injected clock; a different target, a successful attachment or an explicit
 * reset starts over. Only one deadline is ever pending.
 */
export class CanvasPreparationRecovery {
    options;
    #target = null;
    #attempts = 0;
    #stop = null;
    /** Bind the clock and the attachment owner's retry and exhaustion actions. */
    constructor(options) {
        this.options = options;
    }
    /** Schedule the next fresh attempt after a timeout, or report exhaustion. */
    failed(target) {
        if (!this.#target || !sameCanvasPreparationTarget(this.#target, target))
            this.reset();
        this.#target = target;
        this.#stop?.();
        this.#stop = null;
        const delay = CANVAS_PREPARATION_RETRY_DELAYS[this.#attempts];
        this.#attempts += 1;
        if (delay === undefined) {
            this.options.exhausted(target);
            return;
        }
        this.#stop = this.options.clock.later(delay, () => {
            this.#stop = null;
            this.options.retry(target);
        });
    }
    /** Forget attempts and any pending retry after success or a changed request. */
    reset() {
        this.#stop?.();
        this.#stop = null;
        this.#target = null;
        this.#attempts = 0;
    }
}
/** Compare every target member, including the exact mounted element. */
export function sameCanvasPreparationTarget(left, right) {
    return (left.view === right.view &&
        left.generation === right.generation &&
        left.element === right.element &&
        left.renderer === right.renderer);
}
