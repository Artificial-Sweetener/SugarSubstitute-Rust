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
/** Select official controls at the browser boundary without duplicating native value policies. */
import {
  createFluentNumericSlider,
  createBooleanCheckboxControl,
  createBooleanSegmentControls,
  createFinderInputControl,
  createWorkflowTab as createFluentWorkflowTab,
  createWorkflowTablist as createFluentWorkflowTablist,
} from './FluentThemeRuntime.js';
import {
  createSpectrumButton,
  createSpectrumSlider,
  createSpectrumCheckbox,
  createSpectrumSegments,
  createSpectrumFinderInput,
  createSpectrumTablist,
  createSpectrumActivationSwitch,
} from './SpectrumThemeRuntime.js';
import { readOwnedVisualStyle } from './VisualStyleBoundary.js';
import type { NumericSliderControl } from './NativeNumericControls.js';
import type { BooleanCheckboxControl } from './NativeToggleRows.js';
import type { BooleanSegmentsControls } from './NativeBooleanSegments.js';
import type { FinderInputControl } from './FinderControls.js';
import type { VisualStyle } from './VisualStyle.js';
import {
  createNativeActivationSwitch,
  type ActivationSwitchControl,
} from './ActivationSwitchControl.js';

/** Select activation presentation without taking ownership of node state or history. */
export function createActivationSwitch(document: Document): ActivationSwitchControl {
  const result =
    readOwnedVisualStyle(document) === 'spectrum'
      ? createSpectrumActivationSwitch(document)
      : createNativeActivationSwitch(document);
  result.control.dataset.scActivationSwitch = '';
  return result;
}

/** Create an owned command button with the selected system's real interaction implementation. */
export function createButton(document: Document): HTMLElement {
  const button =
    readOwnedVisualStyle(document) === 'spectrum'
      ? createSpectrumButton(document)
      : document.createElement('fluent-button');
  button.dataset.scButton = '';
  return button;
}

/** Normalize design-system numeric APIs before binding to a native Comfy field. */
export function createNumericSlider(document: Document): NumericSliderControl {
  return readOwnedVisualStyle(document) === 'spectrum'
    ? createSpectrumSlider(document)
    : createFluentNumericSlider(document);
}

/** Keep checked-state semantics independent of the selected component library. */
export function createBooleanCheckbox(document: Document): BooleanCheckboxControl {
  return readOwnedVisualStyle(document) === 'spectrum'
    ? createSpectrumCheckbox(document)
    : createBooleanCheckboxControl(document);
}

/** Keep native boolean edits and undo history under the existing shared binding owner. */
export function createBooleanSegments(document: Document): BooleanSegmentsControls {
  return readOwnedVisualStyle(document) === 'spectrum'
    ? createSpectrumSegments(document)
    : createBooleanSegmentControls(document);
}

/** Create a real accessible search field in the selected visual system. */
export function createFinderInput(document: Document): FinderInputControl {
  return readOwnedVisualStyle(document) === 'spectrum'
    ? createSpectrumFinderInput(document)
    : createFinderInputControl(document);
}

/** Create a workflow tab group whose adapter exposes stable child identity. */
export function createWorkflowTablist(document: Document): HTMLElement & { activeid: string } {
  const group =
    readOwnedVisualStyle(document) === 'spectrum'
      ? createSpectrumTablist(document)
      : createFluentWorkflowTablist();
  group.dataset.scWorkflowTabs = '';
  return group;
}

/** Create the selected provider's workflow tab while retaining shared drag and close behavior. */
export function createWorkflowTab(document: Document): HTMLElement {
  const tab =
    readOwnedVisualStyle(document) === 'spectrum'
      ? document.createElement('sp-tab')
      : createFluentWorkflowTab(document);
  tab.dataset.scWorkflowTab = '';
  return tab;
}

/** Choose the provider's accessory slots without leaking library-specific DOM into shared views. */
export function workflowTabAccessorySlot(document: Document, position: 'start' | 'end'): string {
  return readOwnedVisualStyle(document) === 'spectrum' ? '' : position;
}

/** Name the already registered provider element for declarative Vue rendering. */
export function buttonTag(style: VisualStyle): string {
  return style === 'comfy' ? 'button' : style === 'spectrum' ? 'sp-action-button' : 'fluent-button';
}

/** Select a registered text-field adapter for declarative search views. */
export function searchInputTag(style: VisualStyle): string {
  return style === 'comfy'
    ? 'input'
    : style === 'spectrum'
      ? 'sc-spectrum-search'
      : 'fluent-text-input';
}
