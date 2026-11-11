/**
 * Link a mask row, its canvas mask and the related prompt separators while one of them is hovered.
 *
 * Exactly one hover source is current. A clear from a source that is no longer current is
 * ignored, so moving the pointer from one view onto another never clears the newer hover. A
 * prompt related to several ordered loaders links to the first of them, in graph order. A hover
 * whose loader leaves the workflow, as when another workflow loads, is cleared.
 */
export class WorkflowRegionalHover {
    regions;
    canvas;
    report;
    #highlights = new Set();
    #listeners = new Set();
    #stop;
    #source = null;
    #shown = null;
    /** Resolve relations through the regional owner and canvas highlight through appearance. */
    constructor(regions, canvas, report) {
        this.regions = regions;
        this.canvas = canvas;
        this.report = report;
        this.#stop = regions.subscribe(() => {
            if (this.#shown && !regions.mask(this.#shown.endpoint))
                this.clear();
        });
    }
    /** Hover a mask row's region, or clear the row's hover with a null entry. */
    hoverMask(endpoint, entry) {
        const source = 'mask:' + endpoint;
        if (!this.#accept(source, entry))
            return;
        const mask = this.regions.mask(endpoint), ordinal = mask && entry !== null ? mask.entries.indexOf(entry) : -1;
        this.#show({
            endpoint,
            entry,
            prompts: mask && ordinal >= 0 ? mask.prompts : [],
            ordinal,
        });
    }
    /** Hover separator `ordinal` of the prompt at this workflow path, or clear it with null. */
    hoverPrompt(prompt, ordinal) {
        const source = 'prompt:' + prompt;
        if (!this.#accept(source, ordinal))
            return;
        const mask = this.regions.forPrompt(prompt)[0];
        if (!mask || ordinal === null) {
            this.clear();
            return;
        }
        this.#show({
            endpoint: mask.endpoint,
            entry: mask.entries[ordinal] ?? null,
            prompts: mask.prompts,
            ordinal,
        });
    }
    /** The entry shown as hovered among this loader's rows, or null. */
    hovered(endpoint) {
        return this.#shown?.endpoint === endpoint ? this.#shown.entry : null;
    }
    /** Clear the current hover from every linked view. */
    clear() {
        this.#source = null;
        const shown = this.#shown;
        if (!shown)
            return;
        this.#shown = null;
        this.canvas.hover(shown.endpoint, null);
        this.#highlight(shown.prompts, null);
        this.#changed();
    }
    /** Attach prompt editors; they first show the current hover, if any. */
    connect(highlight) {
        this.#highlights.add(highlight);
        const shown = this.#shown;
        if (shown?.prompts.length)
            highlight.highlight(shown.prompts, shown.ordinal);
        return () => {
            this.#highlights.delete(highlight);
        };
    }
    /** Clear the hover and stop following regional relations. */
    dispose() {
        this.clear();
        this.#stop();
        this.#highlights.clear();
        this.#listeners.clear();
    }
    /** Observe changes to which rows are shown as hovered. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Make a hover source current, or clear for its own null; reject stale clears. */
    #accept(source, value) {
        if (value === null) {
            if (this.#source === source)
                this.clear();
            return false;
        }
        this.#source = source;
        return true;
    }
    /** Replace the shown hover in every linked view. */
    #show(next) {
        const previous = this.#shown;
        if (previous?.endpoint === next.endpoint &&
            previous.entry === next.entry &&
            previous.ordinal === next.ordinal &&
            previous.prompts === next.prompts)
            return;
        if (previous) {
            if (previous.endpoint !== next.endpoint)
                this.canvas.hover(previous.endpoint, null);
            this.#highlight(previous.prompts, null);
        }
        this.#shown = next;
        this.canvas.hover(next.endpoint, next.entry);
        this.#highlight(next.prompts, next.ordinal);
        this.#changed();
    }
    /** Highlight prompts through every attached editor host, isolating their failures. */
    #highlight(prompts, ordinal) {
        if (!prompts.length)
            return;
        for (const highlight of this.#highlights)
            try {
                highlight.highlight(prompts, ordinal);
            }
            catch (error) {
                this.report(error);
            }
    }
    /** Notify row observers, isolating their failures. */
    #changed() {
        for (const listener of this.#listeners)
            try {
                listener();
            }
            catch (error) {
                this.report(error);
            }
    }
}
