import { inputLabel } from '../domain/InputLoader.js';
import { InputPreviewRows } from './InputPreviewRows.js';
import { CanvasSourcePicker } from './CanvasSourcePicker.js';
import { CommandButtonMount } from '../../surface/appearance/CommandButtonMount.js';
import { readPluginMode, subscribePluginMode } from '../../core/PluginMode.js';
import { CanvasSourceUnavailable } from '../domain/CanvasSourceAvailability.js';
import { ensureInputEditorStyles } from './InputEditorStyles.js';
/** Keep one source picker and stable preview surfaces while selection, channel and order change. */
export class MasonryInputEditor {
    binding;
    element;
    #rows;
    #status;
    #controls;
    #picker;
    #commands = [];
    #stop;
    #entryField;
    /** Bind presentation to portable state and share raw source acquisition with queue preparation. */
    constructor(document, binding, previews, upload, report, fields, fieldRoot, showField = false) {
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
            source: () => binding.state.entries.find((entry) => entry.id === binding.state.selected)?.source ?? {
                kind: 'file',
                file: null,
            },
            context: () => binding.context,
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
            ? fields.mountChoice(fieldRoot, showField ? binding.loader.field + ' Entry' : 'Entry', (id) => binding.select(id), 'Entry')
            : null;
        this.#controls.append(this.#picker.element);
        if (binding.loader.ordered) {
            const actions = document.createElement('div');
            actions.className = 'sc-input-editor__actions';
            for (const [label, action] of [
                ['Add ' + binding.loader.kind, () => binding.add()],
                ['Remove ' + binding.loader.kind, () => binding.remove()],
                ['Move ' + binding.loader.kind + ' up', () => binding.move(-1)],
                ['Move ' + binding.loader.kind + ' down', () => binding.move(1)],
            ]) {
                const button = new CommandButtonMount(document, label, () => {
                    try {
                        action();
                    }
                    catch (error) {
                        report(error);
                    }
                });
                this.#commands.push(button);
                actions.append(button.element);
            }
            this.#controls.append(actions);
        }
        this.#rows = new InputPreviewRows(document, binding, previews, report);
        this.#status = document.createElement('div');
        this.#status.className = 'sc-input-editor__status';
        this.#status.setAttribute('role', 'status');
        this.element.append(this.#rows.element, this.#controls, this.#status);
        for (const root of [this.element, this.#controls])
            for (const name of ['pointerdown', 'mousedown', 'contextmenu', 'click'])
                root.addEventListener(name, (event) => event.stopPropagation());
        const stopBinding = binding.subscribe(() => this.refresh());
        const stopMode = subscribePluginMode(document, () => this.refresh());
        this.#stop = () => {
            stopBinding();
            stopMode();
        };
        this.refresh();
    }
    /** Expose the owned commands for placement after the native node-field body. */
    get controlsElement() {
        return this.#controls;
    }
    /** Reconcile exact entry identities and reinterpret cached canvas pixels immediately on channel changes. */
    refresh() {
        const state = this.binding.state, passive = readPluginMode(this.element.ownerDocument);
        this.#controls.disabled = this.binding.busy;
        for (const command of this.#commands)
            command.setDisabled(this.binding.busy);
        this.#picker.refresh();
        this.#entryField?.update(state.selected ?? '', state.entries.map((entry) => ({ value: entry.id, label: entry.label, blockers: [] })), this.binding.busy, passive);
        this.#rows.refresh();
        this.#status.textContent = this.binding.busy
            ? 'Loading canvas sources…'
            : (this.binding.error ?? '');
    }
    /** Release subscriptions before removing preview surfaces or ignoring pending file reads. */
    dispose() {
        this.#stop();
        this.#picker.dispose();
        this.#entryField?.dispose();
        for (const command of this.#commands)
            command.dispose();
        this.#controls.remove();
        this.#rows.dispose();
        this.element.remove();
    }
}
