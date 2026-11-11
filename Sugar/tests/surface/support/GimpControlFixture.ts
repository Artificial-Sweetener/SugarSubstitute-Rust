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
/** Mount production GIMP Vue controls while exposing only their existing public adapter contracts. */
import { EditorView } from '@codemirror/view';
import { installSugarPromptStyles } from '../../../frontend/comfyui/ui/prompt_editor/runtime/SugarPromptStyles.js';
import {
  createGimpButton,
  createGimpCheckbox,
  createGimpSpinScale,
  createGimpFinderInput,
  createGimpSegments,
  createGimpEditorField,
} from '../../../frontend/comfyui/ui/surface/appearance/GimpThemeRuntime.js';
import { gimpTokenDeclarations } from '../../../frontend/comfyui/ui/surface/appearance/GimpThemeTokens.js';
import type { NumericSliderControl } from '../../../frontend/comfyui/ui/surface/appearance/NativeNumericControls.js';
import type { FinderInputControl } from '../../../frontend/comfyui/ui/surface/appearance/FinderControls.js';
import { mountGimpNumericFixture } from './GimpNumericFixture.js';

declare global {
  interface Window {
    gimpFixture: {
      slider: NumericSliderControl;
      checkbox: HTMLElement & { checked: boolean; disabled: boolean };
      search: FinderInputControl;
      prompt: EditorView;
      dispose(): void;
      edits: { checkbox: number; slider: number; search: number };
    };
  }
}

/** Retain controls and editor identity while browser tests exercise actual Vue lifecycles. */
export function mountGimpControlFixture(): void {
  document.documentElement.dataset.scVisualStyle = 'gimp';
  document.body.style.cssText = gimpTokenDeclarations(true);
  const button = createGimpButton(document);
  button.id = 'command';
  button.textContent = '<b>Literal command</b>';
  const checkbox = createGimpCheckbox(document);
  checkbox.id = 'enabled';
  checkbox.setAttribute('aria-label', 'Enabled');
  const slider = createGimpSpinScale(document);
  slider.root.id = 'scale';
  slider.root.dataset.scNumberLabel = 'Denoise';
  slider.synchronize(0, 1, 0.01, false, 0.6);
  const search = createGimpFinderInput(document);
  search.id = 'search';
  search.value = 'retained query';
  const segments = createGimpSegments(document);
  segments.segments[0].textContent = 'Off';
  segments.segments[1].textContent = 'On';
  segments.group.append(...segments.segments);
  segments.group.id = 'segments';
  segments.group.activeid = segments.segments[0].id;
  const field = createGimpEditorField(document),
    editor = document.createElement('div');
  editor.className = 'sugarprompt-editor';
  installSugarPromptStyles(document);
  field.root.dataset.scEditorField = 'gimp';
  field.root.style.cssText = 'display:block;width:300px;height:128px';
  document.body.append(button, checkbox, slider.root, search, segments.group, field.root);
  field.attachEditor(editor);
  const prompt = new EditorView({
    doc: 'retained document',
    extensions: [EditorView.lineWrapping],
    parent: editor,
  });
  prompt.contentDOM.id = 'editor';
  const releaseNumbers = mountGimpNumericFixture(document);
  const edits = { checkbox: 0, slider: 0, search: 0 };
  checkbox.addEventListener('change', () => edits.checkbox++);
  slider.root.addEventListener('input', () => edits.slider++);
  search.addEventListener('input', () => edits.search++);
  window.gimpFixture = {
    slider,
    checkbox,
    search,
    prompt,
    edits,
    dispose: () => {
      releaseNumbers();
      prompt.destroy();
    },
  };
}
