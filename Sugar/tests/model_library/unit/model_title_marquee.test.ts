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
/** Characterize the reference's readable holds, linear travel and cycle boundaries. */
import { expect, test } from '@jest/globals';
import { modelTitleMarquee } from '../../../frontend/comfyui/ui/model_library/ModelTitleMarquee.js';
import type { ModelTitleMarqueeState } from '../../../frontend/comfyui/ui/model_library/ModelTitleMarquee.js';

test.each<[number, ModelTitleMarqueeState['phase'], number, boolean, boolean]>([
  [0, 'start', 0, false, true],
  [899, 'start', 0, false, true],
  [900, 'scroll', 0, false, true],
  [1400, 'scroll', 22, true, true],
  [3890, 'scroll', 131.56, true, false],
  [3900, 'end', 132, true, false],
  [4599, 'end', 132, true, false],
  [4600, 'start', 0, false, true],
])('matches the reference at %i milliseconds', (elapsed, phase, offset, leftFade, rightFade) => {
  const state = modelTitleMarquee(elapsed, 132);
  expect(state.phase).toBe(phase);
  expect(state.offset).toBeCloseTo(offset, 8);
  expect(state.leftFade).toBe(leftFade);
  expect(state.rightFade).toBe(rightFade);
});

test('leaves fitting titles still and bounds negative measurements', () => {
  expect(modelTitleMarquee(2000, 0)).toEqual({
    phase: 'start',
    offset: 0,
    leftFade: false,
    rightFade: false,
  });
  expect(modelTitleMarquee(2000, -10)).toEqual(modelTitleMarquee(2000, 0));
});
