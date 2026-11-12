/** Retain one active request and the latest lightweight intent rather than processing every slider event. */
export class InputMaskPreview {
    editing;
    publish;
    report;
    #serial = 0;
    #active = false;
    #latest = null;
    #requested = null;
    #alive = true;
    #completed = null;
    /** Share the same processing port as immutable generation preparation. */
    constructor(editing, publish, report) {
        this.editing = editing;
        this.publish = publish;
        this.report = report;
    }
    /** Preserve the mounted preview while coalescing edits from unchanged captured source pixels. */
    paint(mask, image, settings, mode, maximumSize, frame) {
        const previous = this.#requested;
        if (previous?.mask === mask &&
            previous.image === image &&
            previous.mode === mode &&
            previous.maximumSize === maximumSize &&
            previous.frame?.width === frame?.width &&
            previous.frame?.height === frame?.height &&
            JSON.stringify(previous.settings) === JSON.stringify(settings))
            return;
        const intent = { mask, image, settings, mode, maximumSize, frame, serial: ++this.#serial };
        this.#latest = this.#requested = intent;
        if (this.#completed?.mask === mask && this.#completed.key === JSON.stringify(settings)) {
            this.#latest = null;
            this.#present(intent, this.#completed);
        }
        else {
            this.publish({ kind: 'loading' });
            this.#drain();
        }
    }
    /** Fence pending results when an unedited or unavailable source takes over the same surface. */
    invalidate() {
        this.#serial++;
        this.#latest = this.#requested = null;
    }
    /** Retire presentation before accepting any late processing response. */
    dispose() {
        this.#alive = false;
        this.invalidate();
    }
    /** Process the newest admitted intent after each response, without timer-driven retries. */
    #drain() {
        if (this.#active || !this.#alive || !this.#latest)
            return;
        const intent = this.#latest;
        this.#latest = null;
        this.#active = true;
        void this.editing
            .prepare(intent.mask, intent.settings, 'preview')
            .then((edited) => {
            if (!this.#alive)
                return;
            const completed = {
                mask: intent.mask,
                key: JSON.stringify(intent.settings),
                edited,
                placement: {
                    bounds: edited.bounds,
                    frame: { width: intent.mask.width, height: intent.mask.height },
                },
            };
            this.#completed = completed;
            const current = this.#requested;
            if (current?.mask === intent.mask && JSON.stringify(current.settings) === completed.key) {
                this.#latest = null;
                this.#present(current, completed);
            }
        })
            .catch((error) => {
            if (!this.#alive || intent.serial !== this.#serial)
                return;
            this.publish({
                kind: 'error',
                message: error instanceof Error ? error.message : 'Mask preview unavailable',
            });
            this.report(error);
        })
            .finally(() => {
            this.#active = false;
            this.#drain();
        });
    }
    /** Frame cached display pixels in a viewport instead of making a cropped image for presentation. */
    #present(intent, completed) {
        if (intent.image.width > intent.maximumSize || intent.image.height > intent.maximumSize)
            throw new Error('The image preview exceeds its display limit.');
        const { edited, placement } = completed;
        const input = intent.mode === 'input' && edited.cropped;
        this.publish({
            kind: 'pixels',
            pixels: edited.mask,
            background: intent.image,
            frame: input ? { width: edited.bounds.width, height: edited.bounds.height } : intent.frame,
            placement,
            outline: edited.cropped && !input,
            ...(input ? { viewport: placement } : {}),
        });
    }
}
