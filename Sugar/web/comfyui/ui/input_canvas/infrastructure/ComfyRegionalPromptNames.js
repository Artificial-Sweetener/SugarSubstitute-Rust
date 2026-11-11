import { synchronizeRegionalPromptNames } from '../domain/RegionalPromptNames.js';
import { promptField } from './ComfyRegionalPrompts.js';
/**
 * Watch the text of every prompt related to an ordered mask loader and, when one changes, give
 * all prompts of that loader the same separator names.
 *
 * The first reading after a workflow loads only records the authored text, so opening a workflow
 * never modifies it. Every rewrite is committed as the user's own edit would be, inside one
 * native history step; a rewrite caused by a prompt edit joins that edit's step.
 */
export class ComfyRegionalPromptNames {
    regions;
    node;
    root;
    report;
    #stop;
    #texts = new Map();
    #writing = false;
    /** Observe regional relations and commit through the workflow's root graph history. */
    constructor(regions, node, root, report) {
        this.regions = regions;
        this.node = node;
        this.root = root;
        this.report = report;
        this.#stop = regions.subscribe(() => this.#reconcile());
    }
    /** Stop observing and forget recorded prompt text. */
    dispose() {
        this.#stop();
        this.#texts.clear();
    }
    /** Record current prompt text and synchronize every loader whose prompts were edited. */
    #reconcile() {
        if (this.#writing)
            return;
        try {
            const recorded = this.#texts, current = new Map();
            const groups = this.regions.masks().map((mask) => mask.prompts.flatMap((path) => {
                const node = this.node(path), field = node ? promptField(node) : null;
                if (!node || !field)
                    return [];
                current.set(node, field.value);
                return [{ node, widget: field.widget, text: field.value }];
            }));
            this.#texts = current;
            const writes = new Map();
            for (const group of groups) {
                if (group.length < 2)
                    continue;
                const sources = group.map(({ node, text }) => {
                    const previous = recorded.get(node);
                    return { text, previous: previous !== undefined && previous !== text ? previous : null };
                });
                if (!sources.some((source) => source.previous !== null))
                    continue;
                for (const [index, text] of synchronizeRegionalPromptNames(sources).entries()) {
                    const prompt = group[index];
                    if (prompt && text !== null)
                        writes.set(prompt.node, { widget: prompt.widget, text });
                }
            }
            if (writes.size)
                this.#commit(writes);
        }
        catch (error) {
            this.report(error);
        }
    }
    /** Write every rewritten prompt as one native history step without observing its own writes. */
    #commit(writes) {
        for (const [node, { text }] of writes)
            this.#texts.set(node, text);
        const graph = this.root();
        this.#writing = true;
        graph?.beforeChange?.();
        try {
            for (const [node, { widget, text }] of writes) {
                const previous = widget.value;
                widget.value = text;
                widget.callback?.(text);
                if (typeof node.onWidgetChanged === 'function')
                    node.onWidgetChanged.call(node, widget.name, text, previous, widget);
            }
            graph?.setDirtyCanvas?.(true, true);
        }
        finally {
            graph?.afterChange?.();
            this.#writing = false;
        }
    }
}
