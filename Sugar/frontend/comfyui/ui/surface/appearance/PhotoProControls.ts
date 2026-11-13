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
/** Adapt browser controls to OpenPhoto paint while keeping native values in their existing owners. */
import { NativeTabGroup } from './NativeTabGroup.js';
import type { NumericSliderControl } from './NativeNumericControls.js';
import type { BooleanSegmentsControls } from './NativeBooleanSegments.js';
import type { FinderInputControl } from './FinderControls.js';
import type { EditorFieldControl } from './EditorFieldControl.js';
import { relocateControl } from '../../core/ControlPlacement.js';

/** Retain ordinary browser input events and a single focus target for the search field. */
class PhotoProFinderInput extends HTMLElement implements FinderInputControl {
  readonly control: HTMLInputElement;

  /** Keep attributes written by declarative views on the actual input. */
  static get observedAttributes(): string[] {
    return ['placeholder', 'aria-label', 'disabled'];
  }

  /** Mount the input in light DOM so the shared view can inspect its real value. */
  constructor() {
    super();
    this.control = this.ownerDocument.createElement('input');
    this.control.type = 'search';
    this.control.setAttribute('aria-label', 'Search masonry');
  }

  /** Mount once after construction, as required by the custom-element lifecycle. */
  connectedCallback(): void {
    if (this.control.parentElement !== this) this.append(this.control);
  }

  /** Mirror only supported field attributes across the custom-element boundary. */
  attributeChangedCallback(name: string, _previous: string | null, value: string | null): void {
    if (value === null) this.control.removeAttribute(name);
    else this.control.setAttribute(name, value);
  }

  /** Reflect declarative values into the single native input. */
  get value(): string {
    return this.control.value;
  }

  /** Accept declarative updates without constructing a second input. */
  set value(value: string) {
    this.control.value = value;
  }

  /** Forward focus to the native input. */
  override focus(options?: FocusOptions): void {
    this.control.focus(options);
  }

  /** Select the actual search text. */
  select(): void {
    this.control.select();
  }

  /** Share the finder owner's explicit focus policy. */
  focusInput(selectAll: boolean): void {
    this.control.setAttribute('aria-label', this.getAttribute('aria-label') ?? 'Search masonry');
    this.control.focus();
    if (selectAll) this.control.select();
  }
}

/** Retain the public provider element while sharing native keyboard semantics. */
class PhotoProTabGroup extends NativeTabGroup {}

if (!customElements.get('sc-photopro-search'))
  customElements.define('sc-photopro-search', PhotoProFinderInput);
if (!customElements.get('sc-photopro-tabs'))
  customElements.define('sc-photopro-tabs', PhotoProTabGroup);

/** Keep PhotoPro commands as accessible native buttons. */
export function createPhotoProButton(document: Document): HTMLButtonElement {
  const button = document.createElement('button');
  button.type = 'button';
  return button;
}

/** Adapt the native range value to the existing numeric binding contract. */
export function createPhotoProSlider(document: Document): NumericSliderControl {
  const input = document.createElement('input');
  input.type = 'range';
  input.setAttribute('aria-label', 'Value');
  return {
    root: input,
    readValue: () => input.valueAsNumber,
    synchronize: (minimum, maximum, step, disabled, value) => {
      input.min = String(minimum);
      input.max = String(maximum);
      input.step = String(step);
      input.disabled = disabled;
      if (value !== null) input.valueAsNumber = value;
    },
  };
}

/** Use the browser's checked and disabled state for OpenPhoto's square checkbox. */
export function createPhotoProCheckbox(document: Document): HTMLInputElement {
  const input = document.createElement('input');
  input.type = 'checkbox';
  return input;
}

/** Share one tab group implementation with native booleans and workflow tabs. */
export function createPhotoProTablist(document: Document): PhotoProTabGroup {
  const group = document.createElement('sc-photopro-tabs');
  if (!(group instanceof PhotoProTabGroup))
    throw new TypeError('PhotoPro tabs are not registered.');
  return group;
}

let nextSegment = 0;

/** Present native off/on choices as compact OpenPhoto segments. */
export function createPhotoProSegments(document: Document): BooleanSegmentsControls {
  const group = createPhotoProTablist(document);
  const off = createPhotoProButton(document);
  const on = createPhotoProButton(document);
  for (const tab of [off, on]) {
    tab.id = `sc-photopro-boolean-${++nextSegment}`;
    tab.setAttribute('role', 'tab');
    tab.tabIndex = -1;
  }
  return { group, segments: [off, on], release: () => group.remove() };
}

/** Provide a stable native search field for masonry. */
export function createPhotoProFinderInput(document: Document): FinderInputControl {
  const input = document.createElement('sc-photopro-search');
  if (!(input instanceof PhotoProFinderInput))
    throw new TypeError('PhotoPro search is not registered.');
  return input;
}

/** Keep CodeMirror's document and focus while drawing a compact field frame. */
export function createPhotoProEditorField(document: Document): EditorFieldControl {
  const root = document.createElement('div');
  root.className = 'sc-photopro-editor-field';
  return {
    root,
    attachEditor(editor): void {
      editor.removeAttribute('slot');
      relocateControl(editor, root);
    },
    setReadOnly(readOnly): void {
      root.setAttribute('aria-readonly', String(readOnly));
    },
  };
}

/** Expose a light-DOM workflow tab so its title and close action retain identity. */
export function createPhotoProWorkflowTab(document: Document): HTMLElement {
  const tab = document.createElement('div');
  tab.setAttribute('role', 'tab');
  tab.tabIndex = -1;
  return tab;
}
