import { CanvasSourceUnavailable } from '../domain/CanvasSourceAvailability.js';
import { CANVAS_INPUT_CHOICES, chooseCanvasInput } from '../domain/CanvasInputSource.js';
import { CommandButtonMount } from '../../surface/appearance/CommandButtonMount.js';
/** Supply source values to native fields without owning their label, combo or popup rendering. */
export class CanvasSourcePicker {
    port;
    element;
    #kind;
    #layer;
    #fileRow;
    #file;
    #chooseFile;
    #filename;
    #busy = false;
    #alive = true;
    /** Share Comfy's field renderer and the existing command-button owner. */
    constructor(document, port, fields, fieldRoot, fieldLabel) {
        this.port = port;
        this.element = document.createElement('div');
        this.element.className = 'sc-canvas-source';
        this.#kind = fields.mountChoice(fieldRoot, fieldLabel ? fieldLabel + ' Source' : 'Source', this.#choose, 'Source', this.#prepare);
        this.#layer = fields.mountChoice(fieldRoot, fieldLabel ? fieldLabel + ' Layer' : 'Layer', this.#chooseLayer, 'Layer', this.#prepare);
        this.#file = document.createElement('input');
        this.#file.type = 'file';
        this.#file.accept = 'image/*';
        this.#file.hidden = true;
        this.#file.setAttribute('aria-label', 'Source file');
        this.#chooseFile = new CommandButtonMount(document, 'Browse…', () => {
            if (!this.#busy && !this.port.disabled)
                this.#file.click();
        });
        this.#filename = document.createElement('span');
        this.#filename.className = 'sc-canvas-source__filename';
        this.#fileRow = document.createElement('div');
        this.#fileRow.className = 'sc-canvas-source__file';
        this.#fileRow.setAttribute('role', 'group');
        this.#fileRow.setAttribute('aria-label', 'File');
        this.#fileRow.append(this.#file, this.#filename, this.#chooseFile.element);
        this.element.append(this.#fileRow);
        this.#file.addEventListener('change', this.#upload);
        this.refresh();
    }
    /** Reconcile capability restrictions and exact saved identities through the mounted native fields. */
    refresh() {
        const source = this.port.source(), context = this.port.context();
        const kind = source.kind === 'file' ? 'file' : source.target.kind;
        this.#kind.update(kind, CANVAS_INPUT_CHOICES.map((choice) => ({
            value: choice.kind,
            label: choice.label,
            blockers: choice.kind === 'file' || context?.targets.includes(choice.kind)
                ? []
                : ['the connected canvas'],
        })), this.#busy || this.port.disabled);
        const explicit = source.kind === 'canvas' &&
            (source.target.kind === 'layer' || source.target.kind === 'composition-from-layer');
        this.#fileRow.hidden = source.kind !== 'file';
        this.#file.disabled = this.#busy || this.port.disabled;
        this.#chooseFile.setDisabled(this.#busy || this.port.disabled);
        const file = source.kind === 'file' ? source.file : null;
        this.#filename.textContent = file?.split('/').at(-1) ?? 'No file selected';
        this.#filename.title = file ?? '';
        this.#filename.dataset.empty = String(!file);
        if (!explicit ||
            (source.target.kind !== 'layer' && source.target.kind !== 'composition-from-layer')) {
            this.#layer.update('', [], this.#busy || this.port.disabled, false);
            return;
        }
        const layers = context?.document?.id === source.documentId ? context.layers : [];
        const target = source.target;
        const value = layers.some((layer) => layer.id === target.layerId) ? target.layerId : '';
        this.#layer.update(value, [
            { value: '', label: 'Choose a layer', blockers: ['a document layer'] },
            ...layers.map((layer) => ({
                value: layer.id,
                label: ' '.repeat(layer.depth) + layer.name,
                blockers: [],
            })),
        ], this.#busy || this.port.disabled || !layers.length);
    }
    /** Retire native mounts before removing the owning source surface. */
    dispose() {
        this.#alive = false;
        this.#chooseFile.dispose();
        this.#kind.dispose();
        this.#layer.dispose();
        this.element.remove();
    }
    /** Discover current choices and preview pixels as part of using the source picker. */
    #prepare = async () => {
        if (this.#busy || this.port.disabled || !this.#alive)
            return false;
        try {
            await this.port.refresh();
            if (!this.#alive)
                return false;
            this.refresh();
            return true;
        }
        catch (error) {
            if (this.#alive)
                this.port.report(error);
            return false;
        }
    };
    /** Resolve the initial explicit layer only when the user chooses a layer-based target. */
    #choose = (value) => {
        const kind = CANVAS_INPUT_CHOICES.find((choice) => choice.kind === value)?.kind;
        if (!kind)
            return;
        const context = this.port.context();
        let target;
        if (kind === 'file') {
            const current = this.port.source();
            target = { kind, file: current.kind === 'file' ? current.file : null };
        }
        else if (kind === 'layer' || kind === 'composition-from-layer') {
            const layerId = context?.currentLayerId;
            if (!layerId) {
                this.port.report(new CanvasSourceUnavailable('Select a layer'));
                this.refresh();
                return;
            }
            target = { kind, layerId };
        }
        else
            target = { kind };
        this.#change(target);
    };
    /** Preserve the selected operation when the user chooses another exact document layer. */
    #chooseLayer = (layerId) => {
        const source = this.port.source();
        if (source.kind === 'canvas' &&
            (source.target.kind === 'layer' || source.target.kind === 'composition-from-layer'))
            this.#change({ kind: source.target.kind, layerId });
    };
    /** Persist validated user intent through the consumer's authoritative state transition. */
    #change(target) {
        void this.#operation(async () => {
            await this.port.change(chooseCanvasInput(this.port.canvasId, this.port.context(), target));
        });
    }
    /** Upload once and ignore the result if its owning surface has retired. */
    #upload = () => {
        const file = this.#file.files?.[0];
        if (!file)
            return;
        void this.#operation(async () => {
            const path = await this.port.upload(file);
            if (this.#alive)
                await this.port.change({ kind: 'file', file: path });
            this.#file.value = '';
        });
    };
    /** Keep native controls disabled until one source intent settles. */
    async #operation(action) {
        if (this.#busy || this.port.disabled || !this.#alive)
            return;
        this.#busy = true;
        this.refresh();
        try {
            await action();
        }
        catch (error) {
            if (this.#alive)
                this.port.report(error);
        }
        finally {
            this.#busy = false;
            if (this.#alive)
                this.refresh();
        }
    }
}
