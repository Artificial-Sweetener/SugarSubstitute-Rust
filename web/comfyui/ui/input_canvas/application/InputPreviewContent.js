import { InputSourcePreview } from './InputSourcePreview.js';
import { DEFAULT_MASK_EDITS, hasMaskEdits } from '../domain/MaskEditSettings.js';
import { InputMaskPreview } from './InputMaskPreview.js';
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
    mode;
    #stopAssociation;
    #stopImageBinding = () => { };
    #imageBinding = null;
    #sourceReader = null;
    #imageReader = null;
    #source = null;
    #image = null;
    #imageReference = '';
    /** Whether no usable associated image exists, so the mask previews its coverage alone. */
    #withoutImage = false;
    #alive = true;
    #sourceSize = -1;
    #edited;
    #captureCohort;
    /** Keep the primary source lifetime independent of relationship and associated image value changes. */
    static reference(binding, entry, maximumSize) {
        return InputSourcePreview.reference(binding, entry, maximumSize);
    }
    /** Observe graph relationships and exact image binding changes without graph discovery from presentation. */
    constructor(binding, previews, entry, maximumSize, publish, report, association, mode) {
        this.binding = binding;
        this.previews = previews;
        this.entry = entry;
        this.maximumSize = maximumSize;
        this.publish = publish;
        this.report = report;
        this.association = association;
        this.mode = mode;
        this.#captureCohort = binding.context?.observesChanges === false ? {} : null;
        this.#edited = new InputMaskPreview(binding.maskEditing, publish, report);
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
        this.#edited.invalidate();
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
        this.#edited.dispose();
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
                this.#coverageAlone();
                return;
            }
            const imageSize = this.maximumSize;
            const reference = JSON.stringify([
                entry.id,
                InputSourcePreview.reference(image, entry, imageSize),
                this.#captureCohort ? this.binding.preview.captureSize(this.#captureSize()) : null,
            ]);
            this.#withoutImage = false;
            if (this.#imageReference !== reference) {
                this.#edited.invalidate();
                this.#imageReference = reference;
                this.#image = null;
                const previous = this.#imageReader;
                this.publish({ kind: 'loading' });
                if (!this.#sourceReader || this.#sourceSize !== this.#captureSize())
                    this.#startSource();
                this.#imageReader = new InputSourcePreview(image, this.previews, entry, imageSize, (value) => {
                    this.#image = value;
                    this.#publishPixels();
                }, this.report, this.#captureCohort, this.#captureCohort ? this.binding.preview.captureSize(this.#captureSize()) : null);
                previous?.dispose();
            }
            else
                this.#imageReader?.refresh();
            if (this.#sourceSize !== this.#captureSize())
                this.#startSource();
            this.#sourceReader?.refresh();
            this.#publishPixels();
        }
        catch (error) {
            this.#coverageAlone();
            this.report(error);
        }
    }
    /** Reuse the primary loader's source path for both ordinary image previews and effective mask pixels. */
    #startSource() {
        this.#edited.invalidate();
        this.#sourceReader?.dispose();
        this.#source = null;
        this.#sourceSize = this.#captureSize();
        this.#sourceReader = new InputSourcePreview(this.binding, this.previews, this.entry, this.#sourceSize, (value) => {
            this.#source = value;
            this.#publishPixels();
        }, this.report, this.binding.loader.kind === 'mask' ? this.#captureCohort : null);
    }
    /** Release the associated image's capture and keep reading the mask's own coverage. */
    #coverageAlone() {
        this.#edited.invalidate();
        this.#stopImageReader();
        this.#withoutImage = true;
        if (!this.#sourceReader || this.#sourceSize !== this.#captureSize())
            this.#startSource();
        else
            this.#sourceReader.refresh();
        this.#publishPixels();
    }
    /** Drop the associated image reader and its last value. */
    #stopImageReader() {
        this.#imageReader?.dispose();
        this.#imageReader = null;
        this.#image = null;
        this.#imageReference = '';
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
        if (this.#withoutImage || image?.kind === 'error') {
            this.#edited.invalidate();
            if (source?.kind === 'pixels')
                this.publish({
                    kind: 'pixels',
                    pixels: source.pixels,
                    background: null,
                    frame: source.canvas?.document ?? null,
                });
            else
                this.publish(source ?? { kind: 'loading' });
            return;
        }
        if (image?.kind !== 'pixels') {
            this.#edited.invalidate();
            this.publish({ kind: 'loading' });
            return;
        }
        if (source?.kind === 'error') {
            this.#edited.invalidate();
            this.publish(source);
            return;
        }
        if (source?.kind !== 'pixels') {
            this.#edited.invalidate();
            this.publish({ kind: 'loading' });
            return;
        }
        if (source.canvas &&
            image.canvas &&
            source.canvas.id === image.canvas.id &&
            (source.canvas.revision !== image.canvas.revision ||
                !sameCanvasSourceDocument(source.canvas.document, image.canvas.document))) {
            this.#edited.invalidate();
            this.publish({ kind: 'loading' });
            return;
        }
        const settings = this.binding.state.edits ?? DEFAULT_MASK_EDITS;
        if (this.association?.editable() && hasMaskEdits(settings)) {
            this.#edited.paint(source.pixels, image.pixels, settings, this.mode(), this.maximumSize, image.canvas?.document ?? null);
            return;
        }
        this.#edited.invalidate();
        this.publish({
            kind: 'pixels',
            pixels: source.pixels,
            background: image.pixels,
            frame: image.canvas?.document ?? null,
        });
    }
    /** Preserve full-resolution coverage for pixel-radius edits while ordinary previews remain display sized. */
    #captureSize() {
        return this.binding.loader.kind === 'mask' &&
            this.association?.editable() &&
            hasMaskEdits(this.binding.state.edits ?? DEFAULT_MASK_EDITS)
            ? 0
            : this.maximumSize;
    }
}
