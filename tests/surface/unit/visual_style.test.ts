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
/** Prove persisted visual styles preserve legacy preferences and reject dynamic invalid values. */
import { expect, test } from '@jest/globals';
import { resolveVisualStyle } from '../../../frontend/comfyui/ui/surface/appearance/VisualStyle.js';

test.each(['comfy', 'fluent', 'spectrum'] as const)(
  'prefers saved %s over either legacy toggle',
  (style) => {
    expect(resolveVisualStyle(style, false)).toBe(style);
    expect(resolveVisualStyle(style, true)).toBe(style);
  },
);

test.each([undefined, null, false, true, '', 'Spectrum 2', {}, ['fluent']])(
  'validates unknown saved styles before applying the public legacy fallback: %p',
  (value) => {
    expect(resolveVisualStyle(value, true)).toBe('fluent');
    expect(resolveVisualStyle(value, false)).toBe('comfy');
    expect(resolveVisualStyle(value, 'true')).toBe('comfy');
  },
);
