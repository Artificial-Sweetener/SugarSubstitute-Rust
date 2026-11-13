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
/** Actual native segmentation output proves one-line reuse and retirement without browser input or another source owner. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Text } from '@codemirror/state';
import { GraphemeLineCache } from '../../../../../../frontend/sugarprompt/editor/src/native-graphemes.ts';

test('unchanged native document and same-line caret movement reuse the actual segmentation result', () => {
  const document = Text.of(['a👩‍💻b', 'second']),
    cache = new GraphemeLineCache();
  const first = cache.prepare(document, 1);
  assert.strictEqual(cache.prepare(document, 4), first);
  assert.strictEqual(cache.prepare(document, 7).segments, first.segments);
});

test('only the current line is retained; a different line and authoritative document invalidate the actual output', () => {
  const document = Text.of(['first', 'second']),
    cache = new GraphemeLineCache();
  const first = cache.prepare(document, 1),
    second = cache.prepare(document, 7);
  assert.notStrictEqual(second.segments, first.segments);
  assert.notStrictEqual(cache.prepare(document, 1).segments, first.segments);
  const replaced = Text.of(['first', 'changed']);
  assert.notStrictEqual(cache.prepare(replaced, 7).segments, second.segments);
});

test('explicit source retirement and native teardown release retained segmentation even when the document is unchanged', () => {
  const document = Text.of(['same']),
    cache = new GraphemeLineCache();
  const first = cache.prepare(document, 1);
  cache.clear();
  assert.notStrictEqual(cache.prepare(document, 1), first);
});
