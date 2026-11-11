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
/** Independent scene specimens prove prepared source bounds and literal replacement without a browser. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  prepareSceneQueries,
  sceneReplacement,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';

test('physical scene slots retain marker indentation, blank right text and exact mixed endings', () => {
  const source = 'shared\r\n \t**  Ca \t\r\nbody\r**\n**Other';
  const prepared = prepareSceneQueries(source);
  const caret = source.indexOf('Ca') + 2;
  const query = prepared.queryAt(caret, false);
  assert.ok(query);
  assert.equal(query.prefix === '  Ca', true);
  assert.deepEqual(query.markerRange, { from: 10, to: 12 });
  const replacement = sceneReplacement(source, query, 'Cafe_(two:1.20)');
  assert.equal(
    source.slice(0, replacement.range.from) +
      replacement.text +
      source.slice(replacement.range.to) ===
      'shared\r\n \t**Cafe_(two:1.20) \t\r\nbody\r**\n**Other',
    true,
  );
  assert.equal(prepared.queryAt(caret - 1, false), null);
  assert.equal(prepared.queryAt(caret, true), null);
  assert.equal(prepared.queryAt(source.indexOf('body') + 2, false), null);
  assert.ok(prepared.queryAt(source.indexOf('\r**') + 3, false));
  assert.ok(Object.isFrozen(query.replacementRange));
});

test('scene commands reject forged/stale descriptors and destructive titles; editable scalar and CRLF bounds reject', () => {
  const source = '🙂\r\n**Ca';
  const prepared = prepareSceneQueries(source),
    query = prepared.queryAt(source.length, false);
  assert.ok(query);
  assert.throws(() => sceneReplacement(source, { ...query }, 'Cafe'), TypeError);
  assert.throws(() => sceneReplacement(source + 'x', query, 'Cafe'), TypeError);
  for (const title of ['', ' \t', 'a\nb', 'a\rb', 'a\u2028b', 'a\0b'])
    assert.throws(() => sceneReplacement(source, query, title), TypeError);
  for (const position of [-1, 1, 3, source.length + 1, 0.5])
    assert.throws(() => prepared.queryAt(position, false), RangeError);
});
