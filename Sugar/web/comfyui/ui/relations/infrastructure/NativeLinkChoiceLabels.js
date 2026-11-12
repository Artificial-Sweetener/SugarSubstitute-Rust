/** Own native label decorations while shared layout batches their measurement. */
export class NativeLinkChoiceLabels {
    root;
    layout;
    measure;
    #resize;
    #mutations;
    #labels = [];
    #selected = null;
    #disposed = false;
    #context = null;
    /** Observe native select replacement and row resizing without replacing the host control. */
    constructor(root, layout, measure) {
        this.root = root;
        this.layout = layout;
        this.measure = measure;
        this.#resize = typeof ResizeObserver === 'function' ? new ResizeObserver(this.#schedule) : null;
        this.#resize?.observe(root);
        this.#mutations = new MutationObserver(this.#schedule);
        this.#mutations.observe(root, { childList: true, subtree: true, characterData: true });
    }
    /** Derive one stable width preference from the complete candidate list. */
    update(labels) {
        this.#labels = labels;
        this.#schedule();
    }
    /** Release observers and presentation annotations with the native mount. */
    dispose() {
        this.#disposed = true;
        this.layout.release(this.#measure);
        this.#resize?.disconnect();
        this.#mutations.disconnect();
        this.#restore();
        this.root.style.removeProperty('--sc-link-preferred-width');
    }
    /** Coalesce native replacement and resizing into one presentation pass per frame. */
    #schedule = () => {
        if (!this.#disposed)
            this.layout.invalidate(this.#measure);
    };
    /** Surrender the route prefix before allowing the native text box to elide the cube name. */
    #measure = () => {
        const selected = this.root.querySelector('[data-testid="widget-select-default-trigger"] > span');
        if (!selected)
            return null;
        const font = selected.hasAttribute('data-sc-link-display')
            ? selected.style.getPropertyValue('--sc-link-font')
            : getComputedStyle(selected).font;
        const available = selected.clientWidth;
        const preferred = `${Math.ceil(Math.max(60, ...this.#labels.map((label) => this.#width(label, font) + 44)))}px`;
        const text = selected.textContent ?? '';
        const slash = text.lastIndexOf('/');
        const display = text.startsWith('🔗 ') && slash > 3 && this.#width(text, font) > available
            ? `🔗 ${text.slice(slash + 1).trim()}`
            : null;
        return () => {
            if (this.#disposed)
                return;
            if (this.#selected !== selected)
                this.#restore();
            this.#selected = selected;
            if (this.root.style.getPropertyValue('--sc-link-preferred-width') !== preferred)
                this.root.style.setProperty('--sc-link-preferred-width', preferred);
            if (selected.title !== text)
                selected.title = text;
            if (display !== null) {
                if (selected.style.getPropertyValue('--sc-link-font') !== font)
                    selected.style.setProperty('--sc-link-font', font);
                if (selected.dataset.scLinkDisplay !== display)
                    selected.dataset.scLinkDisplay = display;
            }
            else if (selected.hasAttribute('data-sc-link-display')) {
                selected.removeAttribute('data-sc-link-display');
                selected.style.removeProperty('--sc-link-font');
            }
        };
    };
    /** Measure with the native font rather than treating route length as pixel width. */
    #width(text, font) {
        if (this.measure)
            return this.measure(text, font);
        this.#context ??= this.root.ownerDocument.createElement('canvas').getContext('2d');
        if (!this.#context)
            throw new Error('Native link label font measurement is unavailable.');
        this.#context.font = font;
        return this.#context.measureText(text).width;
    }
    /** Restore decorations when Comfy replaces its selected label element. */
    #restore() {
        this.#selected?.removeAttribute('data-sc-link-display');
        this.#selected?.style.removeProperty('--sc-link-font');
        this.#selected?.removeAttribute('title');
        this.#selected = null;
    }
}
