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
/** Adapt Vue-rendered GIMP controls to the shared native-field and owned-surface contracts. */
import { defineCustomElement, nextTick } from 'vue';
import {
  GimpButtonComponent,
  GimpCheckboxComponent,
  GimpSearchComponent,
  GimpFrameComponent,
  GimpSliderComponent,
} from './GimpControlComponents.js';
import { GimpSpinScaleComponent } from './GimpSpinScale.js';
import { gimpControlStyles } from './GimpControlStyles.js';
import { NativeTabGroup } from './NativeTabGroup.js';
import { usesGimpSpinScale, type NumericSliderSpec } from './NumericSliderPresentation.js';
import type { NumericSliderControl } from './NativeNumericControls.js';
import type { BooleanSegmentsControls } from './NativeBooleanSegments.js';
import type { FinderInputControl } from './FinderControls.js';
import type { EditorFieldControl } from './EditorFieldControl.js';
import { relocateControl } from '../../core/ControlPlacement.js';

const ButtonElement = defineCustomElement(GimpButtonComponent, { styles: [gimpControlStyles] });
const CheckboxElement = defineCustomElement(GimpCheckboxComponent, { styles: [gimpControlStyles] });
const ScaleElement = defineCustomElement(GimpSpinScaleComponent, { styles: [gimpControlStyles] });
const SliderElement = defineCustomElement(GimpSliderComponent, { styles: [gimpControlStyles] });
const SearchElement = defineCustomElement(GimpSearchComponent, { styles: [gimpControlStyles] });
const FrameElement = defineCustomElement(GimpFrameComponent, { styles: [gimpControlStyles] });

/** Forward imperative activation to the browser's real disabled-aware button. */
class GimpButton extends ButtonElement {
  /** Keep native activation rather than synthesizing a host click on a disabled element. */
  override click(): void {
    this.shadowRoot?.querySelector('button')?.click();
  }
  /** Retain keyboard focus on the single interactive shadow control. */
  override focus(options?: FocusOptions): void {
    this.shadowRoot?.querySelector('button')?.focus(options);
  }
  /** Preserve the mounted Vue tree during state-preserving host moves. */
  connectedMoveCallback(): void {}
}

/** Keep checkbox gestures accessible through the same imperative contract as native inputs. */
class GimpCheckbox extends CheckboxElement {
  /** Use the real checkbox default action, including exactly one change event. */
  override click(): void {
    this.shadowRoot?.querySelector('input')?.click();
  }
  /** Preserve one native focus target. */
  override focus(options?: FocusOptions): void {
    this.shadowRoot?.querySelector('input')?.focus(options);
  }
  /** Preserve current state and focus during native presentation relocation. */
  connectedMoveCallback(): void {}
}

/** Expose the native search field without giving finder views a second query owner. */
class GimpSearch extends SearchElement implements FinderInputControl {
  /** Resolve only after connection, when Vue has mounted the actual input. */
  get control(): HTMLInputElement {
    const input = this.shadowRoot?.querySelector('input');
    if (!input) throw new Error('The GIMP search field is not mounted.');
    return input;
  }
  /** Follow Vue attribute reflection before selecting existing text. */
  select(): void {
    void nextTick(() => {
      if (this.isConnected) this.control.select();
    });
  }
  /** Share the finder owner's explicit focus/selection policy. */
  focusInput(selectAll: boolean): void {
    void nextTick(() => {
      if (!this.isConnected) return;
      this.control.focus();
      if (selectAll) this.control.select();
    });
  }
  /** Forward browser focus after synchronous connection rendering. */
  override focus(options?: FocusOptions): void {
    this.shadowRoot?.querySelector('input')?.focus(options);
  }
  /** Keep a focused query intact during state-preserving relocation. */
  connectedMoveCallback(): void {}
}

/** Share the established tab keyboard owner without embedding workflow state in Vue. */
class GimpTabs extends NativeTabGroup {}
/** Render slotted content while the shared editor and workflow owners retain child identity. */
class GimpFrame extends FrameElement {
  /** Keep CodeMirror mounted while its containing surface moves. */
  connectedMoveCallback(): void {}
}

for (const [name, constructor] of [
  ['sc-gimp-button', GimpButton],
  ['sc-gimp-checkbox', GimpCheckbox],
  ['sc-gimp-scale', ScaleElement],
  ['sc-gimp-slider', SliderElement],
  ['sc-gimp-search', GimpSearch],
  ['sc-gimp-frame', GimpFrame],
  ['sc-gimp-tabs', GimpTabs],
] as const)
  if (!customElements.get(name)) customElements.define(name, constructor);

/** Create the registered Vue command without exposing Vue internals to consumers. */
export function createGimpButton(document: Document): GimpButton {
  const button = document.createElement('sc-gimp-button');
  if (!(button instanceof GimpButton)) throw new TypeError('GIMP buttons are not registered.');
  return button;
}

/** Select the GIMP numeric variant while the shared binding retains the host's canonical value. */
export function createGimpNumericSlider(
  document: Document,
  field: NumericSliderSpec,
): NumericSliderControl {
  return usesGimpSpinScale(field) ? createGimpSpinScale(document) : createGimpSlider(document);
}

/** Keep GIMP's filled spin scale distinct from a conventional slider-and-spinner pair. */
export function createGimpSpinScale(document: Document): NumericSliderControl {
  const root = document.createElement('sc-gimp-scale');
  if (!(root instanceof ScaleElement)) throw new TypeError('GIMP scales are not registered.');
  return adaptGimpNumericControl(root);
}

/** Expose the visible native range without taking ownership of the adjacent spinner. */
export function createGimpSlider(document: Document): NumericSliderControl {
  const root = document.createElement('sc-gimp-slider');
  if (!(root instanceof SliderElement)) throw new TypeError('GIMP sliders are not registered.');
  return adaptGimpNumericControl(root);
}

/** Project both numeric primitives through the shared host-value contract. */
function adaptGimpNumericControl(
  root: InstanceType<typeof ScaleElement> | InstanceType<typeof SliderElement>,
): NumericSliderControl {
  return {
    root,
    readValue: () => Number(root.value),
    synchronize(minimum, maximum, step, disabled, value): void {
      root.min = minimum;
      root.max = maximum;
      root.step = step;
      root.disabled = disabled;
      if (value !== null) root.value = value;
    },
  };
}

/** Return a boolean control whose canonical checked value is supplied by the host binding. */
export function createGimpCheckbox(document: Document): GimpCheckbox {
  const checkbox = document.createElement('sc-gimp-checkbox');
  if (!(checkbox instanceof GimpCheckbox))
    throw new TypeError('GIMP checkboxes are not registered.');
  return checkbox;
}

/** Keep tab selection and keyboard handling under the shared native owner. */
export function createGimpTablist(document: Document): GimpTabs {
  const group = document.createElement('sc-gimp-tabs');
  if (!(group instanceof GimpTabs)) throw new TypeError('GIMP tabs are not registered.');
  return group;
}

let nextSegment = 0;
/** Reuse native tab gestures for compact GTK-style boolean buttons. */
export function createGimpSegments(document: Document): BooleanSegmentsControls {
  const group = createGimpTablist(document);
  const off = createGimpButton(document),
    on = createGimpButton(document);
  for (const tab of [off, on]) {
    tab.id = `sc-gimp-boolean-${++nextSegment}`;
    tab.setAttribute('role', 'tab');
    tab.tabIndex = -1;
  }
  return { group, segments: [off, on], release: () => group.remove() };
}

/** Keep declarative and imperative search on one registered Vue entry. */
export function createGimpFinderInput(document: Document): FinderInputControl {
  const input = document.createElement('sc-gimp-search');
  if (!(input instanceof GimpSearch)) throw new TypeError('GIMP search fields are not registered.');
  return input;
}

/** Provide entry chrome while CodeMirror owns document, selection, history and editing. */
export function createGimpEditorField(document: Document): EditorFieldControl {
  const root = document.createElement('sc-gimp-frame');
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

/** Keep workflow title and close controls as slotted children of a presentation-only Vue frame. */
export function createGimpWorkflowTab(document: Document): HTMLElement {
  const tab = document.createElement('sc-gimp-frame');
  tab.setAttribute('role', 'tab');
  tab.tabIndex = -1;
  return tab;
}
