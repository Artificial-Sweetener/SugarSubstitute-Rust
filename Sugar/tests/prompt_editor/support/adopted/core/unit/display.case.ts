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
/** Independent literal-storage display contracts use exact-source UTF-16 ranges. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { parenthesisEscapes } from '../../../../../../frontend/sugarprompt/core/src/index.ts';

test('only backslashes immediately before parentheses form concealed pairs', () => {
  assert.deepEqual(parenthesisEscapes(String.raw`\(cat\) \x \[y\] tail` + '\\'), [
    { from: 0, to: 2 },
    { from: 5, to: 7 },
  ]);
  assert.deepEqual(parenthesisEscapes(String.raw`\\(x\\)`), [
    { from: 1, to: 3 },
    { from: 5, to: 7 },
  ]);
  assert.deepEqual(parenthesisEscapes(''), []);
  assert.deepEqual(parenthesisEscapes('(cat:1.20)'), []);
});

test('escape ranges preserve astral, combining, CRLF and malformed source coordinates', () => {
  const source = '🙂\r\n\\(e\u0301\\)\r\\(\ud800';
  assert.deepEqual(parenthesisEscapes(source), [
    { from: 4, to: 6 },
    { from: 8, to: 10 },
    { from: 11, to: 13 },
  ]);
});
