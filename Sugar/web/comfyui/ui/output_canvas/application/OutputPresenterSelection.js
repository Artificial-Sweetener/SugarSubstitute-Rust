/**
 * Keep desktop image presentation native throughout startup and connection changes.
 *
 * A page whose host presents output waits for discovery, then presents native pixels or native
 * diagnostics, and never falls back to browser images. Until the host describes itself the page
 * waits rather than showing browser images it may have to withdraw. Other hosts retain browser
 * presentation until a native surface is negotiated.
 */
export class OutputPresenterSelection {
    port;
    #native = false;
    constructor(port) {
        this.port = port;
    }
    /** Keep still images native and hand distinct media to its viewer after current discovery. */
    current() {
        const available = this.port.surface() !== null;
        if (available)
            this.#native = true;
        const owner = this.port.owner();
        this.#native ||= owner === 'host';
        if (!this.#native)
            return owner === 'pending' ? 'pending' : 'browser';
        if (!available && !this.port.discovered())
            return 'pending';
        return this.port.browserMedia?.() ? 'browser' : 'native';
    }
}
/** Consume the existing display plan rather than introducing a second selection or grid policy. */
export function canvasRequiresBrowserMedia(plan) {
    const media = (item) => {
        if (!item)
            return false;
        return (item.mediaKind === 'animated' || item.mediaKind === 'video' || item.mediaKind === 'model');
    };
    if (plan.view.mode === 'overview')
        return plan.grid.some((entry) => entry.items.some(media));
    return !plan.preview && (media(plan.primary?.item) || media(plan.secondary?.item));
}
