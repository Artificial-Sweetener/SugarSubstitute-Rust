/** Own suppression only for top-level native nodes absent from the masonry projection. */
export class WorkflowMasonryGraphReveal {
    #nodes = new Map();
    #hidden = false;
    /** Reconcile known graph presentation membership at topology lifecycles, never popup activity. */
    reconcile(container, sections) {
        const participating = new Set(sections.flatMap((section) => section.cube
            ? [String(section.cube.id)]
            : section.cards.filter((card) => card.visible).map((card) => String(card.node.id))));
        const root = container.closest('#graph-canvas-container') ?? container;
        const nodes = new Set([...root.querySelectorAll('.lg-node[data-node-id]')].filter((element) => !element.parentElement?.closest('.lg-node, .sugarcubes-workflow-masonry') &&
            !participating.has(element.dataset.nodeId ?? '')));
        for (const [element, saved] of this.#nodes)
            if (!nodes.has(element)) {
                this.#restore(element, saved);
                this.#nodes.delete(element);
            }
        for (const element of nodes) {
            if (this.#nodes.has(element))
                continue;
            this.#nodes.set(element, {
                value: element.style.getPropertyValue('opacity'),
                priority: element.style.getPropertyPriority('opacity'),
                opacity: getComputedStyle(element).opacity || '1',
            });
            if (this.#hidden)
                element.style.setProperty('opacity', '0');
        }
    }
    /** Supply concurrent fades while keeping hidden nodes suppressed before an exit can paint. */
    frames(entering) {
        const requests = [...this.#nodes].map(([element, saved]) => ({
            element,
            frames: [
                { opacity: getComputedStyle(element).opacity || (this.#hidden ? '0' : saved.opacity) },
                { opacity: entering ? '0' : saved.opacity },
            ],
        }));
        this.#hidden = entering;
        // Important native declarations would override the fade; restore their priority at teardown.
        for (const [element, saved] of this.#nodes)
            element.style.setProperty('opacity', entering ? '0' : saved.opacity);
        return requests;
    }
    /** Release every retained node and restore exact native inline opacity on teardown. */
    dispose() {
        for (const [element, saved] of this.#nodes)
            this.#restore(element, saved);
        this.#nodes.clear();
    }
    /** Preserve host priorities and the absence of inline styling. */
    #restore(element, saved) {
        if (saved.value)
            element.style.setProperty('opacity', saved.value, saved.priority);
        else
            element.style.removeProperty('opacity');
    }
}
