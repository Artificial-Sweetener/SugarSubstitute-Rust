import { settleNativeDisclosure } from './NativeDisclosureSettlement.js';
/** Separate native content invalidation from value updates and initial hydration. */
export class NativeCardContentDisclosure {
    target;
    motion;
    #membership = null;
    #preparedHeight = null;
    /** Share the exact mount's motion lifetime with native collapse gestures. */
    constructor(target, motion) {
        this.target = target;
        this.motion = motion;
    }
    /** Capture batch geometry only for an upcoming membership transition. */
    prepare(widgets) {
        this.#preparedHeight =
            this.#membership !== null && this.#membership !== JSON.stringify(widgets)
                ? this.motion.captureHeight()
                : null;
    }
    /** Render native updates immediately, animating only a changed set of presented widgets. */
    reconcile(widgets, render) {
        const membership = JSON.stringify(widgets);
        const changed = this.#membership !== null && this.#membership !== membership;
        const fromHeight = this.#preparedHeight;
        this.#preparedHeight = null;
        this.#membership = membership;
        if (changed)
            this.motion.resize(render, {
                ...(fromHeight !== null ? { fromHeight } : {}),
                settle: () => settleNativeDisclosure(this.target),
            });
        else
            render();
    }
}
