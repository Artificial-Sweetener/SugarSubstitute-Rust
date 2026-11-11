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
/** Independent clock/identity specimens prove wheel authorization at its pure owner. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { WheelIntent } from '../../../../../../frontend/sugarprompt/editor/src/wheel-intent.ts';
import {
  validateWheelMode,
  initialWheelMode,
} from '../../../../../../frontend/sugarprompt/editor/src/wheel-mode.ts';

test('stable dwell includes the 400ms boundary and six-pixel radius; premature wheel restarts dwell', () => {
  const intent = new WheelIntent(),
    token = Symbol();
  intent.move(token, { x: 0, y: 0 }, 0);
  intent.move(token, { x: 6, y: 0 }, 100);
  assert.equal(intent.ready(token, 'hover_dwell', 399), false);
  assert.equal(intent.ready(token, 'hover_dwell', 400), true);
  intent.clear();
  intent.move(token, { x: 0, y: 0 }, 0);
  intent.move(token, { x: 4, y: 5 }, 100);
  assert.equal(intent.ready(token, 'hover_dwell', 400), false);
  assert.equal(intent.allow(token, 'hover_dwell', 450), false);
  assert.equal(intent.ready(token, 'hover_dwell', 849), false);
  assert.equal(intent.allow(token, 'hover_dwell', 850), true);
});

test('activation belongs to exactly one token and is retired by source/context/focus lifetime', () => {
  const intent = new WheelIntent(),
    first = Symbol(),
    second = Symbol();
  intent.move(first, { x: 0, y: 0 }, 0);
  assert.equal(intent.allow(first, 'focus_required', 1000), false);
  intent.activate(first);
  assert.equal(intent.allow(first, 'focus_required', 1000), true);
  intent.move(second, { x: 20, y: 0 }, 1010);
  assert.equal(intent.allow(second, 'focus_required', 1020), false);
  intent.leave();
  assert.equal(intent.ready(first, 'focus_required', 1100), true);
  intent.activate(second);
  assert.equal(intent.allow(first, 'focus_required', 1110), false);
  assert.equal(intent.allow(second, 'focus_required', 1110), true);
  intent.clear();
  assert.equal(intent.ready(second, 'focus_required', 1120), false);
  assert.throws(() => validateWheelMode('automatic'), TypeError);
  assert.throws(() => validateWheelMode(null), TypeError);
  assert.throws(() => initialWheelMode(null), TypeError);
  assert.equal(initialWheelMode(undefined), 'hover_dwell');
});

test('an armed burst stays token-specific; target departure requires independent authorization', () => {
  const intent = new WheelIntent(),
    first = Symbol(),
    second = Symbol();
  intent.move(first, { x: 0, y: 0 }, 0);
  assert.equal(intent.allow(first, 'hover_dwell', 400), true);
  assert.equal(intent.allow(first, 'hover_dwell', 649), true);
  assert.equal(intent.allow(first, 'hover_dwell', 899), true);
  intent.move(second, { x: 20, y: 0 }, 900);
  assert.equal(intent.allow(second, 'hover_dwell', 901), false);
  assert.equal(intent.allow(first, 'hover_dwell', 902), false);
  assert.equal(intent.ready(second, 'hover_dwell', 1300), false);
  assert.equal(intent.allow(second, 'hover_dwell', 1301), true);
  intent.leave();
  assert.equal(intent.allow(second, 'hover_dwell', 1302), false);
});
