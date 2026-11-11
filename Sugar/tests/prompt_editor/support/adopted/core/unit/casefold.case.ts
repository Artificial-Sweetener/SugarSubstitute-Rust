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
/** Canonical ASCII and pinned Unicode specimens protect the common identity-folding path. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { caseFold } from '../../../../../../frontend/sugarprompt/core/src/index.ts';

test('every ASCII scalar folds only the canonical Latin capitals', () => {
  const prefix = String.fromCharCode(...Array.from({ length: 65 }, (_, point) => point)),
    suffix = String.fromCharCode(...Array.from({ length: 37 }, (_, offset) => offset + 91)),
    input = prefix + 'ABCDEFGHIJKLMNOPQRSTUVWXYZ' + suffix,
    expected = prefix + 'abcdefghijklmnopqrstuvwxyz' + suffix;
  assert.equal(input.length, 128);
  assert.equal(caseFold(input) === expected, true, 'ascii-folding-differs');
});

test('ASCII boundaries compose with full expanding Unicode and supplementary mappings', () => {
  const input = 'A@Z[az\u007f\u0080 ÀÖØÞ İßẞ Σς Kﬃ 𐐀 \ud800X\udfff \u1c89',
    expected = 'a@z[az\u007f\u0080 àöøþ i\u0307ssss σσ kffi 𐐨 \ud800x\udfff \u1c89';
  assert.equal(caseFold(input) === expected, true, 'mixed-folding-differs');
  assert.equal(caseFold(expected) === expected, true, 'folding-is-not-idempotent');
});
