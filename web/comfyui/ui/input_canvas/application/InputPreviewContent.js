import { InputSourcePreview } from './InputSourcePreview.js';
import { sameCanvasSourceDocument, } from '../../core/embedded/CanvasRuntime.js';
/** Consume reusable associations while source acquisition stays with the shared loader preview owner. */
export class InputPreviewContent {
    binding;
    previews;
    entry;
    maximumSize;
    publish;
    report;
    association;
    #stopAssociation;
    #stopImageBinding = () => { };
    #imageBinding = null;
    #sourceReader = null;
    #imageReader = null;
    #source = null;
    #image = null;
    #imageReference = '';
    #alive = true;
    /** Keep the primary source lifetime independent of relationship and associated image value changes. */
    static reference(binding, entry, maximumSize) {
        return InputSourcePreview.reference(binding, entry, maximumSize);
    }
    /** Observe graph relationships and exact image binding changes without graph discovery from presentation. */
    constructor(binding, previews, entry, maximumSize, publish, report, association) {
        this.binding = binding;
        this.previews = previews;
        this.entry = entry;
        this.maximumSize = maximumSize;
        this.publish = publish;
        this.report = report;
        this.association = association;
        this.#stopAssociation =
            binding.loader.kind === 'mask'
                ? (association?.subscribe(() => this.#refreshImage()) ?? (() => { }))
                : () => { };
        if (binding.loader.kind === 'mask')
            this.#refreshImage();
        else
            this.#startSource();
    }
    /** Apply settings changes through existing source work while related source choices retain their own owner. */
    refresh() {
        if (!this.#alive)
            return;
        if (this.binding.loader.kind === 'mask')
            this.#refreshImage();
        else
            this.#sourceReader?.refresh();
    }
    /** Preserve black for missing image context while valid image previews retain concise source feedback. */
    unavailable(message) {
        this.publish(this.binding.loader.kind === 'mask' && this.#image?.kind !== 'pixels'
            ? { kind: 'black' }
            : { kind: 'error', message });
    }
    /** Release relationships and source observers before accepting any late source results. */
    dispose() {
        this.#alive = false;
        this.#stopAssociation();
        this.#stopImageBinding();
        this.#sourceReader?.dispose();
        this.#imageReader?.dispose();
    }
    /** Follow the associated loader's selected entry and actual source availability. */
    #refreshImage() {
        if (!this.#alive)
            return;
        try {
            const image = this.association?.read() ?? null;
            if (image !== this.#imageBinding) {
                this.#stopImageBinding();
                this.#imageBinding = image;
                this.#stopImageBinding = image?.subscribe(() => this.#refreshImage()) ?? (() => { });
                this.#imageReference = '';
            }
            const entry = image?.state.entries.find((entry) => entry.id === image.state.selected);
            if (!image ||
                image.loader.kind !== 'image' ||
                image.busy ||
                image.error ||
                !entry ||
                InputSourcePreview.unavailable(image, entry)) {
                this.#black();
                return;
            }
            const reference = JSON.stringify([
                entry.id,
                InputSourcePreview.reference(image, entry, this.maximumSize),
            ]);
            if (this.#imageReference !== reference) {
                this.#imageReference = reference;
                this.#image = null;
                const previous = this.#imageReader;
                this.publish({ kind: 'loading' });
                if (!this.#sourceReader)
                    this.#startSource();
                this.#imageReader = new InputSourcePreview(image, this.previews, entry, this.maximumSize, (value) => {
                    this.#image = value;
                    this.#publishPixels();
                }, this.report);
                previous?.dispose();
            }
            else
                this.#imageReader?.refresh();
            this.#sourceReader?.refresh();
        }
        catch (error) {
            this.#black();
            this.report(error);
        }
    }
    /** Reuse the primary loader's source path for both ordinary image previews and effective mask pixels. */
    #startSource() {
        this.#sourceReader = new InputSourcePreview(this.binding, this.previews, this.entry, this.maximumSize, (value) => {
            this.#source = value;
            this.#publishPixels();
        }, this.report);
    }
    /** Release unnecessary capture work when no real image is selected and keep the preview solid black. */
    #black() {
        this.#sourceReader?.dispose();
        this.#imageReader?.dispose();
        this.#sourceReader = this.#imageReader = null;
        this.#source = this.#image = null;
        this.#imageReference = '';
        this.publish({ kind: 'black' });
    }
    /** Overlay only the associated image, waiting for matching revisions when both sources share a canvas. */
    #publishPixels() {
        if (!this.#alive)
            return;
        const source = this.#source;
        if (this.binding.loader.kind === 'image') {
            if (source?.kind === 'pixels')
                this.publish({
                    kind: 'pixels',
                    pixels: source.pixels,
                    background: null,
                    frame: source.canvas?.document ?? null,
                });
            else if (source)
                this.publish(source);
            return;
        }
        const image = this.#image;
        if (image?.kind === 'error') {
            this.publish({ kind: 'black' });
            return;
        }
        if (image?.kind !== 'pixels') {
            this.publish({ kind: 'loading' });
            return;
        }
        if (source?.kind === 'error') {
            this.publish(source);
            return;
        }
        if (source?.kind !== 'pixels') {
            this.publish({ kind: 'loading' });
            return;
        }
        if (source.canvas &&
            image.canvas &&
            source.canvas.id === image.canvas.id &&
            (source.canvas.revision !== image.canvas.revision ||
                !sameCanvasSourceDocument(source.canvas.document, image.canvas.document))) {
            this.publish({ kind: 'loading' });
            return;
        }
        this.publish({
            kind: 'pixels',
            pixels: source.pixels,
            background: image.pixels,
            frame: image.canvas?.document ?? null,
        });
    }
}
