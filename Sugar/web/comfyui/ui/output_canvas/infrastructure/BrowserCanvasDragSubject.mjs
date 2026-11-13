/** Resolve display identity without graph discovery, image admission, or transfer policy. */
export class BrowserCanvasDragSubjects {
    viewport;
    images;
    overview;
    plan;
    /** Bind presentation owners and an explicit current-plan reader for one viewport. */
    constructor(viewport, images, overview, plan) {
        this.viewport = viewport;
        this.images = images;
        this.overview = overview;
        this.plan = plan;
    }
    /** Find exact batch files already represented in the current display plan. */
    item(key) {
        const plan = this.plan();
        const items = plan.view.mode === 'overview'
            ? plan.grid.flatMap((entry) => entry.items)
            : [plan.primary?.item, plan.secondary?.item];
        return items.find((item) => item?.id === key && item.mediaKind === 'image') ?? null;
    }
    /** Refuse latent previews, outgoing decoded replacements, non-image media, and enlarged cameras. */
    resolve(target, event) {
        if (!event)
            return null;
        let element = null;
        let key = null;
        if (this.plan().view.mode === 'overview') {
            const frame = this.overview.draggable(target);
            key = frame?.key ?? null;
            element = frame?.handle ?? null;
        }
        else {
            const bounds = this.viewport.getBoundingClientRect();
            const frame = this.images.draggable(target, {
                x: event.clientX - bounds.left,
                y: event.clientY - bounds.top,
            });
            key = frame?.key ?? null;
            element = frame?.handle ?? null;
        }
        return key && element && this.item(key) ? { key, element } : null;
    }
}
