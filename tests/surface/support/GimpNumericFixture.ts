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
/** Represent the typed Comfy boundary while exercising the shared numeric owner and real GIMP control. */
import { NativeNumericControls } from '../../../frontend/comfyui/ui/surface/appearance/NativeNumericControls.js';
import type { NativeNumericField } from '../../../frontend/comfyui/ui/surface/appearance/ComfyNativeNumericField.js';
import { createGimpNumericSlider } from '../../../frontend/comfyui/ui/surface/appearance/GimpThemeRuntime.js';

/** Mount both numeric variants with the host's actual entry-and-stepper nesting. */
export function mountGimpNumericFixture(document: Document): () => void {
  const card = document.createElement('section');
  card.className = 'sugarcubes-cube-face__node-card';
  card.style.width = '360px';
  const node = document.createElement('article');
  node.id = 'numeric-node';
  node.dataset.scStyleNode = '';
  node.dataset.scStyleThemeRoot = '';
  const fields = document.createElement('div');
  fields.dataset.scStyleFields = '';
  node.append(fields);
  card.append(node);
  document.body.append(card);
  const nativeFields = [
    nativeField(document, fields, 'width', 'numeric-size', 1),
    nativeField(document, fields, 'opacity', 'numeric-scale', 2),
    nativeField(document, fields, 'denoise', 'numeric-slider', 3),
  ];
  const controls = new NativeNumericControls(createGimpNumericSlider);
  controls.reconcile(node, nativeFields, new Set());
  return () => {
    controls.clear();
    card.remove();
  };
}

/** Keep host values and subscriptions outside the presentation owner under test. */
function nativeField(
  document: Document,
  parent: HTMLElement,
  name: string,
  id: string,
  rowIndex: number,
): NativeNumericField {
  const row = document.createElement('div');
  row.id = id;
  row.dataset.scStyleRow = '';
  row.dataset.scStyleWidget = 'WidgetInputNumber';
  row.dataset.scStyleKind = 'scalar';
  row.dataset.scStyleColumn = '1';
  row.dataset.scStyleSpan = '2';
  row.style.cssText = `--sc-field-row:${String(rowIndex)};--sc-field-column:1;--sc-field-span:2`;
  const layout = document.createElement('div');
  layout.dataset.scStyleLayout = '';
  layout.dataset.scStyleControl = '';
  const label = document.createElement('span');
  label.textContent = name;
  const container = document.createElement('div');
  const frame = document.createElement('div');
  const spinner = document.createElement('div');
  spinner.dataset.scStyleEditFrame = '';
  const entry = document.createElement('div');
  entry.dataset.numericEntry = '';
  const input = document.createElement('input');
  input.setAttribute('role', 'spinbutton');
  entry.append(input);
  const decrement = document.createElement('button');
  decrement.type = 'button';
  decrement.setAttribute('aria-label', 'Decrement');
  const increment = document.createElement('button');
  increment.type = 'button';
  increment.setAttribute('aria-label', 'Increment');
  spinner.append(decrement, entry, increment);
  frame.append(spinner);
  container.append(frame);
  layout.append(label, container);
  row.append(layout);
  parent.append(row);
  let value = name === 'width' ? 512 : 0.75;
  input.value = String(value);
  const listeners = new Set<() => void>();
  return {
    row,
    container,
    spinner,
    input,
    increment,
    decrement,
    name,
    label: name,
    minimum: 0,
    maximum: name === 'width' ? 16384 : 1,
    step: name === 'width' ? 8 : 0.01,
    precision: name === 'width' ? 0 : 2,
    rangeFill: null,
    hasNativeSlider: false,
    readValue: () => value,
    disabled: () => false,
    writeValue: (next) => {
      value = next;
      input.value = String(next);
      for (const listener of listeners) listener();
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}
