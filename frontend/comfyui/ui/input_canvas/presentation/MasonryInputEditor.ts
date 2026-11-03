//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Present actual image sources and effective masks with reusable source choices and ordered collection controls. */
import type {
  InputEditorBinding,
  InputImageAssociation,
  InputSurfaceCardPort,
} from '../application/InputEditorPorts.js';
import { MaskOpacityField, maskOpacityLabel } from './MaskOpacityField.js';
import { MaskImportCommand } from './MaskImportCommand.js';
import type { CanvasSourcePreviews } from '../application/CanvasSourcePreviews.js';
import { inputLabel } from '../domain/InputLoader.js';
import { InputPreviewRows } from './InputPreviewRows.js';
import { CanvasSourcePicker } from './CanvasSourcePicker.js';
import type { NativeNodeFields } from '../../surface/NativeNodeFields.js';
import { CommandButtonMount } from '../../surface/appearance/CommandButtonMount.js';
import { readPluginMode, subscribePluginMode } from '../../core/PluginMode.js';
import { CanvasSourceUnavailable } from '../domain/CanvasSourceAvailability.js';
import type { NativeNodeChoiceMount } from '../../surface/NativeNodeFields.js';
import { ensureInputEditorStyles } from './InputEditorStyles.js';
import { MaskEditFields } from './MaskEditFields.js';

/** Keep one source picker and stable preview surfaces while selection, channel and order change. */
export class MasonryInputEditor {
  readonly element: HTMLElement;
  readonly #rows: InputPreviewRows;
  readonly #status: HTMLElement;
  readonly #controls: HTMLFieldSetElement;
  readonly #picker: CanvasSourcePicker;
  readonly #commands: CommandButtonMount[] = [];
  readonly #stop: () => void;
  readonly #entryField: NativeNodeChoiceMount | null;
  readonly #maskFields: MaskEditFields;
  readonly #opacity: MaskOpacityField | null;
  #import: MaskImportCommand | null = null;

  /** Bind presentation to portable state and share raw source acquisition with queue preparation. */
  constructor(
    document: Document,
    private readonly binding: InputEditorBinding,
    previews: CanvasSourcePreviews,
    upload: (file: File) => Promise<string>,
    report: (error: unknown) => void,
    fields: NativeNodeFields,
    fieldRoot: HTMLElement,
    association: InputImageAssociation | null,
    showField = false,
    surface: InputSurfaceCardPort | null = null,
  ) {
    ensureInputEditorStyles(document);
    this.element = document.createElement('section');
    this.element.className = 'sc-input-editor';
    this.element.setAttribute('data-sc-input-editor', '');
    this.element.setAttribute('data-sc-host-opaque', '');
    this.element.setAttribute(
      'aria-label',
      inputLabel(binding.loader.kind) + ' sources: ' + binding.loader.field,
    );
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
    this.#picker = new CanvasSourcePicker(
      document,
      {
        get canvasId() {
          return binding.canvasId;
        },
        get disabled() {
          return binding.busy;
        },
        source: () =>
          binding.state.entries.find((entry) => entry.id === binding.state.selected)?.source ?? {
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
          } else report(error);
        },
      },
      fields,
      fieldRoot,
      showField ? binding.loader.field : null,
    );
    this.#entryField = binding.loader.ordered
      ? fields.mountChoice(
          fieldRoot,
          showField ? binding.loader.field + ' Entry' : 'Entry',
          (id) => {
            binding.select(id);
            surface?.activate(id);
          },
          'Entry',
        )
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
      ] as const) {
        const button = new CommandButtonMount(document, label, () => {
          try {
            action();
          } catch (error: unknown) {
            report(error);
          }
        });
        this.#commands.push(button);
        actions.append(button.element);
      }
      if (surface && binding.loader.kind === 'mask') {
        this.#import = new MaskImportCommand(document, surface, report);
        actions.append(this.#import.element);
      }
      this.#controls.append(actions);
    }
    this.#rows = new InputPreviewRows(document, binding, previews, report, association, surface);
    this.#opacity =
      surface && binding.loader.kind === 'mask'
        ? new MaskOpacityField(
            surface,
            fields,
            maskOpacityLabel(showField ? binding.loader.field : null),
            report,
          )
        : null;
    this.#maskFields = new MaskEditFields(
      binding,
      association,
      fields,
      fieldRoot,
      showField,
      (mode) => this.#rows.setMaskMode(mode),
      report,
    );
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
  get controlsElement(): HTMLElement {
    return this.#controls;
  }

  /** Reconcile exact entry identities and reinterpret cached canvas pixels immediately on channel changes. */
  refresh(): void {
    const state = this.binding.state,
      passive = readPluginMode(this.element.ownerDocument);
    this.#controls.disabled = this.binding.busy;
    for (const command of this.#commands) command.setDisabled(this.binding.busy);
    this.#picker.refresh();
    this.#maskFields.refresh();
    this.#entryField?.update(
      state.selected ?? '',
      state.entries.map((entry) => ({ value: entry.id, label: entry.label, blockers: [] })),
      this.binding.busy,
      passive,
    );
    this.#rows.refresh();
    this.#status.textContent = this.binding.busy
      ? 'Loading canvas sources…'
      : (this.binding.error ?? '');
  }

  /** Release subscriptions before removing preview surfaces or ignoring pending file reads. */
  dispose(): void {
    this.#stop();
    this.#picker.dispose();
    this.#maskFields.dispose();
    this.#opacity?.dispose();
    this.#import?.dispose();
    this.#entryField?.dispose();
    for (const command of this.#commands) command.dispose();
    this.#controls.remove();
    this.#rows.dispose();
    this.element.remove();
  }
}
