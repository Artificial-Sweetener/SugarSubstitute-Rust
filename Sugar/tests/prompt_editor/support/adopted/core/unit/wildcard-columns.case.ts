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
/** Independent canonical display contracts preserve exact authored source and Unicode scalar groups. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  parsePrompt,
  prepareWildcardColumnAliases,
  type WildcardSpan,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';

/** A canonical specimen places column bounds after an astral scalar and exact CRLF source. */
function reference(column: string): WildcardSpan {
  return parsePrompt(`🙂\r\n{csv:characters:${column}|2}`).wildcardSpans[0]!;
}

test('ordinary casing replaces only changed source scalars and retains native letter boundaries', () => {
  const span = reference('name'),
    from = span.contentRange.from + 5 + span.identifier.length,
    aliases = prepareWildcardColumnAliases(span, 'NaMe');
  assert.deepEqual(aliases, [
    { range: { from, to: from + 1 }, text: 'N' },
    { range: { from: from + 2, to: from + 3 }, text: 'M' },
  ]);
  assert.equal(span.csvColumn, 'name');
  assert.equal(Object.isFrozen(aliases), true);
  assert.equal(Object.isFrozen(aliases[0]!.range), true);
});

test('expanded lower identities, titlecase, contextual sigma and astral letters retain complete literal glyphs', () => {
  for (const [authored, canonical, expected] of [
    ['i\u0307dea', 'İdea', [[0, 2, 'İ']]],
    ['İdea', 'i\u0307dea', [[0, 1, 'i\u0307']]],
    [
      'ǆfield',
      'ǅField',
      [
        [0, 1, 'ǅ'],
        [1, 2, 'F'],
      ],
    ],
    ['ΟΣ', 'Ος', [[1, 2, 'ς']]],
    [
      '𐐨name',
      '𐐀Name',
      [
        [0, 2, '𐐀'],
        [2, 3, 'N'],
      ],
    ],
  ] as const) {
    const span = reference(authored),
      from = span.contentRange.from + 5 + span.identifier.length;
    assert.deepEqual(
      prepareWildcardColumnAliases(span, canonical),
      expected.map(([start, end, text]) => ({
        range: { from: from + start, to: from + end },
        text,
      })),
    );
  }
});

test('missing, identical and simple references publish no aliases; unrelated and merely case-fold-equivalent metadata rejects', () => {
  assert.deepEqual(prepareWildcardColumnAliases(reference('Color'), 'Color'), []);
  assert.deepEqual(prepareWildcardColumnAliases(reference('Color'), null), []);
  assert.deepEqual(
    prepareWildcardColumnAliases(parsePrompt('{animals}').wildcardSpans[0]!, 'Name'),
    [],
  );
  for (const [authored, canonical] of [
    ['Color', 'Name'],
    ['Straße', 'STRASSE'],
    ['οσ', 'ΟΣ'],
  ])
    assert.throws(() => prepareWildcardColumnAliases(reference(authored!), canonical!), RangeError);
});
