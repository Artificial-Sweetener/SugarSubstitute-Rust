import { canvasSourceUnavailable } from '../domain/CanvasSourceAvailability.js';
import { canvasInputSourceKey, } from '../../core/embedded/CanvasRuntime.js';
/** Own asynchronous source work independently of preview geometry and relationships. */
export class InputSourcePreview {
    binding;
    entry;
    maximumSize;
    publish;
    report;
    #stop;
    #source = null;
    #interpretation = '';
    #serial = 0;
    #alive = true;
    /** Keep availability policy shared by source admission and lifetime invalidation. */
    static unavailable(binding, entry) {
        return entry.source.kind === 'canvas'
            ? canvasSourceUnavailable(entry.source, binding.context)
            : entry.source.file
                ? null
                : 'Choose a file';
    }
    /** Reuse source work across interpretation edits while file codecs keep their authored settings. */
    static reference(binding, entry, maximumSize) {
        return JSON.stringify([
            canvasInputSourceKey(entry.source),
            maximumSize,
            this.unavailable(binding, entry),
            entry.source.kind === 'file' ? binding.preview.settings : null,
        ]);
    }
    /** Read canvas or file data through the same source path used by the loader's own preview. */
    constructor(binding, previews, entry, maximumSize, publish, report) {
        this.binding = binding;
        this.entry = entry;
        this.maximumSize = maximumSize;
        this.publish = publish;
        this.report = report;
        this.#stop = () => { };
        const reason = InputSourcePreview.unavailable(binding, entry);
        if (reason) {
            publish({ kind: 'error', message: reason });
            return;
        }
        if (entry.source.kind === 'canvas') {
            this.#stop = previews.observe(entry.source, (value) => {
                this.#source = value.kind === 'pixels' ? value : null;
                this.#interpretation = '';
                this.#serial++;
                if (value.kind === 'pixels')
                    this.refresh();
                else
                    publish(value.kind === 'loading'
                        ? value
                        : {
                            kind: 'error',
                            message: value.unavailable ? value.message : 'Preview unavailable',
                        });
            }, binding.preview.captureSize(maximumSize), binding.loader.kind === 'image' ? 'image' : 'raw');
        }
        else if (entry.source.file) {
            publish({ kind: 'loading' });
            void binding.preview
                .file(entry.source.file, maximumSize)
                .then((pixels) => {
                if (this.#alive)
                    publish({ kind: 'pixels', pixels, canvas: null });
            })
                .catch((error) => this.#failed(error));
        }
    }
    /** Reinterpret the current native snapshot without repeating capture or file staging. */
    refresh() {
        const source = this.#source;
        if (!this.#alive || !source || this.entry.source.kind !== 'canvas')
            return;
        const settings = JSON.stringify(this.binding.preview.settings);
        if (this.#interpretation === settings)
            return;
        this.#interpretation = settings;
        const serial = ++this.#serial, canvasId = this.entry.source.canvasId;
        this.publish({ kind: 'loading' });
        void this.binding.preview
            .canvas(source.pixels, this.maximumSize)
            .then((pixels) => {
            if (this.#alive && this.#source === source && this.#serial === serial)
                this.publish({
                    kind: 'pixels',
                    pixels,
                    canvas: { id: canvasId, document: source.document, revision: source.revision },
                });
        })
            .catch((error) => {
            if (this.#alive && this.#source === source && this.#serial === serial)
                this.#failed(error);
        });
    }
    /** Retire pending results before releasing the shared source subscription. */
    dispose() {
        this.#alive = false;
        this.#stop();
    }
    /** Preserve codec failure context while publishing concise source feedback. */
    #failed(error) {
        if (!this.#alive)
            return;
        this.publish({ kind: 'error', message: 'Preview unavailable' });
        this.report(error);
    }
}
