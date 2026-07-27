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
/** Verify Cube-only prompt textarea expansion. */

import { describe, expect, test } from '@jest/globals';

import { fitCubeFacePromptTextarea } from '../../frontend/comfyui/ui/surface/CubeFacePromptTextarea.js';

describe('fitCubeFacePromptTextarea', () => {
  test('uses the complete content height and disables the internal scrollbar', () => {
    const textarea = document.createElement('textarea');
    Object.defineProperty(textarea, 'scrollHeight', { configurable: true, value: 168 });

    expect(fitCubeFacePromptTextarea(textarea)).toBe(168);
    expect(textarea.style.height).toBe('168px');
    expect(textarea.style.overflowY).toBe('hidden');
    expect(textarea.style.getPropertyPriority('height')).toBe('important');
  });
});
