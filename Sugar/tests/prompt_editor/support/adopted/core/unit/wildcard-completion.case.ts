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
/** Independent curly completion contracts preserve physical lines, query provenance and literal candidate grammar. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  prepareWildcardQueries,
  wildcardReplacement,
  isWildcardCompletionBody,
  wildcardGhostText,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';

test('prepared curly queries consume a blank right suffix and closer, retain other source and suppress selections', () => {
  const source = '🙂\r\n{ani\t } tail',
    queries = prepareWildcardQueries(source),
    query = queries.queryAt(8, false)!;
  assert.equal(query.prefix, 'ani');
  assert.deepEqual(query.contentRange, { from: 5, to: 8 });
  assert.deepEqual(query.replacementRange, { from: 4, to: 11 });
  assert.deepEqual(wildcardReplacement(source, query, 'animals', 'simple'), {
    range: { from: 4, to: 11 },
    text: '{animals}',
  });
  assert.equal(queries.queryAt(8, true), null);
  assert.equal(prepareWildcardQueries('{anrest}').queryAt(3, false), null);
  assert.equal(prepareWildcardQueries('{an\r}').queryAt(3, false)?.replacementRange.to, 3);
  assert.equal(prepareWildcardQueries('{an\n}').queryAt(3, false)?.replacementRange.to, 3);
  assert.equal(prepareWildcardQueries('{an\rnext').queryAt(8, false), null);
  assert.equal(prepareWildcardQueries('{animals}').queryAt(9, false), null);
  assert.equal(
    prepareWildcardQueries('{csv:characters:Na').queryAt(18, false)?.prefix,
    'csv:characters:Na',
  );
});

test('invalid editable boundaries and stale or copied query descriptors reject explicitly', () => {
  const source = '🙂\r\n{',
    queries = prepareWildcardQueries(source),
    query = queries.queryAt(source.length, false)!;
  for (const position of [-1, 1, 3, 1.5, 7])
    assert.throws(() => queries.queryAt(position, false), RangeError);
  assert.throws(() => wildcardReplacement(source, { ...query }, 'animals', 'simple'), TypeError);
  assert.throws(() => wildcardReplacement(source + 'a', query, 'animals', 'simple'), TypeError);
  assert.equal(Object.isFrozen(query), true);
  assert.equal(Object.isFrozen(query.replacementRange), true);
});

test('sources without openers stay inactive and a late first opener retains physical-line boundaries', () => {
  const inactive = '🙂\r\n(plain:1.20), tail';
  const empty = prepareWildcardQueries(inactive);
  for (const position of [0, 2, 4, inactive.length])
    assert.equal(empty.queryAt(position, false), null);
  assert.throws(() => empty.queryAt(3, false), RangeError);
  const source = inactive + '\r\n{an }\rnext';
  const queries = prepareWildcardQueries(source);
  const caret = source.indexOf('{an') + 3;
  assert.deepEqual(queries.queryAt(caret, false)?.replacementRange, {
    from: caret - 3,
    to: caret + 2,
  });
  assert.equal(queries.queryAt(source.length, false), null);
});

test('catalog acceptance preserves one untagged placeholder of the declared source kind', () => {
  const query = prepareWildcardQueries('{').queryAt(1, false)!;
  for (const body of [
    '',
    'animals} tail',
    '{animals',
    'animals|variant',
    'csv:characters:Name|variant',
    '../animals',
    'animals\\nested',
    'csv:characters:Name:Other',
  ]) {
    assert.equal(isWildcardCompletionBody(body, 'simple'), false);
    assert.equal(isWildcardCompletionBody(body, 'csv'), false);
    assert.throws(() => wildcardReplacement('{', query, body, 'simple'), RangeError);
  }
  assert.equal(isWildcardCompletionBody('csv:characters:Name', 'simple'), false);
  assert.equal(isWildcardCompletionBody('animals', 'csv'), false);
  assert.equal(isWildcardCompletionBody('nested/animals', 'simple'), true);
  assert.equal(isWildcardCompletionBody('csv:characters:Full Name', 'csv'), true);
  assert.equal(
    wildcardReplacement('{', query, 'csv:characters:Full Name', 'csv').text,
    '{csv:characters:Full Name}',
  );
});

test('ghost previews preserve original scalar boundaries and omit an already authored closer', () => {
  for (const [source, caret, body, suffix] of [
    ['{strass', 7, 'Straße', 'e}'],
    ['{stras', 6, 'Straße', ''],
    ['{i\u0307', 3, 'İdea', 'dea}'],
    ['{𐐨', 3, '𐐀name', 'name}'],
    ['{animals}', 8, 'animals', ''],
    ['{an }', 3, 'animals', 'imals'],
    ['{ani', 4, 'nested/animals', ''],
  ] as const) {
    const query = prepareWildcardQueries(source).queryAt(caret, false)!;
    assert.equal(wildcardGhostText(source, query, body), suffix);
  }
  const query = prepareWildcardQueries('{').queryAt(1, false)!;
  assert.throws(() => wildcardGhostText('{', { ...query }, 'animals'), TypeError);
});
