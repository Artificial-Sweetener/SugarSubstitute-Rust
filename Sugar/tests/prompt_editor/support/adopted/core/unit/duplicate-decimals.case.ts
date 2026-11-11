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
/** Exact independent decimal sums prove final-only rounding and preservation of existing fixed steps. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  addWeights,
  formatWeight,
  stepWeight,
} from '../../../../../../frontend/sugarprompt/core/src/normalization/decimals.ts';

test('exact addition retains significant digits, exponent expansion, signs and unrestricted coefficients', () => {
  assert.equal(addWeights('1.004', '0.004'), '1.008');
  assert.equal(formatWeight(addWeights(addWeights('1.004', '0.10'), '0.004')), '1.11');
  assert.equal(addWeights('1E3', '0.00001'), '1000.00001');
  assert.equal(addWeights('-1.125', '1.125'), '0.000');
  assert.equal(addWeights('999999999999999999999.99', '0.01'), '1000000000000000000000.00');
  assert.equal(stepWeight('-0.05', 'increase'), '0.00');
  assert.equal(stepWeight('1.005', 'increase'), '1.06');
  assert.throws(() => addWeights('NaN', '1'), TypeError);
});
