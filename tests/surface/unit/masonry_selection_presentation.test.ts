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
/** Prove masonry suppresses native selection color while retaining graph and control feedback. */
import { afterEach, expect, test } from '@jest/globals';
import { ensureWorkflowMasonryStyles } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryStyles.js';

afterEach(() => {
  document.head.replaceChildren();
  document.body.replaceChildren();
});

/** Render native selection and execution borders on the same card in either presentation. */
function fixture(masonry: boolean) {
  const native = document.createElement('style');
  native.textContent =
    '.outline-node-component-outline { outline-color:blue; } .border-node-component-outline { border:3px solid blue; } .border-node-stroke-executing { border:3px solid green; } input { outline-color:blue; }';
  document.head.append(native);
  ensureWorkflowMasonryStyles(document);
  const surface = document.createElement('div');
  if (masonry) surface.className = 'sugarcubes-workflow-masonry';
  const card = document.createElement('article');
  if (masonry) card.className = 'sugarcubes-workflow-masonry__card';
  const root = document.createElement('div');
  root.className = 'lg-node outline-node-component-outline';
  const selection = document.createElement('div');
  selection.dataset.testid = 'node-state-outline-overlay';
  selection.className = 'border-node-component-outline';
  const execution = document.createElement('div');
  execution.dataset.testid = 'node-state-outline-overlay';
  execution.className = 'border-node-stroke-executing';
  const input = document.createElement('input');
  root.append(selection, execution, input);
  card.append(root);
  surface.append(card);
  document.body.append(surface);
  return { root, selection, execution, input, card };
}

test('hides the native selection stroke only in masonry and preserves editing and execution feedback', () => {
  const { root, selection, execution, input, card } = fixture(true);
  expect(getComputedStyle(root).outlineColor).toBe('transparent');
  expect(getComputedStyle(selection).borderColor).toBe('transparent');
  expect(getComputedStyle(execution).borderColor).toBe('green');
  expect(getComputedStyle(input).outlineColor).toBe('blue');
  expect(root.classList.contains('outline-node-component-outline')).toBe(true);
  card.className = '';
  expect(getComputedStyle(root).outlineColor).toBe('blue');
  expect(getComputedStyle(selection).borderColor).toBe('blue');
});

test('retains Comfy selection color for normal graph cards', () => {
  const { root, selection } = fixture(false);
  expect(getComputedStyle(root).outlineColor).toBe('blue');
  expect(getComputedStyle(selection).borderColor).toBe('blue');
});
