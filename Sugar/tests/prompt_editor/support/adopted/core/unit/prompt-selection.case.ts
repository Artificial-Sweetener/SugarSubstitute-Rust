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
/** Independent canonical prompt-segment and Unicode word specimens. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { parsePrompt } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  promptWordRange,
  segmentSelectionRange,
} from '../../../../../../frontend/sugarprompt/core/src/prompt/selection.ts';

test('segment selection omits only leading ASCII horizontal whitespace and retains its exact trailing source', () => {
  const document = parsePrompt(' \talpha beta  , \r\n日 本\t, \u00a0tail\r');
  assert.deepEqual(document.segments.map(segmentSelectionRange), [
    { from: 2, to: 14 },
    { from: 18, to: 22 },
    { from: 24, to: 30 },
  ]);
  assert.deepEqual(parsePrompt('\r\n alpha').segments.map(segmentSelectionRange), [
    { from: 0, to: 8 },
  ]);
});

test('prompt words preserve scalar letters and their combining marks without swallowing punctuation or emoji marks', () => {
  for (const [source, position, expected] of [
    ['soft-light_name, tail', 5, { from: 0, to: 15 }],
    ['a  beta', 2, { from: 2, to: 2 }],
    ['a  beta', 3, { from: 3, to: 7 }],
    ['日𐐀²_Ⅳ-z!', 3, { from: 0, to: 8 }],
    ['e\u0301lan', 1, { from: 0, to: 5 }],
    ['e\u0301lan', 2, { from: 0, to: 5 }],
    ['cafe\u0301', 5, { from: 0, to: 5 }],
    ['✈️tail', 1, { from: 1, to: 1 }],
    ['✈️tail', 2, { from: 2, to: 6 }],
    ['\u0301abc', 0, { from: 0, to: 0 }],
    ['\u0301abc', 1, { from: 1, to: 4 }],
    ['\u0301\u0301', 1, { from: 1, to: 1 }],
    ['🙂tail', 2, { from: 2, to: 6 }],
    ['', 0, { from: 0, to: 0 }],
    ['a\r\nb', 2, { from: 2, to: 2 }],
    ['a\u{2ebf0}b', 1, { from: 0, to: 1 }],
    ['a\u{2ebf0}b', 3, { from: 3, to: 4 }],
  ] as const)
    assert.deepEqual(promptWordRange(source, position), expected);
  for (const offset of [-1, 1, 4, 0.5, NaN])
    assert.throws(() => promptWordRange('🙂a', offset), RangeError);
});
