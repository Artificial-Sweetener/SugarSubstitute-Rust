/**
 * Hold the page's one update source and announce when it appears or retires.
 *
 * The host's source takes this slot when the host declares `sugarcubes.updates`. SugarCubes'
 * own updater attaches only once the host description has settled without that protocol, so
 * two sources never compete for the entry.
 */
export class UpdateSourceSlot {
    #source = null;
    #listeners = new Set();
    /** The attached source, or null when nothing on this page updates. */
    read() {
        return this.#source;
    }
    /** Observe a source attaching or retiring. */
    subscribe(changed) {
        this.#listeners.add(changed);
        return () => this.#listeners.delete(changed);
    }
    /**
     * Attach the page's update source.
     *
     * @returns The release that retires it from the slot.
     * @throws Error when another source is already attached.
     */
    attach(source) {
        if (this.#source)
            throw new Error('An update source is already attached to this page.');
        this.#source = source;
        this.#publish();
        return () => {
            if (this.#source !== source)
                return;
            this.#source = null;
            this.#publish();
        };
    }
    #publish() {
        for (const changed of [...this.#listeners])
            changed();
    }
}
