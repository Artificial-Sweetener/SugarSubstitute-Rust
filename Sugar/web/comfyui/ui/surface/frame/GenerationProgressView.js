/** Preserve two compact transparent tracks with accessible progress values. */
export class GenerationProgressView {
    element;
    #workflow;
    #sampler;
    /** Build retained progress nodes without labels occupying the menu row. */
    constructor(document) {
        this.element = document.createElement('div');
        this.element.className = 'sc-generation-progress';
        this.element.hidden = true;
        this.#workflow = this.#bar(document, 'workflow', 'Workflow progress');
        this.#sampler = this.#bar(document, 'sampler', 'Sampler progress');
        this.element.append(this.#workflow, this.#sampler);
    }
    /** Set widths only from the validated execution projection and retain bar identity. */
    update(snapshot) {
        this.element.hidden = !snapshot.visible;
        for (const [bar, value] of [
            [this.#workflow, snapshot.workflow],
            [this.#sampler, snapshot.sampler],
        ]) {
            const percent = Number.isFinite(value) ? Math.max(0, Math.min(100, value)) : 0;
            bar.setAttribute('aria-valuenow', String(Math.round(percent)));
            bar.style.width = `${String(percent)}%`;
        }
    }
    /** Create one transparent source-matched track with an explicit accessible name. */
    #bar(document, kind, label) {
        const bar = document.createElement('div');
        bar.className = `sc-generation-progress__${kind}`;
        bar.setAttribute('role', 'progressbar');
        bar.setAttribute('aria-label', label);
        bar.setAttribute('aria-valuemin', '0');
        bar.setAttribute('aria-valuemax', '100');
        return bar;
    }
}
