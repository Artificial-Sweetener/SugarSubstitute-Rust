import { canvasInputSourceKey } from '../../core/embedded/CanvasRuntime.js';
import { InputPreviewSurface } from './InputPreviewSurface.js';
import { InputPreviewResolutionObserver } from './InputPreviewResolutionObserver.js';
import { readPluginMode } from '../../core/PluginMode.js';
/** Share one mounted preview lifecycle across value, target, settings and geometry refreshes. */
export class InputPreviewRows {
    binding;
    previews;
    report;
    element;
    #empty;
    #entries = new Map();
    #alive = true;
    /** Bind pixels and state through the existing source and interpretation owners. */
    constructor(document, binding, previews, report) {
        this.binding = binding;
        this.previews = previews;
        this.report = report;
        this.element = document.createElement('div');
        this.element.className = 'sc-input-editor__rows';
        this.#empty = new InputPreviewSurface(document, true, () => { });
        this.#empty.element.dataset.inputEmpty = '';
        this.#empty.update('', true, false, true, () => { });
        this.#empty.unavailable('Choose a source');
        this.element.append(this.#empty.element);
    }
    /** Reconcile only this loader's previews, retaining geometry and cached pixels during replacement. */
    refresh() {
        if (!this.#alive)
            return;
        const state = this.binding.state, ids = new Set(state.entries.map((entry) => entry.id));
        const passive = readPluginMode(this.element.ownerDocument);
        this.#empty.element.hidden = state.entries.length > 0;
        const canvas = this.binding.context?.document;
        if (canvas)
            this.#empty.frame(canvas.width, canvas.height);
        for (const [id, row] of this.#entries)
            if (!ids.has(id)) {
                row.stop();
                row.resolution.dispose();
                row.surface.element.remove();
                this.#entries.delete(id);
            }
        for (const [index, entry] of state.entries.entries()) {
            let row = this.#entries.get(entry.id);
            if (!row) {
                row = this.#row(entry);
                this.#entries.set(entry.id, row);
            }
            row.surface.update(entry.label, entry.id === state.selected, this.binding.busy, passive, () => this.#select(entry.id));
            const destination = this.element.querySelectorAll('[data-input-entry]')[index] ?? this.#empty.element;
            if (destination !== row.surface.element)
                this.element.insertBefore(row.surface.element, destination);
            const document = this.binding.context?.document;
            if (document && (entry.source.kind === 'canvas' || !entry.source.file))
                row.surface.frame(document.width, document.height);
            if (this.binding.busy || this.binding.error) {
                row.stop();
                row.stop = () => { };
                row.reference = '';
                row.raw = null;
                row.paintSerial++;
                if (this.binding.error)
                    row.surface.unavailable('Source unavailable');
                else
                    row.surface.loading();
                continue;
            }
            const size = row.resolution.resolution;
            if (!size)
                continue;
            const reference = canvasInputSourceKey(entry.source) +
                ':' +
                String(size) +
                (entry.source.kind === 'file' ? ':' + JSON.stringify(this.binding.preview.settings) : '');
            if (row.reference !== reference) {
                const stop = row.stop;
                row.stop = () => { };
                row.reference = reference;
                row.raw = null;
                row.paintSerial++;
                row.surface.loading();
                this.#observe(entry, row);
                stop();
            }
            else if (row.raw)
                this.#interpret(row, row.raw);
        }
    }
    /** Explain a rejected source intent within its existing selected preview. */
    unavailable(message) {
        (this.#entries.get(this.binding.state.selected ?? '')?.surface ?? this.#empty).unavailable(message);
    }
    /** Release geometry and capture observations before accepting any late interpretation result. */
    dispose() {
        this.#alive = false;
        for (const row of this.#entries.values()) {
            row.stop();
            row.resolution.dispose();
        }
        this.#entries.clear();
        this.element.remove();
    }
    /** Create one literal, stable entry selection surface without native mask mutation intents. */
    #row(entry) {
        const surface = new InputPreviewSurface(this.element.ownerDocument, readPluginMode(this.element.ownerDocument), () => this.#select(entry.id));
        surface.element.dataset.inputEntry = entry.id;
        return {
            surface,
            resolution: new InputPreviewResolutionObserver(surface.frameElement, () => this.refresh()),
            reference: '',
            raw: null,
            interpretation: '',
            paintSerial: 0,
            stop: () => { },
        };
    }
    /** Preserve ordinary browser entry selection without attaching actions to plugin previews. */
    #select(id) {
        try {
            this.binding.select(id);
        }
        catch (error) {
            this.report(error);
        }
    }
    /** Read file masks from their loader or subscribe to raw canvas data without per-edit uploads. */
    #observe(entry, row) {
        if (entry.source.kind === 'canvas') {
            row.stop = this.previews.observe(entry.source, (value) => {
                if (value.kind === 'pixels') {
                    row.raw = value.pixels;
                    row.interpretation = '';
                    this.#interpret(row, value.pixels);
                }
                else {
                    row.raw = null;
                    row.interpretation = '';
                    row.paintSerial++;
                    if (value.kind === 'error')
                        row.surface.unavailable(value.unavailable ? value.message : 'Preview unavailable');
                    else
                        row.surface.loading();
                }
            }, this.binding.preview.captureSize(row.resolution.resolution), this.binding.loader.kind === 'image' ? 'image' : 'raw');
            return;
        }
        if (!entry.source.file) {
            row.surface.unavailable('Choose a file');
            return;
        }
        let alive = true;
        row.stop = () => {
            alive = false;
        };
        void this.binding.preview
            .file(entry.source.file, row.resolution.resolution)
            .then((pixels) => {
            if (alive && this.#alive)
                this.#paint(row, pixels);
        })
            .catch((error) => {
            if (alive && this.#alive) {
                row.surface.unavailable('Preview unavailable');
                this.report(error);
            }
        });
    }
    /** Fence asynchronous native interpretation so superseded settings and retired rows never repaint. */
    #interpret(row, pixels) {
        const settings = JSON.stringify([this.binding.preview.settings, row.resolution.resolution]);
        if (settings === row.interpretation)
            return;
        row.interpretation = settings;
        const serial = ++row.paintSerial;
        row.surface.loading();
        void this.binding.preview
            .canvas(pixels, row.resolution.resolution)
            .then((mask) => {
            if (this.#alive &&
                row.paintSerial === serial &&
                row.raw === pixels &&
                this.#entries.has(row.surface.element.dataset.inputEntry ?? ''))
                this.#paint(row, mask);
        })
            .catch((error) => {
            if (this.#alive &&
                row.paintSerial === serial &&
                row.raw === pixels &&
                this.#entries.get(row.surface.element.dataset.inputEntry ?? '') === row) {
                row.surface.unavailable('Preview unavailable');
                this.report(error);
            }
        });
    }
    /** Paint the consumer representation in its full source frame without changing captured pixels. */
    #paint(row, pixels) {
        try {
            const entry = this.binding.state.entries.find((entry) => entry.id === row.surface.element.dataset.inputEntry);
            row.surface.paint(pixels, entry?.source.kind === 'canvas' ? (this.binding.context?.document ?? null) : null);
        }
        catch (error) {
            row.surface.unavailable('Preview unavailable');
            this.report(error);
        }
    }
}
