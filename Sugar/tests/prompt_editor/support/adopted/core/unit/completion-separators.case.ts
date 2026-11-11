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
/** Independent ordinary continuation contracts preserve authored separators and exact source boundaries. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  parsePrompt,
  prepareTagQueries,
  tagReplacement,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';

test('keyboard completion reuses the adjacent authored comma and horizontal whitespace', () => {
  for (const [source, caret, from, to, tag, text, fallback] of [
    ['soft lght, next', 6, 0, 11, 'soft_light', 'soft light, ', false],
    ['soft li,\tnext', 7, 0, 9, 'soft_light', 'soft light,\t', false],
    ['soft ght  ,\t next', 5, 0, 13, 'soft_light', 'soft light  ,\t ', false],
    ['🙂\r\nsoft lght,\t\rnext', 10, 4, 15, 'soft_light', 'soft light,\t', false],
    ['blue ha, next', 7, 5, 9, 'hair', 'hair, ', true],
    ['"soft li", next', 8, 0, 11, '"soft_light"', '"soft light", ', false],
    ['soft li,, next', 7, 0, 8, 'soft_light', 'soft light,', false],
  ] as const) {
    const resolved = prepareTagQueries(parsePrompt(source)).queryAt(caret, false);
    assert.ok(resolved);
    const query = fallback ? resolved.fallback : resolved;
    assert.ok(query);
    const replacement = tagReplacement(source, query, tag, true);
    assert.deepEqual(
      [replacement.range.from, replacement.range.to, replacement.text === text],
      [from, to, true],
      'Authored separator replacement differs',
    );
    const pointer = tagReplacement(source, query, tag, false);
    assert.equal(pointer.range.to <= query.activeTagEnd, true);
    assert.equal(pointer.text.endsWith(', '), false);
  }
});

test('unmatched right text, hard lines and enclosing syntax retain their independent boundaries', () => {
  for (const [source, caret, to] of [
    ['soft tail, next', 5, 5],
    ['soft li \r\n, next', 7, 7],
    ['soft li\r, next', 7, 7],
    ['(soft li:1.20), next', 8, 8],
    ['soft li\u00a0, next', 7, 7],
  ] as const) {
    const query = prepareTagQueries(parsePrompt(source)).queryAt(caret, false);
    assert.ok(query);
    const replacement = tagReplacement(source, query, 'soft_light', true);
    assert.equal(replacement.range.to, to);
    assert.equal(replacement.text === 'soft light, ', true);
  }
});
