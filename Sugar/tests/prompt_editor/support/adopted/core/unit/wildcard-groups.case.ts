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
/** Independent group and exact-source mutation contracts retain implicit display and delimiter integrity. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  parsePrompt,
  prepareWildcardGroups,
  mutateWildcardTag,
  isNumericWildcardTag,
  wildcardBodyRange,
  type WildcardSyntax,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';

test('group defaults share source form and identifier, ignoring CSV columns and tags without changing source', () => {
  const source = '{animal}, {animal|word}, {csv:animal:Color}, {csv:animal:Size}, {other}',
    document = parsePrompt(source),
    groups = prepareWildcardGroups(document.wildcardSpans);
  assert.deepEqual(
    groups.map(({ sourceKey, occurrenceCount, displayTag, explicit, numeric }) => [
      sourceKey,
      occurrenceCount,
      displayTag,
      explicit,
      numeric,
    ]),
    [
      ['simple:animal', 2, '1', false, true],
      ['simple:animal', 2, 'word', true, false],
      ['csv:animal', 2, '1', false, true],
      ['csv:animal', 2, '1', false, true],
      ['simple:other', 1, null, false, false],
    ],
  );
  assert.equal(document.sourceText === source, true);
  assert.equal(
    groups.every((group, i) => group.span === document.wildcardSpans[i]),
    true,
  );
  assert.equal(Object.isFrozen(groups) && groups.every(Object.isFrozen), true);
});

test('numeric controls use strict positive ASCII decimal strings of arbitrary precision', () => {
  for (const tag of [null, '', '0', '01', '-1', '+1', '1.0', '１', '١', 'word', '1\n'])
    assert.equal(isNumericWildcardTag(tag), false);
  for (const tag of ['1', '9', '123456789012345678901234567890'])
    assert.equal(isNumericWildcardTag(tag), true);
  const source = '😀\r\n{animal|999999999999999999999999}, tail\r',
    document = parsePrompt(source),
    outer = document.wildcardSpans[0]!.outerRange,
    result = mutateWildcardTag(document, outer, {
      kind: 'step',
      direction: 'increase',
    });
  assert.equal(result?.text === '😀\r\n{animal|1000000000000000000000000}, tail\r', true);
  assert.equal(result?.selectionRange.from, result?.text.indexOf('}'));
});

test('an explicit step persists display-only group 1, clamps at 1 and refuses nonnumeric or absent groups', () => {
  const document = parsePrompt('{animal}, {animal}, {word|variant}, {alone}'),
    spans = document.wildcardSpans;
  for (const direction of ['increase', 'decrease'] as const) {
    const result = mutateWildcardTag(document, spans[0]!.outerRange, {
      kind: 'step',
      direction,
    });
    assert.equal(
      result?.text ===
        `{animal|${direction === 'increase' ? '2' : '1'}}, {animal}, {word|variant}, {alone}`,
      true,
    );
  }
  for (const span of spans.slice(2))
    assert.equal(
      mutateWildcardTag(document, span.outerRange, {
        kind: 'step',
        direction: 'increase',
      }),
      null,
    );
  assert.equal(mutateWildcardTag(document, { from: 0, to: 1 }, { kind: 'set', tag: 'x' }), null);
});

test('tag editing preserves each wrapper and surrounding exact source, rejecting tags that terminate their own placeholder', () => {
  for (const [placeholder, profile, bad] of [
    ['{folder/animal|2}', { style: 'curly' }, 'x}tail'],
    ['__folder/animal|2__', { style: 'double-underscore' }, 'x__tail'],
    [
      '%%csv:animal:Color|2!!',
      {
        style: 'custom',
        prefix: '%%',
        suffix: '!!',
        alsoRecognizeCurly: false,
      },
      'x!!tail',
    ],
  ] as const satisfies readonly (readonly [string, WildcardSyntax, string])[]) {
    const source = `😀\r\n${placeholder}, tail\r`,
      document = parsePrompt(source, profile),
      span = document.wildcardSpans[0]!;
    for (const tag of ['', ' x', 'x\u0085', bad])
      assert.equal(mutateWildcardTag(document, span.outerRange, { kind: 'set', tag }), null);
    const result = mutateWildcardTag(document, span.outerRange, {
      kind: 'set',
      tag: 'variant|detail',
    });
    assert.equal(result?.text === source.replace('|2', '|variant|detail'), true);
    assert.equal(parsePrompt(result!.text, profile).wildcardSpans[0]!.tag, 'variant|detail');
  }
});

test('invalid exact-source ranges refuse scalar and CRLF splits before semantic targeting', () => {
  const document = parsePrompt('😀\r\n{animal}');
  for (const range of [
    { from: 1, to: 2 },
    { from: 3, to: 4 },
    { from: -1, to: 0 },
    { from: 8, to: 4 },
  ])
    assert.throws(() => mutateWildcardTag(document, range, { kind: 'set', tag: '2' }), RangeError);
});

test('editable body ranges retain identifier/CSV spelling while excluding activators and group suffixes', () => {
  const source = '😀\r\n{csv:folder/animal:Color|variant}, {animal}, {animal|2}',
    document = parsePrompt(source);
  assert.deepEqual(
    document.wildcardSpans.map((span) => {
      const range = wildcardBodyRange(span);
      return source.slice(range.from, range.to);
    }),
    ['folder/animal:Color', 'animal', 'animal'],
  );
});
