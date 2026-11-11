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
/** Prove unchanged toolbar mounts perform no forced style reads while native changes still propagate. */
import { jest } from '@jest/globals';
import { NativeOverrideToolbarLayout } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeOverrideToolbarLayout.js';

test('retains native geometry observation without rereading styles on every unrelated refresh', async () => {
  const bar = document.createElement('div');
  const native = document.createElement('div');
  native.className = 'actionbar';
  native.style.position = 'fixed';
  bar.append(native);
  document.body.append(bar);
  const read = jest.spyOn(window, 'getComputedStyle');
  const changed = jest.fn();
  const layout = new NativeOverrideToolbarLayout();
  layout.subscribe(changed);
  layout.mount(bar);
  read.mockClear();
  for (let index = 0; index < 100; index++) layout.mount(bar);
  expect(read).not.toHaveBeenCalled();
  const child = document.createElement('div');
  child.className = 'unrelated';
  native.append(child);
  await Promise.resolve();
  expect(read).not.toHaveBeenCalled();
  native.style.position = 'relative';
  native.classList.add('docked');
  await Promise.resolve();
  expect(read).toHaveBeenCalled();
  expect(changed).toHaveBeenCalledTimes(2);
  layout.dispose();
  bar.remove();
  read.mockRestore();
});
