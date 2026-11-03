import { InputPreviewContent } from '../application/InputPreviewContent.js';
import { InputPreviewSurface } from './InputPreviewSurface.js';
import { InputPreviewResolutionObserver } from './InputPreviewResolutionObserver.js';
import { readPluginMode } from '../../core/PluginMode.js';
/** Share one mounted preview lifecycle across value, target, settings and geometry refreshes. */
export class InputPreviewRows {
    binding;
    previews;
    report;
    association;
    surface;
    element;
    #empty;
    #entries = new Map();
    #alive = true;
    #refreshingNativeSource = false;
    #mode = 'context';
    /** Bind pixels and state through the existing source and interpretation owners. */
    constructor(document, binding, previews, report, association, surface = null) {
        this.binding = binding;
        this.previews = previews;
        this.report = report;
        this.association = association;
        this.surface = surface;
        this.element = document.createElement('div');
        this.element.className = 'sc-input-editor__rows';
        this.#empty = new InputPreviewSurface(document, true, () => { });
        this.#empty.element.dataset.inputEmpty = '';
        this.#empty.update('', true, false, true, () => { });
        if (binding.loader.kind === 'mask')
            this.#empty.black();
        else
            this.#empty.unavailable('Choose a source');
        this.element.append(this.#empty.element);
        document.defaultView?.addEventListener('focus', this.#nativeReturned);
        document.documentElement.addEventListener('pointerenter', this.#nativeReturned);
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
                this.binding.loader.kind === 'image' &&
                (entry.source.kind === 'canvas' || !entry.source.file))
                row.surface.frame(document.width, document.height);
            if (this.binding.busy || this.binding.error) {
                row.content?.dispose();
                row.content = null;
                row.reference = '';
                if (this.binding.loader.kind === 'mask')
                    row.surface.black();
                else if (this.binding.error)
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
    /** Switch framing on the existing preview surfaces without changing mask settings or source identity. */
    setMaskMode(mode) {
        if (this.#mode === mode)
            return;
        this.#mode = mode;
        this.refresh();
    }
    /** Explain a rejected source intent within its existing selected preview. */
    unavailable(message) {
        const row = this.#entries.get(this.binding.state.selected ?? '');
        if (row?.content)
            row.content.unavailable(message);
        else if (this.binding.loader.kind === 'mask')
            (row?.surface ?? this.#empty).black();
        else
            (row?.surface ?? this.#empty).unavailable(message);
    }
    /** Release geometry and capture observations before accepting any late interpretation result. */
    dispose() {
        this.#alive = false;
        this.element.ownerDocument.defaultView?.removeEventListener('focus', this.#nativeReturned);
        this.element.ownerDocument.documentElement.removeEventListener('pointerenter', this.#nativeReturned);
        for (const row of this.#entries.values()) {
            row.content?.dispose();
            row.resolution.dispose();
        }
        this.#entries.clear();
        this.element.remove();
    }
    /** Refresh mounted capture-only sources on return from native editing without claiming live observation. */
    #nativeReturned = (event) => {
        if (event.type === 'pointerenter' &&
            'relatedTarget' in event &&
            event.relatedTarget instanceof Node &&
            event.relatedTarget.ownerDocument === this.element.ownerDocument)
            return;
        if (!this.#alive ||
            this.#refreshingNativeSource ||
            this.binding.context?.observesChanges !== false)
            return;
        this.#refreshingNativeSource = true;
        void this.binding
            .refreshSources()
            .catch(this.report)
            .finally(() => {
            this.#refreshingNativeSource = false;
        });
    };
    /** Create one literal, stable entry selection surface without native mask mutation intents. */
    #row(entry) {
        const surface = new InputPreviewSurface(this.element.ownerDocument, readPluginMode(this.element.ownerDocument), () => this.#select(entry.id));
        surface.element.dataset.inputEntry = entry.id;
        if (this.binding.loader.kind === 'mask' && this.binding.loader.ordered) {
            surface.element.addEventListener('pointerenter', () => this.surface?.hover(entry.id));
            surface.element.addEventListener('pointerleave', () => this.surface?.hover(null));
        }
        const document = this.binding.context?.document;
        if (document)
            surface.frame(document.width, document.height);
        if (this.binding.loader.kind === 'mask')
            surface.black();
        return {
            surface,
            resolution: new InputPreviewResolutionObserver(surface.frameElement, () => this.refresh()),
            reference: '',
            content: null,
        };
    }
    /** Select the entry, then show its input surface when a workflow-surface canvas holds it. */
    #select(id) {
        try {
            this.binding.select(id);
            this.surface?.activate(this.binding.loader.ordered ? id : null);
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
            else if (value.kind === 'black')
                row.surface.black();
            else if (value.kind === 'error')
                row.surface.unavailable(value.message);
            else {
                try {
                    row.surface.paint(value.pixels, value.frame, value.background, value.placement ?? null, value.outline ?? false, value.viewport ?? null);
                }
                catch (error) {
                    row.surface.unavailable('Preview unavailable');
                    this.report(error);
                }
            }
        }, this.report, this.association, () => this.#mode);
    }
}
