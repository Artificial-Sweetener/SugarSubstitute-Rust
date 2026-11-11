import { CANVAS_IMAGE_BYTE_LIMIT } from './CanvasImageBudget.mjs';
/** Own temporary navigation staging without caching pixels or changing selection policy. */
export class CanvasImagePreparation {
    resources;
    #stop;
    #base = [];
    #preview = null;
    #current = null;
    #sequence = 0;
    #disposed = false;
    /** Observe the existing resource authority instead of creating a second decoder or budget. */
    constructor(resources) {
        this.resources = resources;
        this.#stop = resources.subscribe(() => this.#settle());
    }
    /** Merge current interests with a bounded pending cohort until adoption completes. */
    update(images, retry = false, preview = null) {
        if (this.#disposed)
            return;
        const changed = signature(images) !== signature(this.#base) || preview !== null;
        if (changed && this.#current?.phase !== 'committing')
            this.#retire();
        this.#base = images.filter((image) => !image.input.preview);
        this.#preview = preview;
        this.resources.update(this.#merge(images), retry, preview);
    }
    /** Stage only final pixels that fit the existing visible-slot and byte admission rules. */
    prepare(images) {
        this.cancel();
        if (this.#disposed ||
            this.#preview ||
            this.resources.diagnostics().estimatedBytes > CANVAS_IMAGE_BYTE_LIMIT ||
            !this.#base.length ||
            images.some((image) => image.input.preview))
            return null;
        const sequence = ++this.#sequence;
        const staged = images
            .filter((image) => !this.#base.some((base) => base.input.key === image.input.key && base.input.role === image.input.role))
            .map((image, index) => ({ slot: `preparation:${sequence}:${index}`, input: image.input }));
        const combined = [...this.#base, ...staged];
        if (!this.resources.slotsFit(combined))
            return null;
        let resolve = () => { };
        const ready = new Promise((complete) => {
            resolve = complete;
        });
        const request = {
            images: staged,
            resolve,
            phase: 'loading',
            unavailable: false,
        };
        this.#current = request;
        this.resources.update(combined);
        this.#settle();
        return {
            ready,
            get unavailable() {
                return request.unavailable;
            },
            commit: (apply) => {
                if (this.#current !== request || request.phase !== 'ready')
                    return false;
                request.phase = 'committing';
                try {
                    return apply();
                }
                finally {
                    this.cancel();
                }
            },
        };
    }
    /** Cancel superseded navigation without releasing the active presentation. */
    cancel() {
        if (!this.#current)
            return;
        this.#retire();
        this.resources.update(this.#base, false, this.#preview);
    }
    /** Expose pending navigation to the existing delayed loading feedback. */
    get pending() {
        return this.#current?.phase === 'loading' || this.#current?.phase === 'ready';
    }
    /** Remove observation and all temporary interests at the surface lifetime boundary. */
    dispose() {
        this.cancel();
        this.#disposed = true;
        this.#stop();
        this.#base = [];
        this.#preview = null;
    }
    /** Exclude staged duplicates when their ordinary destination is being adopted. */
    #merge(images) {
        return [
            ...images,
            ...(this.#current?.images.filter((staged) => !images.some((image) => image.input.key === staged.input.key && image.input.role === staged.input.role)) ?? []),
        ];
    }
    /** Resolve a complete cohort or relinquish a failed/hidden/blocked preparation. */
    #settle() {
        const request = this.#current;
        if (!request || request.phase !== 'loading')
            return;
        const snapshots = this.resources.read();
        const staged = request.images.map((image) => snapshots.find((snapshot) => snapshot.slot === image.slot));
        if (staged.some((snapshot) => !snapshot || snapshot.status.state === 'error' || snapshot.status.state === 'budget')) {
            request.unavailable = staged.some((snapshot) => snapshot?.status.state === 'error' || snapshot?.status.state === 'budget');
            this.cancel();
            return;
        }
        if (staged.every((snapshot) => snapshot?.frame)) {
            request.phase = 'ready';
            request.resolve(true);
        }
    }
    /** Resolve canceled waiters before any reentrant resource notification. */
    #retire() {
        const request = this.#current;
        if (!request)
            return;
        this.#current = null;
        request.phase = 'finished';
        request.resolve(false);
    }
}
/** Compare routing interest without retaining mutable payloads or platform handles. */
function signature(images) {
    return JSON.stringify(images.map((image) => [image.slot, image.input.key, image.input.lane, image.input.role]));
}
