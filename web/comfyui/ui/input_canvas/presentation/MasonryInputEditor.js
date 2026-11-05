import { MaskOpacityField, maskOpacityLabel } from './MaskOpacityField.js';
import { MaskImportCommand } from './MaskImportCommand.js';
import { MaskFileCommand } from './MaskFileCommand.js';
import { inputLabel } from '../domain/InputLoader.js';
import { InputPreviewRows } from './InputPreviewRows.js';
import { CanvasSourcePicker } from './CanvasSourcePicker.js';
import { CommandButtonMount } from '../../surface/appearance/CommandButtonMount.js';
import { readPluginMode, subscribePluginMode } from '../../core/PluginMode.js';
import { CanvasSourceUnavailable } from '../domain/CanvasSourceAvailability.js';
import { ensureInputEditorStyles } from './InputEditorStyles.js';
import { MaskEditFields } from './MaskEditFields.js';
/** Keep one source picker and stable preview surfaces while selection, channel and order change. */
export class MasonryInputEditor {
    binding;
    element;
    #rows;
    #status;
    #controls;
    #picker;
    #commands = [];
    /** Reordering, which the input canvas's regional masks do not offer. */
    #moves;
    #browse;
    #stop;
    #entryField;
    #maskFields;
    #opacity;
    #surface;
    #import = null;
    /** Bind presentation to portable state and share raw source acquisition with queue preparation. */
    constructor(document, binding, previews, upload, report, fields, fieldRoot, association, showField = false, surface = null) {
        this.binding = binding;
        ensureInputEditorStyles(document);
        this.element = document.createElement('section');
        this.element.className = 'sc-input-editor';
        this.element.setAttribute('data-sc-input-editor', '');
        this.element.setAttribute('data-sc-host-opaque', '');
        this.element.setAttribute('aria-label', inputLabel(binding.loader.kind) + ' sources: ' + binding.loader.field);
        this.element.dataset.inputField = binding.loader.field;
        this.element.dataset.inputKind = binding.loader.kind;
        if (showField) {
            const title = document.createElement('h4');
            title.className = 'sc-input-editor__field';
            title.textContent = binding.loader.field;
            this.element.append(title);
        }
        this.#controls = document.createElement('fieldset');
        this.#controls.className = 'sc-input-editor__controls';
        this.#controls.setAttribute('data-sc-input-controls', '');
        this.#controls.setAttribute('data-sc-host-opaque', '');
        this.#picker = new CanvasSourcePicker(document, {
            get canvasId() {
                return binding.canvasId;
            },
            get disabled() {
                return binding.busy;
            },
            get hidden() {
                return MasonryInputEditor.#presentsInputCanvasMask(binding);
            },
            source: () => binding.state.entries.find((entry) => entry.id === binding.state.selected)?.source ?? {
                kind: 'file',
                file: null,
            },
            context: () => binding.context,
            refresh: () => binding.refreshSources(),
            change: (source) => binding.changeSource(source),
            upload,
            report: (error) => {
                if (error instanceof CanvasSourceUnavailable) {
                    this.#rows.unavailable(error.message);
                }
                else
                    report(error);
            },
        }, fields, fieldRoot, showField ? binding.loader.field : null);
        this.#entryField = binding.loader.ordered
            ? fields.mountChoice(fieldRoot, showField ? binding.loader.field + ' Entry' : 'Entry', (id) => {
                binding.select(id);
                surface?.activate(id);
            }, 'Entry')
            : null;
        this.#controls.append(this.#picker.element);
        const command = (label, action) => {
            const button = new CommandButtonMount(document, label, () => {
                try {
                    action();
                }
                catch (error) {
                    report(error);
                }
            });
            this.#commands.push(button);
            return button.element;
        };
        this.#moves = null;
        this.#browse = null;
        if (binding.loader.ordered) {
            const actions = document.createElement('div');
            actions.className = 'sc-input-editor__actions';
            actions.append(command('Add ' + binding.loader.kind, () => binding.add()));
            // The input canvas's regional cards order their commands Add, Choose mask, Remove.
            if (surface && binding.loader.kind === 'mask') {
                this.#import = new MaskImportCommand(document, surface, report);
                actions.append(this.#import.element);
            }
            actions.append(command('Remove ' + binding.loader.kind, () => binding.remove()));
            this.#moves = document.createElement('span');
            this.#moves.className = 'sc-input-editor__moves';
            this.#moves.append(command('Move ' + binding.loader.kind + ' up', () => binding.move(-1)), command('Move ' + binding.loader.kind + ' down', () => binding.move(1)));
            actions.append(this.#moves);
            this.#controls.append(actions);
        }
        else if (binding.loader.kind === 'mask') {
            this.#browse = new MaskFileCommand(document, upload, (file) => binding.loadFile(file), report);
            this.#controls.append(this.#browse.element);
        }
        this.#rows = new InputPreviewRows(document, binding, previews, report, association, surface);
        this.#opacity = surface
            ? new MaskOpacityField(surface, fields, maskOpacityLabel(showField ? binding.loader.field : null), report)
            : null;
        this.#maskFields = new MaskEditFields(binding, association, fields, fieldRoot, showField, (mode) => this.#rows.setMaskMode(mode), report);
        this.#status = document.createElement('div');
        this.#status.className = 'sc-input-editor__status';
        this.#status.setAttribute('role', 'status');
        this.element.append(this.#rows.element, this.#controls, this.#status);
        for (const root of [this.element, this.#controls])
            for (const name of ['pointerdown', 'mousedown', 'contextmenu', 'click'])
                root.addEventListener(name, (event) => event.stopPropagation());
        const stopBinding = binding.subscribe(() => this.refresh());
        const stopMode = subscribePluginMode(document, () => this.refresh());
        const stopSurface = surface?.subscribe(() => this.refresh());
        this.#surface = surface;
        this.#stop = () => {
            stopBinding();
            stopMode();
            stopSurface?.();
        };
        this.refresh();
    }
    /** Expose the owned commands for placement after the native node-field body. */
    get controlsElement() {
        return this.#controls;
    }
    /** Reconcile exact entry identities and reinterpret cached canvas pixels immediately on channel changes. */
    refresh() {
        const state = this.binding.state, passive = readPluginMode(this.element.ownerDocument), inputCanvas = MasonryInputEditor.#presentsInputCanvasMask(this.binding);
        this.#controls.disabled = this.binding.busy;
        for (const command of this.#commands)
            command.setDisabled(this.binding.busy);
        if (this.#moves)
            this.#moves.hidden = inputCanvas;
        this.#browse?.update(inputCanvas, this.binding.busy);
        this.#picker.refresh();
        this.#maskFields.refresh();
        this.#entryField?.update(state.selected ?? '', state.entries.map((entry) => ({
            value: entry.id,
            label: this.#surface?.region(entry.id)?.label ?? entry.label,
            blockers: [],
        })), this.binding.busy, passive);
        this.#rows.refresh();
        this.#status.textContent = this.binding.busy
            ? 'Loading canvas sources…'
            : (this.binding.error ?? '');
    }
    /** Release subscriptions before removing preview surfaces or ignoring pending file reads. */
    dispose() {
        this.#stop();
        this.#picker.dispose();
        this.#maskFields.dispose();
        this.#opacity?.dispose();
        this.#import?.dispose();
        this.#browse?.dispose();
        this.#entryField?.dispose();
        for (const command of this.#commands)
            command.dispose();
        this.#controls.remove();
        this.#rows.dispose();
        this.element.remove();
    }
    /** Whether the card is a mask card of a canvas that provisions its input surfaces. */
    static #presentsInputCanvasMask(binding) {
        return binding.loader.kind === 'mask' && binding.provisioned;
    }
}
