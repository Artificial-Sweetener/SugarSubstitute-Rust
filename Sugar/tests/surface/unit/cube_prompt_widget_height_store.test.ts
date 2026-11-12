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
/** Verify transient prompt widget height ownership. */

import { describe, expect, test } from '@jest/globals';

import {
  clearCubePromptWidgetHeight,
  cubePromptWidgetHeight,
  setCubePromptWidgetHeight,
} from '../../../frontend/comfyui/ui/surface/CubePromptWidgetHeightStore.js';

describe('CubePromptWidgetHeightStore', () => {
  test('normalizes, deduplicates, and releases one graph-owned widget allocation', () => {
    const widget = { name: 'text', type: 'customtext' };

    expect(setCubePromptWidgetHeight(widget, 168)).toBe(true);
    expect(setCubePromptWidgetHeight(widget, 168.25)).toBe(false);
    expect(cubePromptWidgetHeight(widget)).toBe(168);

    clearCubePromptWidgetHeight(widget);

    expect(cubePromptWidgetHeight(widget)).toBeNull();
  });
});
