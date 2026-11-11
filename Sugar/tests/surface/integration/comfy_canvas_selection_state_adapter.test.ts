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
/** Verify root selection preservation across Cube editor navigation. */

import { jest } from '@jest/globals';
import { ComfyCanvasSelectionStateAdapter } from '../../../frontend/comfyui/ui/surface/ComfyCanvasSelectionStateAdapter.js';

test('restores the exact selected root objects and refreshes Comfy state', () => {
  const first = { id: 'first' };
  const second = { id: 'second' };
  const selectedItems = new Set<unknown>([first, second]);
  const updateSelectedItems = jest.fn();
  const adapter = new ComfyCanvasSelectionStateAdapter(selectedItems, updateSelectedItems);
  const captured = adapter.capture();

  selectedItems.clear();
  selectedItems.add({ id: 'nested' });
  adapter.restore(captured);

  expect([...selectedItems]).toEqual([first, second]);
  expect(updateSelectedItems).toHaveBeenCalledTimes(1);
});
