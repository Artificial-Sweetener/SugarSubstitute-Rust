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
/** Independent readable endpoint, fade and integer-duration contracts have no browser or clock owner. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { titleMarqueeState } from '../../../../../../frontend/sugarprompt/editor/src/title-marquee.ts';
import { roundToEven } from '../../../../../../frontend/sugarprompt/editor/src/round-to-even.ts';

test('title starts for 900ms, crosses 44 pixels in one second and ends for 700ms before repeating', () => {
  assert.deepEqual(titleMarqueeState(899, 44), {
    phase: 'start',
    offset: 0,
    showLeftFade: false,
    showRightFade: true,
  });
  assert.deepEqual(titleMarqueeState(1400, 44), {
    phase: 'scroll',
    offset: 22,
    showLeftFade: true,
    showRightFade: true,
  });
  assert.deepEqual(titleMarqueeState(1900, 44), {
    phase: 'end',
    offset: 44,
    showLeftFade: true,
    showRightFade: false,
  });
  assert.equal(titleMarqueeState(2599, 44).phase, 'end');
  assert.equal(titleMarqueeState(2600, 44).phase, 'start');
  assert.equal(titleMarqueeState(900, 44).showLeftFade, false);
});

test('fitting labels stay still and positive half-duration ties share the geometry rounding owner', () => {
  for (const overflow of [-20, 0])
    assert.deepEqual(titleMarqueeState(2000, overflow), {
      phase: 'start',
      offset: 0,
      showLeftFade: false,
      showRightFade: false,
    });
  assert.deepEqual([roundToEven(2.5), roundToEven(3.5)], [2, 4]);
  assert.equal(titleMarqueeState(902, 0.11).phase, 'end');
  assert.equal(titleMarqueeState(903, 0.154).phase, 'scroll');
});
