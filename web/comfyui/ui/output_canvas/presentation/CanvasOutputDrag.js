/** Retain at most one encoded image and retire it on navigation, hiding, leaving, or disposal. */
export class CanvasOutputDrag {
    root;
    options;
    #preparation = null;
    #disposed = false;
    /** Delegate only local image gestures without observing host DOM or owning image decoding. */
    constructor(root, options) {
        this.root = root;
        this.options = options;
        root.addEventListener('pointerover', this.#hover);
        root.addEventListener('pointermove', this.#hover);
        root.addEventListener('pointerleave', this.#leave);
        root.addEventListener('dragstart', this.#start);
        root.addEventListener('dragend', this.#leave);
    }
    /** Admit native dragging only when the current fitted image's complete file is ready. */
    allows(event) {
        if (this.#disposed || !this.options.accepts() || !this.#preparation?.payload)
            return false;
        const subject = this.options.resolve(event.target, event);
        return (subject?.key === this.#preparation.subject.key &&
            subject.element === this.#preparation.subject.element);
    }
    /** Invalidate bytes when their displayed identity or fitted footprint changes. */
    reconcile() {
        const prepared = this.#preparation;
        if (!prepared)
            return;
        const current = this.options.accepts()
            ? this.options.resolve(prepared.subject.element, prepared.event)
            : null;
        if (current?.key !== prepared.subject.key || current.element !== prepared.subject.element)
            this.#clear();
    }
    /** Discard a prepared representation when shared transfer preferences change. */
    invalidate() {
        this.#clear();
    }
    /** Cancel finite preparation and unregister exactly this surface's listeners. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#clear();
        this.root.removeEventListener('pointerover', this.#hover);
        this.root.removeEventListener('pointermove', this.#hover);
        this.root.removeEventListener('pointerleave', this.#leave);
        this.root.removeEventListener('dragstart', this.#start);
        this.root.removeEventListener('dragend', this.#leave);
    }
    /** Prepare only the pointer's current exact image, never a whole workflow or batch. */
    #hover = (event) => {
        if (this.#disposed || event.pointerType === 'touch')
            return;
        const subject = this.options.accepts() ? this.options.resolve(event.target, event) : null;
        if (subject?.key === this.#preparation?.subject.key &&
            subject?.element === this.#preparation?.subject.element) {
            if (this.#preparation)
                this.#preparation.event = event;
            return;
        }
        this.#clear();
        if (!subject)
            return;
        const preparation = {
            subject,
            controller: new AbortController(),
            event,
            payload: null,
        };
        this.#preparation = preparation;
        subject.element.draggable = false;
        void this.#prepare(preparation);
    };
    /** Fence asynchronous success and failure to the single current image lease. */
    async #prepare(preparation) {
        try {
            const payload = await this.options.prepare(preparation.subject.key, preparation.controller.signal);
            this.reconcile();
            if (this.#preparation === preparation && !preparation.controller.signal.aborted) {
                preparation.payload = payload;
                preparation.subject.element.draggable = payload !== null;
            }
        }
        catch (error) {
            this.reconcile();
            if (this.#preparation !== preparation || preparation.controller.signal.aborted)
                return;
            preparation.controller.abort();
            preparation.subject.element.draggable = false;
            this.options.report(error);
        }
    }
    /** Fill the browser-owned drag store synchronously with exact preferred bytes and a download URL. */
    #start = (event) => {
        this.reconcile();
        const preparation = this.#preparation;
        const subject = this.options.accepts() ? this.options.resolve(event.target, event) : null;
        const payload = preparation?.payload;
        if (!payload ||
            !event.dataTransfer ||
            subject?.key !== preparation?.subject.key ||
            subject?.element !== preparation?.subject.element) {
            event.preventDefault();
            return;
        }
        event.stopPropagation();
        try {
            // Chromium supplies an image named "view" before dragstart; retain only the selected representation.
            event.dataTransfer.items.clear();
            event.dataTransfer.effectAllowed = 'copy';
            event.dataTransfer.items.add(payload.file);
            event.dataTransfer.setData('DownloadURL', `${payload.file.type}:${payload.file.name}:${payload.url}`);
            event.dataTransfer.setData('text/uri-list', payload.url);
        }
        catch (error) {
            event.preventDefault();
            this.#clear();
            this.options.report(error);
        }
    };
    /** Release retained bytes without releasing the resource owner's decoded image. */
    #clear() {
        const preparation = this.#preparation;
        this.#preparation = null;
        if (!preparation)
            return;
        preparation.controller.abort();
        preparation.subject.element.draggable = false;
        preparation.payload = null;
    }
    /** End hover or completed native dragging without scheduling idle work. */
    #leave = () => this.#clear();
}
