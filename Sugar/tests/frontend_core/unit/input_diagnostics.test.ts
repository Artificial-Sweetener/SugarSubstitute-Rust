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
/** Prove the input diagnostics name every holder of input and nothing else. */
import { afterEach, expect, test } from '@jest/globals';
import { readInputDiagnostics } from '../../../frontend/comfyui/ui/debug/InputDiagnostics.js';

afterEach(() => document.body.replaceChildren());

test('names held, inert and transitioning surfaces and the focused element', () => {
  const held = document.createElement('section');
  held.className = 'sugarcubes-workflow-masonry';
  held.setAttribute('data-sc-input-held', '');
  const inert = document.createElement('div');
  inert.className = 'ph-no-capture';
  inert.setAttribute('inert', '');
  const moving = document.createElement('div');
  moving.dataset.scMasonryTransition = 'enter';
  const field = document.createElement('input');
  field.className = 'cfg';
  field.value = 'secret prompt text';
  document.body.append(held, inert, moving, field);
  field.focus();

  const state = readInputDiagnostics(document);

  expect(state.blockers).toEqual([
    { element: 'section.sugarcubes-workflow-masonry', reason: 'held' },
    { element: 'div.ph-no-capture', reason: 'inert' },
    { element: 'div', reason: 'transition' },
  ]);
  expect(state.activeElement).toBe('input.cfg');
  expect(JSON.stringify(state)).not.toContain('secret prompt text');
});

test('reports nothing holding input on a quiet page', () => {
  expect(readInputDiagnostics(document).blockers).toEqual([]);
});
