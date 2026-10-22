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
/** Prove ordinary and enhanced comboboxes share click-to-close behavior. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { acquireComboToggle } from '../../../frontend/comfyui/ui/surface/appearance/ComboToggleController.js';

afterEach(() => document.body.replaceChildren());

test('closes a host combobox when its projected search field is clicked again', () => {
  const frame = document.createElement('div');
  frame.dataset.scStyleComboFrame = '';
  const trigger = document.createElement('button');
  trigger.setAttribute('role', 'combobox');
  trigger.setAttribute('aria-expanded', 'true');
  trigger.setAttribute('aria-controls', 'host-popup');
  frame.append(trigger);
  const popup = document.createElement('div');
  popup.id = 'host-popup';
  const search = document.createElement('input');
  search.dataset.scInlineSearch = '';
  search.setAttribute('aria-expanded', 'true');
  popup.append(search);
  document.body.append(frame, popup);
  const release = acquireComboToggle(document);
  const escape = jest.fn(() => trigger.setAttribute('aria-expanded', 'false'));
  search.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') escape();
  });

  search.click();

  expect(escape).toHaveBeenCalledTimes(1);
  expect(trigger.getAttribute('aria-expanded')).toBe('false');
  release();
});

test('closes a short host combobox with no native search field', () => {
  const frame = document.createElement('div');
  frame.dataset.captureWheel = 'true';
  const trigger = document.createElement('button');
  trigger.setAttribute('role', 'combobox');
  trigger.setAttribute('aria-expanded', 'true');
  trigger.setAttribute('aria-controls', 'short-popup');
  frame.append(trigger);
  const popup = document.createElement('div');
  popup.id = 'short-popup';
  popup.setAttribute('role', 'listbox');
  document.body.append(frame, popup);
  const release = acquireComboToggle(document);
  const escape = jest.fn(() => trigger.setAttribute('aria-expanded', 'false'));
  popup.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') escape();
  });

  trigger.click();

  expect(escape).toHaveBeenCalledTimes(1);
  expect(trigger.getAttribute('aria-expanded')).toBe('false');
  release();
});

test('closes the enhanced model field from either its input or arrow', () => {
  const frame = document.createElement('div');
  frame.dataset.scStyleComboFrame = '';
  frame.className = 'sc-model-field';
  const input = document.createElement('input');
  input.setAttribute('role', 'combobox');
  input.setAttribute('aria-expanded', 'true');
  const arrow = document.createElement('button');
  arrow.className = 'sc-model-field-arrow';
  frame.append(input, arrow);
  document.body.append(frame);
  const release = acquireComboToggle(document);
  const escape = jest.fn(() => input.setAttribute('aria-expanded', 'false'));
  input.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') escape();
  });

  input.click();
  expect(escape).toHaveBeenCalledTimes(1);
  input.setAttribute('aria-expanded', 'true');
  arrow.click();
  expect(escape).toHaveBeenCalledTimes(2);
  release();
});
