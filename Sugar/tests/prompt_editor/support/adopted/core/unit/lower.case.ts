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
/** Independent Unicode identity specimens distinguish lowercase from search folding. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  unicodeLowerCase,
  caseFold,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';

test('lowercase preserves contextual identity and expanding mappings without search folding', () => {
  for (const [input, expected] of [
    ['STRASSE Straße', 'strasse straße'],
    ['ΟΣ ΟΣΑ Σ AΣ AΣA', 'ος οσα σ aς aσa'],
    ['AΣ\u0301 AΣ\u0301A', 'aς\u0301 aσ\u0301a'],
    ['A\u0345Σ AΣ\u0345A', 'a\u0345ς aσ\u0345a'],
    ['Σ\u0345 \u0345Σ', 'σ\u0345 \u0345σ'],
    ["AΣ'A AΣ' AΣ.A AΣ0A", "aσ'a aς' aσ.a aς0a"],
    ['İ 𐐀 K ẞ', 'i\u0307 𐐨 k ß'],
    ['\ud800Σ\udfff', '\ud800σ\udfff'],
    ['\u1c89', '\u1c89'],
  ])
    assert.equal(unicodeLowerCase(input!), expected);
  assert.notEqual(unicodeLowerCase('ΟΣ Straße'), caseFold('ΟΣ Straße'));
});
