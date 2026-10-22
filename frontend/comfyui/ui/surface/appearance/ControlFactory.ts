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
  createFluentEditorField,
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
  createSpectrumEditorField,
} from './SpectrumThemeRuntime.js';
import { readOwnedVisualStyle } from './VisualStyleBoundary.js';
import type { NumericSliderControl } from './NativeNumericControls.js';
import type { BooleanCheckboxControl } from './NativeToggleRows.js';
import type { BooleanSegmentsControls } from './NativeBooleanSegments.js';
import type { FinderInputControl } from './FinderControls.js';
import type { VisualStyle } from './VisualStyle.js';
import { adoptEditorFieldPadding, type EditorFieldControl } from './EditorFieldControl.js';
import { relocateControl } from '../../core/ControlPlacement.js';
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

/** Select the existing provider's input asset for an editor with its own canonical document. */
export function createEditorField(document: Document, style: VisualStyle): EditorFieldControl {
  const field =
    style === 'fluent'
      ? createFluentEditorField(document)
      : style === 'spectrum'
        ? createSpectrumEditorField(document)
        : createComfyEditorField(document);
  field.root.dataset.scEditorField = style;
  field.root.style.cssText +=
    ';display:block;box-sizing:border-box;width:100%;height:100%;min-width:0;min-height:0';
  return field;
}

/** Reuse Comfy's native multiline input asset and surrounding widget interaction frame. */
function createComfyEditorField(document: Document): EditorFieldControl {
  const root = document.createElement('div');
  root.className = 'comfy-multiline-input';
  return {
    root,
    /** Preserve the same editable node when leaving a shadow-slot provider. */
    attachEditor(editor): void {
      adoptEditorFieldPadding(root, root);
      editor.removeAttribute('slot');
      relocateControl(editor, root);
    },
    /** Native CodeMirror owns editing permission; expose it to host control presentation. */
    setReadOnly(readOnly): void {
      root.setAttribute('aria-readonly', String(readOnly));
    },
  };
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
      ? document.createElement('sc-spectrum-workflow-tab')
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
