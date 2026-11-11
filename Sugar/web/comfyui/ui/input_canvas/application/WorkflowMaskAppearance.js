import { workflowMaskKey } from '../domain/WorkflowSurfacePlan.js';
/**
 * Apply visual opacity to every mask of one loader as one undoable canvas step, and highlight a
 * hovered region. The stored opacity also reaches later syncs, so new masks of the loader match.
 */
export class WorkflowMaskAppearance {
    surfaces;
    host;
    store;
    report;
    #hovered = null;
    /** Address masks through the synced state of the canvas that holds them. */
    constructor(surfaces, host, store, report) {
        this.surfaces = surfaces;
        this.host = host;
        this.store = store;
        this.report = report;
    }
    /** Whether the loader currently has masks on a workflow-surface canvas. */
    provisioned(endpoint) {
        return Boolean(this.surfaces.state?.surfaces.some((surface) => surface.masks.some((mask) => mask.endpoint === endpoint)));
    }
    /** Read the loader's synced opacity in 0..1, or null when it has no masks. */
    opacity(endpoint) {
        for (const surface of this.surfaces.state?.surfaces ?? [])
            for (const mask of surface.masks)
                if (mask.endpoint === endpoint)
                    return mask.opacity;
        return null;
    }
    /** Store and apply one loader's visual opacity; values outside 0..1 are rejected. */
    changeOpacity(endpoint, opacity) {
        if (!Number.isFinite(opacity) || opacity < 0 || opacity > 1)
            throw new RangeError('Mask opacity must be between 0 and 1.');
        const state = this.surfaces.state, host = this.host();
        const masks = state?.surfaces.flatMap((surface) => surface.masks
            .filter((mask) => mask.endpoint === endpoint)
            .map((mask) => ({ surface: surface.key, mask: mask.key, color: mask.color, opacity }))) ?? [];
        this.store.write(endpoint, opacity);
        if (!state || !host || host.canvasId !== state.canvasId || !masks.length)
            return;
        this.#send(host, { type: 'appearance', workflow: state.workflow, masks });
    }
    /** Highlight one region of a loader, or clear the highlight with a null entry. */
    hover(endpoint, entry) {
        const state = this.surfaces.state, host = this.host();
        if (!state || !host || host.canvasId !== state.canvasId)
            return;
        const key = entry === null ? null : workflowMaskKey(endpoint, entry);
        const surface = state.surfaces.find((item) => item.masks.some((mask) => mask.endpoint === endpoint)) ?? null;
        if (!surface || key === this.#hovered)
            return;
        this.#hovered = key;
        this.#send(host, { type: 'hover', workflow: state.workflow, surface: surface.key, mask: key });
    }
    /** Report canvas refusals except cancellation. */
    #send(host, command) {
        host.execute(command).then((result) => {
            if (result.kind === 'failure' && result.status !== 'cancelled')
                this.report(new Error(result.message));
        }, (error) => this.report(error));
    }
}
