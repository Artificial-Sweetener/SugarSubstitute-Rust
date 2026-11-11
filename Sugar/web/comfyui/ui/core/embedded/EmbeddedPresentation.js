import { freezePanelPresentation, samePanelPresentation } from './PanelRuntime.js';
/** Publish only negotiated presentation changes for this frontend lifetime. */
export class EmbeddedPresentation {
    #value = null;
    #listeners = new Set();
    /** Read the immutable accepted host configuration. */
    read() {
        return this.#value;
    }
    /** Observe preference precedence without sharing mutable host state. */
    subscribe(changed) {
        this.#listeners.add(changed);
        return () => this.#listeners.delete(changed);
    }
    /** Apply or release a host session while preserving each owner's saved preference. */
    set(value) {
        if (samePanelPresentation(value, this.#value))
            return;
        this.#value = value ? freezePanelPresentation(value) : null;
        for (const changed of this.#listeners)
            changed();
    }
}
