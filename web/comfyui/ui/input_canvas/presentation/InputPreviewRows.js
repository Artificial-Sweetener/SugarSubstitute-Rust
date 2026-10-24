import { InputPreviewContent } from '../application/InputPreviewContent.js';
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
                row.content?.dispose();
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
            if (document &&
                ((this.binding.loader.kind === 'mask' &&
                    this.binding.context?.targets.includes('composition')) ||
                    entry.source.kind === 'canvas' ||
                    !entry.source.file))
                row.surface.frame(document.width, document.height);
            if (this.binding.busy || this.binding.error) {
                row.content?.dispose();
                row.content = null;
                row.reference = '';
                if (this.binding.error)
                    row.surface.unavailable('Source unavailable');
                else
                    row.surface.loading();
                continue;
            }
            const size = row.resolution.resolution;
            if (!size)
                continue;
            const reference = InputPreviewContent.reference(this.binding, entry, size);
            if (row.reference !== reference) {
                const previous = row.content;
                row.reference = reference;
                row.surface.loading();
                row.content = this.#content(entry, row, size);
                previous?.dispose();
            }
            else
                row.content?.refresh();
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
            row.content?.dispose();
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
            content: null,
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
    /** Keep interpretation and capture lifetimes outside the persistent preview's geometry owner. */
    #content(entry, row, size) {
        return new InputPreviewContent(this.binding, this.previews, entry, size, (value) => {
            if (!this.#alive)
                return;
            if (value.kind === 'loading')
                row.surface.loading();
            else if (value.kind === 'error')
                row.surface.unavailable(value.message);
            else {
                try {
                    row.surface.paint(value.pixels, entry.source.kind === 'canvas' || value.composition
                        ? (this.binding.context?.document ?? null)
                        : null, value.composition);
                }
                catch (error) {
                    row.surface.unavailable('Preview unavailable');
                    this.report(error);
                }
            }
        }, this.report);
    }
}
