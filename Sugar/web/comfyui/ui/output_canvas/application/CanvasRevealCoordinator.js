import { resolveCanvasReveal } from '../domain/CanvasReveal.js';
/** Keep reveal policy renderer-neutral, without retaining files, pixels, or a second output inventory. */
export class CanvasRevealCoordinator {
    options;
    /** Bind the active-session authority and an explicit canvas destination. */
    constructor(options) {
        this.options = options;
    }
    /** Validate before opening the pane and delegate stale-safe asynchronous preparation. */
    async reveal(target) {
        const current = this.options.read();
        if (!current || !resolveCanvasReveal(current.catalog, target))
            return false;
        const destination = this.options.open(current.viewId);
        const next = this.options.read();
        if (!destination ||
            next?.viewId !== current.viewId ||
            !resolveCanvasReveal(next.catalog, target))
            return false;
        return await destination.reveal(target);
    }
}
