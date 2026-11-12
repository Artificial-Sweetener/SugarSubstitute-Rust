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
/** Prove native choice locks retain Comfy menu rendering and block incompatible selection. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeChoiceRestrictions } from '../../../frontend/comfyui/ui/surface/NativeChoiceRestrictions.js';

afterEach(() => document.body.replaceChildren());

test('locks only the owned native portal, renders explanations literally, and restores its annotations', () => {
  const root = document.createElement('div');
  const trigger = document.createElement('button');
  trigger.setAttribute('role', 'combobox');
  trigger.setAttribute('aria-controls', 'native-list');
  root.append(trigger);
  const list = document.createElement('div');
  list.id = 'native-list';
  list.setAttribute('role', 'listbox');
  const options = ['euler', 'custom'].map((value) => {
    const option = document.createElement('div');
    option.setAttribute('role', 'option');
    const label = document.createElement('span');
    label.textContent = value;
    option.append(label);
    return option;
  });
  list.append(...options);
  document.body.append(root, list);
  const adapter = new NativeChoiceRestrictions(root);
  adapter.update([
    { value: 'euler', blockers: [] },
    { value: 'custom', blockers: ['<img src=x onerror=attack()>'] },
  ]);
  const custom = options[1];
  const euler = options[0];
  if (!custom || !euler) throw new Error('Native options missing');
  expect(custom.getAttribute('aria-disabled')).toBe('true');
  expect(custom.title).toContain('<img');
  expect(custom.querySelector('img')).toBeNull();
  const select = jest.fn();
  custom.addEventListener('click', select);
  custom.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
  expect(select).not.toHaveBeenCalled();
  expect(euler.getAttribute('aria-disabled')).toBeNull();
  adapter.dispose();
  expect(custom.getAttribute('aria-disabled')).toBeNull();
  expect(custom.querySelector('[data-sc-choice-lock]')).toBeNull();
});
