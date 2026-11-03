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
/** Prove the shared ordinal palette and which base each kind of input mask rotates from. */
import {
  maskColor,
  regionMaskColor,
} from '../../../frontend/comfyui/ui/input_canvas/domain/MaskColorScheme.js';
import { parseRegionHex, regionHue } from '../../../frontend/sugarprompt/core/src/index.js';
import { prepareRegionColor } from '../../../frontend/sugarprompt/editor/src/region-color.js';

test('hue rotation follows the mask count', () => {
  expect(regionHue(10, 0, 4)).toBe(10);
  expect(regionHue(10, 1, 2)).toBe(190);
  expect([1, 2].map((index) => regionHue(10, index, 3))).toEqual([160, 220]);
  expect([1, 2, 3].map((index) => regionHue(10, index, 4))).toEqual([100, 190, 280]);
  expect([1, 2, 3, 4, 5].map((index) => regionHue(10, index, 6))).toEqual([70, 130, 190, 250, 310]);
  expect(regionHue(-1, 1, 2)).toBe(179);
  expect(regionHue(10, 3, 1)).toBe(10);
});

test('the first mask keeps the accent and later masks are vivid rotations', () => {
  expect(maskColor('#479EF5', 0, 3)).toBe('#479ef5');
  const second = parseRegionHex(maskColor('#806040', 1, 2));
  expect(second).not.toBeNull();
  if (!second) return;
  const max = Math.max(second.r, second.g, second.b),
    min = Math.min(second.r, second.g, second.b);
  expect(max).toBeGreaterThanOrEqual(220);
  expect(Math.round(((max - min) * 255) / max)).toBeGreaterThanOrEqual(199);
  expect(second.b).toBe(max);
  expect(maskColor('#808080', 1, 2)).toMatch(/^#[\da-f]{6}$/);
  expect(() => maskColor('red', 1, 2)).toThrow('#rrggbb');
});

test('a region mask has the colour SugarPrompt gives the same separator', () => {
  for (const total of [1, 2, 3, 4, 5, 7])
    for (let ordinal = 0; ordinal < total; ordinal++) {
      const prompt = prepareRegionColor(ordinal, total).normal;
      const channels = /^rgb\((\d+), (\d+), (\d+)\)$/.exec(prompt);
      const expected = channels
        ? '#' +
          channels
            .slice(1)
            .map((channel) => Number(channel).toString(16).padStart(2, '0'))
            .join('')
        : prompt;
      expect(regionMaskColor(ordinal, total)).toBe(expected);
    }
});
