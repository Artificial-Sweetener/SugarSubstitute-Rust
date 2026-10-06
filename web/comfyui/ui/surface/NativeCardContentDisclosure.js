import { settleNativeDisclosure } from './NativeDisclosureSettlement.js';
/** Separate native content invalidation from value updates and initial hydration. */
export class NativeCardContentDisclosure {
    target;
    motion;
    #membership = null;
    /** Share the exact mount's motion lifetime with native collapse gestures. */
    constructor(target, motion) {
        this.target = target;
        this.motion = motion;
    }
    /** Render native updates immediately, animating only a changed set of presented widgets. */
    reconcile(widgets, render) {
        const membership = JSON.stringify(widgets);
        const changed = this.#membership !== null && this.#membership !== membership;
        this.#membership = membership;
        if (changed)
            this.motion.resize(render, { settle: () => settleNativeDisclosure(this.target) });
        else
            render();
    }
}
