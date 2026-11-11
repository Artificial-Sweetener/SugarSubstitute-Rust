/** Own one protocol request's cancellation without exposing browser, scheduler or native handles. */
export class DesktopSessionCancellation {
    #listeners = new Set();
    #cancelled = false;
    /** Read cancellation before every effect and after delayed external completion. */
    cancelled() {
        return this.#cancelled;
    }
    /** Observe retirement; already-cancelled requests notify synchronously. */
    subscribe(listener) {
        if (this.#cancelled)
            listener();
        else
            this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Cancel exactly once and release every observer after notification. */
    cancel() {
        if (this.#cancelled)
            return;
        this.#cancelled = true;
        for (const listener of this.#listeners)
            listener();
        this.#listeners.clear();
    }
}
/** Preserve actionable portable refusal and its underlying cause across frontend adapter boundaries. */
export class DesktopSessionRefusal extends Error {
    code;
    /** Receive a G0 machine-readable code; localized product feedback remains with the host owner. */
    constructor(code, message, options) {
        super(message, options);
        this.code = code;
        this.name = 'DesktopSessionRefusal';
    }
}
