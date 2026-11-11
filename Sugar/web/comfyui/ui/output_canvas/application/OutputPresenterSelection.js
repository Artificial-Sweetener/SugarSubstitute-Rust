/**
 * Keep desktop image presentation native throughout startup and connection changes.
 *
 * A desktop page waits for discovery, then presents native pixels or native diagnostics. Its
 * identity is available before discovery and never permits browser image fallback. Other hosts
 * retain browser presentation until a native surface is negotiated.
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
        this.#native ||= this.port.requiresNative();
        if (!this.#native)
            return 'browser';
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
