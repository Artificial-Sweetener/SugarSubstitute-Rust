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
import {
  createPhotoProButton,
  createPhotoProCheckbox,
  createPhotoProEditorField,
  createPhotoProFinderInput,
  createPhotoProSegments,
  createPhotoProSlider,
  createPhotoProTablist,
  createPhotoProWorkflowTab,
} from './PhotoProControls.js';
import {
  createGimpButton,
  createGimpCheckbox,
  createGimpNumericSlider,
  createGimpSegments,
  createGimpFinderInput,
  createGimpEditorField,
  createGimpTablist,
  createGimpWorkflowTab,
} from './GimpThemeRuntime.js';
import type { NumericSliderControl } from './NativeNumericControls.js';
import type { NumericSliderSpec } from './NumericSliderPresentation.js';
import type { BooleanCheckboxControl } from './NativeToggleRows.js';
import type { BooleanSegmentsControls } from './NativeBooleanSegments.js';
import type { FinderInputControl } from './FinderControls.js';
import type { VisualStyle } from './VisualStyle.js';
import { adoptEditorFieldPadding, type EditorFieldControl } from './EditorFieldControl.js';
import { relocateControl } from '../../core/ControlPlacement.js';
import {
  createNativeActivationSwitch,
  createCheckboxActivation,
  type ActivationSwitchControl,
} from './ActivationSwitchControl.js';

/** Select activation presentation without taking ownership of node state or history. */
export function createActivationSwitch(document: Document): ActivationSwitchControl {
  const result =
    readOwnedVisualStyle(document) === 'gimp'
      ? createCheckboxActivation(createGimpCheckbox(document))
      : readOwnedVisualStyle(document) === 'spectrum'
        ? createSpectrumActivationSwitch(document)
        : readOwnedVisualStyle(document) === 'photopro'
          ? createCheckboxActivation(createPhotoProCheckbox(document))
          : createNativeActivationSwitch(document);
  result.control.dataset.scActivationSwitch = '';
  return result;
}

/** Create an owned command button with the selected system's real interaction implementation. */
export function createButton(document: Document): HTMLElement {
  const button =
    readOwnedVisualStyle(document) === 'gimp'
      ? createGimpButton(document)
      : readOwnedVisualStyle(document) === 'spectrum'
        ? createSpectrumButton(document)
        : readOwnedVisualStyle(document) === 'photopro'
          ? createPhotoProButton(document)
          : document.createElement('fluent-button');
  button.dataset.scButton = '';
  return button;
}

/** Normalize design-system numeric APIs before binding to a native Comfy field. */
export function createNumericSlider(
  document: Document,
  field: NumericSliderSpec,
): NumericSliderControl {
  const style = readOwnedVisualStyle(document);
  if (style === 'gimp') return createGimpNumericSlider(document, field);
  return style === 'spectrum'
    ? createSpectrumSlider(document)
    : style === 'photopro'
      ? createPhotoProSlider(document)
      : createFluentNumericSlider(document);
}

/** Keep checked-state semantics independent of the selected component library. */
export function createBooleanCheckbox(document: Document): BooleanCheckboxControl {
  const style = readOwnedVisualStyle(document);
  if (style === 'gimp') return createGimpCheckbox(document);
  return style === 'spectrum'
    ? createSpectrumCheckbox(document)
    : style === 'photopro'
      ? createPhotoProCheckbox(document)
      : createBooleanCheckboxControl(document);
}

/** Keep native boolean edits and undo history under the existing shared binding owner. */
export function createBooleanSegments(document: Document): BooleanSegmentsControls {
  const style = readOwnedVisualStyle(document);
  if (style === 'gimp') return createGimpSegments(document);
  return style === 'spectrum'
    ? createSpectrumSegments(document)
    : style === 'photopro'
      ? createPhotoProSegments(document)
      : createBooleanSegmentControls(document);
}

/** Create a real accessible search field in the selected visual system. */
export function createFinderInput(document: Document): FinderInputControl {
  const style = readOwnedVisualStyle(document);
  if (style === 'gimp') return createGimpFinderInput(document);
  return style === 'spectrum'
    ? createSpectrumFinderInput(document)
    : style === 'photopro'
      ? createPhotoProFinderInput(document)
      : createFinderInputControl(document);
}

/** Select the existing provider's input asset for an editor with its own canonical document. */
export function createEditorField(document: Document, style: VisualStyle): EditorFieldControl {
  const field =
    style === 'gimp'
      ? createGimpEditorField(document)
      : style === 'fluent'
        ? createFluentEditorField(document)
        : style === 'spectrum'
          ? createSpectrumEditorField(document)
          : style === 'photopro'
            ? createPhotoProEditorField(document)
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
  const style = readOwnedVisualStyle(document);
  const group =
    style === 'gimp'
      ? createGimpTablist(document)
      : style === 'spectrum'
        ? createSpectrumTablist(document)
        : style === 'photopro'
          ? createPhotoProTablist(document)
          : createFluentWorkflowTablist();
  group.dataset.scWorkflowTabs = '';
  return group;
}

/** Create the selected provider's workflow tab while retaining shared drag and close behavior. */
export function createWorkflowTab(document: Document): HTMLElement {
  const style = readOwnedVisualStyle(document);
  const tab =
    style === 'gimp'
      ? createGimpWorkflowTab(document)
      : style === 'spectrum'
        ? document.createElement('sc-spectrum-workflow-tab')
        : style === 'photopro'
          ? createPhotoProWorkflowTab(document)
          : createFluentWorkflowTab(document);
  tab.dataset.scWorkflowTab = '';
  return tab;
}

/** Choose the provider's accessory slots without leaking library-specific DOM into shared views. */
export function workflowTabAccessorySlot(document: Document, position: 'start' | 'end'): string {
  return readOwnedVisualStyle(document) === 'fluent' ? position : '';
}

/** Name the already registered provider element for declarative Vue rendering. */
export function buttonTag(style: VisualStyle): string {
  if (style === 'gimp') return 'sc-gimp-button';
  return style === 'comfy' || style === 'photopro'
    ? 'button'
    : style === 'spectrum'
      ? 'sp-action-button'
      : 'fluent-button';
}

/** Select a registered text-field adapter for declarative search views. */
export function searchInputTag(style: VisualStyle): string {
  if (style === 'gimp') return 'sc-gimp-search';
  return style === 'comfy'
    ? 'input'
    : style === 'spectrum'
      ? 'sc-spectrum-search'
      : style === 'photopro'
        ? 'sc-photopro-search'
        : 'fluent-text-input';
}
