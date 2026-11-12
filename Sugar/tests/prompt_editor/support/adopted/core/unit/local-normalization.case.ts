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
/** Independent local ingestion, caret and original-coordinate correction contracts. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  normalizePasteRange,
  normalizeTypedEdit,
  normalizeTypedDeletion,
  prepareTagLexicon,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';

test('typed closing shells normalize their complete known-tag segment and delay unmatched ancestors', () => {
  const lexicon = prepareTagLexicon(['vertin (reverse:1999)']);
  const known = '(untouched), vertin (reverse:1999)';
  assert.equal(
    normalizeTypedEdit(
      known,
      { from: known.length - 1, to: known.length, replacementText: ')' },
      { lexicon },
    ).text,
    String.raw`(untouched), vertin \(reverse:1999\)`,
  );
  assert.equal(
    normalizeTypedEdit('((cat)', { from: 5, to: 6, replacementText: ')' }).text,
    '((cat)',
  );
  assert.equal(
    normalizeTypedEdit('((cat))', { from: 6, to: 7, replacementText: ')' }).text,
    '(cat:1.21)',
  );
  assert.equal(normalizeTypedEdit('cat)', { from: 3, to: 4, replacementText: ')' }).text, 'cat)');
  assert.equal(
    normalizeTypedEdit('(cat)', { from: 0, to: 5, replacementText: '(cat)' }).text,
    '(cat:1.10)',
  );
  assert.equal(
    normalizeTypedEdit('(cat:1.234)', {
      from: 10,
      to: 11,
      replacementText: ')',
    }).text,
    '(cat:1.234)',
  );
});

test('committed multi-character ranges normalize completed shells while retaining surrounding spelling and unfinished ancestors', () => {
  const source = '(outside:.333), ((猫)) (dog:1.234), tail';
  const entered = '((猫)) (dog:1.234)',
    from = source.indexOf(entered);
  const result = normalizeTypedEdit(source, {
    from,
    to: from + entered.length,
    replacementText: entered,
  });
  assert.equal(result.text, '(outside:.333), (猫:1.21) (dog:1.234), tail');
  assert.deepEqual(result.transitions, [
    {
      from,
      to: from + entered.length,
      kind: 'implicit_emphasis',
      nestingDepth: 2,
    },
  ]);
  assert.deepEqual(result.generatedEmphases, [{ from, to: from + 8, nestingDepth: 2 }]);
  const mixed = '[SEP]((cat))';
  const combined = normalizeTypedEdit(mixed, {
    from: 0,
    to: mixed.length,
    replacementText: mixed,
  });
  assert.equal(combined.text === '[SEP]\n(cat:1.21)', true);
  assert.deepEqual(combined.transitions, [
    { from: 0, to: mixed.length, kind: 'implicit_emphasis', nestingDepth: 2 },
  ]);
  assert.equal(combined.boundaryPositions.at(-1), combined.text.length);
  const existingEnding = normalizeTypedEdit('[SEP]\r\n', {
    from: 0,
    to: 5,
    replacementText: '[SEP]',
  });
  assert.equal(existingEnding.text === '[SEP]\r\n', true);
  assert.equal(existingEnding.boundaryPositions[5], 7);
  const completing = normalizeTypedEdit('before\r\nx[SEP]', {
    from: 11,
    to: 14,
    replacementText: 'EP]',
  });
  assert.equal(completing.text === 'before\r\nx\r\n[SEP]\r\n', true);
  assert.equal(completing.boundaryPositions[14], 18);
  assert.equal(
    normalizeTypedEdit('(long)', { from: 1, to: 5, replacementText: 'long' }).text === '(long)',
    true,
  );
  assert.equal(result.boundaryPositions.at(-1), result.text.length);
  for (const raw of ['((cat)', '"((cat))"', '<lora:(cat):.8>'])
    assert.equal(
      normalizeTypedEdit(raw, { from: 0, to: raw.length, replacementText: raw }).text,
      raw,
    );
  const lexicon = prepareTagLexicon(['vertin (reverse:1999)']);
  const known = 'vertin (reverse:1999)';
  assert.equal(
    normalizeTypedEdit(known, { from: 0, to: known.length, replacementText: known }, { lexicon })
      .text,
    String.raw`vertin \(reverse:1999\)`,
  );
});

test('numeric reclassification removes exactly outer escapes and maps every removed boundary', () => {
  const committed = normalizeTypedEdit(String.raw`\(cat:1.23\)`, {
    from: 8,
    to: 10,
    replacementText: '23',
  });
  assert.equal(committed.text === '(cat:1.23)', true);
  const source = String.raw`\(wide shot:6\)`,
    result = normalizeTypedEdit(source, {
      from: 12,
      to: 13,
      replacementText: '6',
    });
  assert.equal(result.text, '(wide shot:6)');
  assert.deepEqual(result.boundaryPositions, [0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 12, 13]);
  assert.deepEqual(result.transitions, [
    { from: 0, to: 15, kind: 'escaped_literal_to_emphasis', nestingDepth: 1 },
  ]);
  assert.equal(normalizeTypedEdit(source, { from: 12, to: 13, replacementText: '-' }).text, source);
});

test('generated shells preserve authored depth during local outer completion', () => {
  const source = '((cat:1.21))';
  const result = normalizeTypedEdit(
    source,
    { from: 11, to: 12, replacementText: ')' },
    { generatedEmphases: [{ from: 1, to: 11, nestingDepth: 2 }] },
  );
  assert.equal(result.text, '(cat:1.331)');
  assert.equal(result.transitions[0]?.nestingDepth, 3);
});

test('paste canonicalizes its own range while preserving outside spelling and line endings', () => {
  const source = '(left), (cat), <lora:model:.8>\r\n(right)',
    from = source.indexOf('(cat)'),
    to = source.indexOf('\r\n');
  const result = normalizePasteRange(source, { from, to });
  assert.equal(result.text, '(left), (cat:1.10), <lora:model:0.80>\r\n(right)');
  assert.equal(result.boundaryPositions[from], from);
  assert.equal(result.boundaryPositions.at(-1), result.text.length);
  assert.throws(() => normalizePasteRange(source, { from: -1, to: 2 }), RangeError);
  assert.throws(() => normalizePasteRange('a\r\nb', { from: 2, to: 4 }), RangeError);
});

test('typed markers receive minimal line placement and place the caret after following exact endings', () => {
  for (const ending of ['\n', '\r', '\r\n']) {
    const source = 'before' + ending + 'x[SEP]' + ending + 'next',
      end = source.indexOf(']') + 1;
    const result = normalizeTypedEdit(source, {
      from: end - 1,
      to: end,
      replacementText: ']',
    });
    assert.equal(result.text, 'before' + ending + 'x' + ending + '[SEP]' + ending + 'next');
    assert.equal(result.boundaryPositions[end], result.text.indexOf('next'));
  }
  assert.equal(
    normalizeTypedEdit('[SEP]', { from: 4, to: 5, replacementText: ']' }).text,
    '[SEP]\n',
  );
  assert.equal(
    normalizeTypedEdit(String.raw`\[SEP]`, {
      from: 5,
      to: 6,
      replacementText: ']',
    }).text,
    String.raw`\[SEP]`,
  );
});

test('pasted adjacent markers share one separator newline and do not force a trailing EOF line', () => {
  const source = 'x[SEP][SEP|Front]y',
    result = normalizePasteRange(source, { from: 0, to: source.length });
  assert.equal(result.text, 'x\n[SEP]\n[SEP|Front]\ny');
  assert.equal(normalizePasteRange('[SEP]', { from: 0, to: 5 }).text, '[SEP]');
});

test("empty partition insertion preserves the preceding separator's exact newline and caret affinity", () => {
  for (const ending of ['\n', '\r', '\r\n']) {
    const from = 5 + ending.length,
      source = '[SEP]' + ending + 'cat[SEP]';
    const result = normalizePasteRange(source, { from, to: from + 3 });
    assert.equal(result.text, '[SEP]' + ending + 'cat' + ending + '[SEP]');
    assert.equal(result.boundaryPositions[from + 3], from + 3);
  }
});

test('empty-region detection requires the actual newline rather than unrelated two-character context', () => {
  const source = '[SEP]\nab[SEP]';
  assert.equal(normalizeTypedEdit(source, { from: 7, to: 8, replacementText: 'b' }).text, source);
  assert.deepEqual(
    normalizeTypedEdit(source, { from: 7, to: 8, replacementText: 'b' }).boundaryPositions,
    Array.from({ length: source.length + 1 }, (_, position) => position),
  );
});

test('paste transition provenance stays inside original source after separator insertion', () => {
  const source = '[SEP](cat)',
    result = normalizePasteRange(source, { from: 0, to: source.length });
  assert.equal(result.text, '[SEP]\n(cat:1.10)');
  assert.deepEqual(result.transitions, [
    { from: 0, to: 10, kind: 'implicit_emphasis', nestingDepth: 1 },
  ]);
  for (const transition of result.transitions)
    assert.ok(transition.from >= 0 && transition.to <= source.length);
});

test('separator deletion repair stays local and retains caret before a restored marker boundary', () => {
  const result = normalizeTypedDeletion('a[SEP]\r\nnext\nuntouched[SEP]tail', 1);
  assert.equal(result.text, 'a\r\n[SEP]\r\nnext\nuntouched[SEP]tail');
  assert.equal(result.boundaryPositions[1], 1);
  assert.equal(normalizeTypedDeletion('[SEP]x', 5).text, '[SEP]\nx');
  const multiple = normalizeTypedDeletion('A[SEP]B[SEP]C', 7);
  assert.equal(multiple.text, 'A\n[SEP]\nB\n[SEP]\nC');
  assert.deepEqual(multiple.boundaryPositions, [0, 2, 3, 4, 5, 6, 8, 9, 11, 12, 13, 14, 16, 17]);
  assert.ok(
    multiple.boundaryPositions.every(
      (position, index, positions) => index === 0 || position >= positions[index - 1]!,
    ),
  );
  const adjacent = normalizeTypedDeletion('A[SEP][SEP]C', 6);
  assert.equal(adjacent.boundaryPositions[6], 7);
});
