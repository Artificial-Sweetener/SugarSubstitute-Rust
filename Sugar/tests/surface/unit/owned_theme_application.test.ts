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
/** Prove repeated accepted field themes preserve descendant style identity. */
import { jest } from '@jest/globals';
import { applyOwnedTheme } from '../../../frontend/comfyui/ui/surface/appearance/ThemeService.js';

test('reapplying the same field theme does not invalidate descendant styles', () => {
  const element = document.createElement('div');
  document.body.append(element);
  applyOwnedTheme(element, true, 'spectrum');
  const write = jest.spyOn(element, 'setAttribute');
  try {
    applyOwnedTheme(element, true, 'spectrum');
    expect(write).not.toHaveBeenCalled();
    applyOwnedTheme(element, false, 'spectrum');
    expect(element.dataset.scColorScheme).toBe('light');
  } finally {
    write.mockRestore();
    element.remove();
  }
});
