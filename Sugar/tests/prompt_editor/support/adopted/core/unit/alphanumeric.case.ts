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
/** Independent pinned alphanumeric identities cover shared apostrophe and word semantics. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { parsePrompt } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { unicodeAlphanumeric } from '../../../../../../frontend/sugarprompt/core/src/prompt/alphanumeric.ts';

test('alphanumeric classification retains numeric letters and excludes marks, punctuation and newer assignments', () => {
  for (const text of ['a', '日', '𐐀', '\u{1e4d0}', '²', 'Ⅳ', '½', 'a²日𐐀'])
    assert.equal(unicodeAlphanumeric(text), true);
  for (const text of [
    '',
    'e\u0301',
    '\u0301',
    '_',
    ' ',
    '🙂',
    '\u1c89',
    '\u{2ebf0}',
    '\ud800',
    '\udfff',
    'a\u{2ebf0}',
  ])
    assert.equal(unicodeAlphanumeric(text), false);
});

test('apostrophes next to scalars unknown to Unicode 15 remain structural quotes', () => {
  for (const character of ['\u1c89', '\u{2ebf0}', '\ud800'])
    assert.equal(parsePrompt(`${character}'${character}, tail`).segments.length, 1);
  for (const character of ['日', '𐐀', '²', 'Ⅳ', '\u{1e4d0}'])
    assert.equal(parsePrompt(`${character}'${character}, tail`).segments.length, 2);
});
