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
/** Independent normalization contracts, including intended corrections to donor decimal limits. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  canonicalizeParentheses,
  isExplicitEmphasis,
  normalizeForStorage,
  normalizeWeights,
  prepareTagLexicon,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';

test('numeric formatting preserves signed zero and rounds exact ties to the even hundredth', () => {
  for (const [weight, expected] of [
    ['1', '1.00'],
    ['.5', '0.50'],
    ['1.234', '1.23'],
    ['1.235', '1.24'],
    ['1.245', '1.24'],
    ['1.2451', '1.25'],
    ['9.999', '10.00'],
    ['-.005', '-0.00'],
    ['-.015', '-0.02'],
    ['.0000001', '0.00'],
    ['１２.３', '12.30'],
    ['١.٢', '1.20'],
    ['𝟙𝟚.𝟛', '12.30'],
    ['000001.0000', '1.00'],
  ])
    assert.equal(normalizeWeights(`(x:${weight})`).text, `(x:${expected})`);
  assert.equal(
    normalizeWeights('<lora:path/model: +.8: -.665:block>, (x:1e2), (x: 1), (x:1 )').text,
    '<lora:path/model: 0.80: -0.66:block>, (x:1e2), (x: 1), (x:1 )',
  );
});

test('generated provenance identifies actual output shells independently of whole-segment transitions', () => {
  const result = canonicalizeParentheses('🙂\r\n(cat) ((dog)), ( (bird) )');
  assert.equal(result.text, '🙂\r\n(cat:1.10) (dog:1.21), ( (bird:1.10) :1.10)');
  assert.deepEqual(result.generatedEmphases, [
    { from: 4, to: 14, nestingDepth: 1 },
    { from: 15, to: 25, nestingDepth: 2 },
    { from: 27, to: 47, nestingDepth: 1 },
    { from: 29, to: 40, nestingDepth: 1 },
  ]);
  assert.equal(Object.isFrozen(result.generatedEmphases), true);
  assert.equal(result.generatedEmphases.every(Object.isFrozen), true);
  const retained = canonicalizeParentheses(result.text, {
    generatedEmphases: result.generatedEmphases,
  });
  assert.deepEqual(retained.generatedEmphases, result.generatedEmphases);
  assert.equal(retained.text, result.text);
  const stored = normalizeForStorage('(cat) ((dog)), (authored:.8)');
  assert.deepEqual(stored.generatedEmphases, [
    { from: 0, to: 10, nestingDepth: 1 },
    { from: 11, to: 21, nestingDepth: 2 },
  ]);
  assert.deepEqual(canonicalizeParentheses('(cat)', { forceLiteral: true }).generatedEmphases, []);
});

test('valid large weights normalize without a fixed precision context', () => {
  const huge = '123456789012345678901234567890';
  assert.equal(normalizeWeights(`(x:${huge})`).text, `(x:${huge}.00)`);
  assert.equal(
    normalizeWeights(`<lora:model:${huge}:${huge}.995>`).text,
    `<lora:model:${huge}.00:123456789012345678901234567891.00>`,
  );
  const manyNines = '9'.repeat(1000);
  assert.equal(normalizeWeights(`(x:${manyNines}.995)`).text, `(x:1${'0'.repeat(1000)}.00)`);
});

test('weight boundary mappings retain source before/after replacements and clamp numeric interiors', () => {
  const result = normalizeWeights('🙂(x:01.000)\r\nend');
  assert.equal(result.text, '🙂(x:1.00)\r\nend');
  assert.deepEqual(
    result.boundaryPositions,
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 9, 9, 10, 11, 12, 13, 14, 15],
  );
  assert.equal(Object.isFrozen(result), true);
  assert.equal(Object.isFrozen(result.boundaryPositions), true);
});

test('prepared exact membership escapes balanced literal parentheses and leaves surrounding source exact', () => {
  const seen: string[] = [],
    tags = new Set(['vertin (reverse:1999)', 'literal (a (b))']);
  const lexicon = prepareTagLexicon([...tags]);
  const result = canonicalizeParentheses('\tvertin (reverse:1999) , (cat), literal (a (b))\r\n', {
    lexicon: {
      contains(text) {
        seen.push(text);
        return lexicon.contains(text);
      },
    },
  });
  assert.equal(
    result.text,
    String.raw`\tvertin \(reverse:1999\) , (cat:1.10), literal \(a \(b\)\)`.replace('\\t', '\t') +
      '\r\n',
  );
  assert.deepEqual(seen, ['vertin (reverse:1999)', '(cat)', 'literal (a (b))\r\n']);
  assert.deepEqual(
    result.transitions.map((value) => value.kind),
    ['known_literal_tag', 'implicit_emphasis', 'known_literal_tag'],
  );
  assert.equal(Object.isFrozen(result.transitions[0]), true);
});

test('prepared tag identities fold case, underscores and Python whitespace without retaining mutable input', () => {
  const tags = ['Straße_(tag)', 'VERTIN (reverse:1999)', ''];
  const lexicon = prepareTagLexicon(tags);
  tags.length = 0;
  assert.equal(lexicon.contains('STRASSE\u0085(tag)\r\n'), true);
  assert.equal(lexicon.contains('vertin_(reverse:1999)'), true);
  assert.equal(lexicon.contains('\ufeffvertin (reverse:1999)'), false);
  assert.equal(lexicon.contains('strasse (tag) extra'), false);
  assert.equal(lexicon.contains(' \r\n'), false);
  assert.equal(Object.isFrozen(lexicon), true);
});

test('unknown implicit shells collapse only direct nesting and preserve authored explicit spelling', () => {
  for (const [before, after] of [
    ['(cat)', '(cat:1.10)'],
    ['((cat))', '(cat:1.21)'],
    ['(((cat)))', '(cat:1.331)'],
    ['( (cat) )', '( (cat:1.10) :1.10)'],
    ['((cat:.80))', '((cat:.80):1.10)'],
    ['((cat):1.234)', '((cat:1.10):1.234)'],
    ['(broken (cat)', '(broken (cat:1.10)'],
    ['"(cat)", <lora:(model):0.8>', '"(cat)", <lora:(model):0.8>'],
    [String.raw`\(cat\), another's (word)`, String.raw`\(cat\), another's (word:1.10)`],
  ] as const)
    assert.equal(canonicalizeParentheses(before).text, after);
  assert.equal(canonicalizeParentheses('(cat:1.20)').transitions.length, 0);
  assert.equal(canonicalizeParentheses('(cat)', { forceLiteral: true }).text, String.raw`\(cat\)`);
});

test('generated shell identity carries authored depth while manual explicit shells retain their weight', () => {
  const source = '((cat:1.21))';
  const normalized = canonicalizeParentheses(source, {
    generatedEmphases: [{ from: 1, to: 11, nestingDepth: 2 }],
  });
  assert.equal(normalized.text, '(cat:1.331)');
  assert.equal(normalized.transitions[0]?.nestingDepth, 3);
  assert.equal(canonicalizeParentheses(source).text, '((cat:1.21):1.10)');
  assert.throws(
    () =>
      canonicalizeParentheses(source, {
        generatedEmphases: [{ from: 1, to: 11, nestingDepth: 0 }],
      }),
    RangeError,
  );
});

test('deep implicit emphasis retains every digit of the authored multiplier', () => {
  const source = '('.repeat(28) + 'cat' + ')'.repeat(28);
  assert.equal(canonicalizeParentheses(source).text, '(cat:14.4209936106499234037676064081)');
  assert.equal(normalizeForStorage(source).text, '(cat:14.42)');
  const nested = '(prefix '.repeat(1000) + 'cat' + ')'.repeat(1000);
  assert.equal(
    canonicalizeParentheses(nested).text,
    '(prefix '.repeat(1000) + 'cat' + ':1.10)'.repeat(1000),
  );
});

test('canonical boundaries keep content identity and place the completed caret after inserted syntax', () => {
  assert.deepEqual(canonicalizeParentheses('(cat)').boundaryPositions, [0, 1, 2, 3, 9, 10]);
  assert.deepEqual(canonicalizeParentheses('(🙂)').boundaryPositions, [0, 1, 2, 8, 9]);
  assert.deepEqual(
    canonicalizeParentheses('(cat)', { forceLiteral: true }).boundaryPositions,
    [1, 2, 3, 4, 6, 7],
  );
  const source = '((cat)), (x:01.000)\r\n';
  const result = normalizeForStorage(source);
  assert.equal(result.text, '(cat:1.21), (x:1.00)\r\n');
  assert.equal(result.boundaryPositions.at(-1), result.text.length);
  assert.equal(result.transitions[0]?.to, 7);
  assert.deepEqual(normalizeForStorage(result.text).text, result.text);
});

test('explicit recognition requires exactly one complete weighted shell', () => {
  for (const text of ['(cat:1.2)', '((cat):1)', '(:1)'])
    assert.equal(isExplicitEmphasis(text), true);
  for (const text of [
    '(cat)',
    ' (cat:1.2)',
    '(cat:1.2),',
    '(cat:1.2), next',
    String.raw`\(cat:1.2\)`,
    '(cat:1e2)',
  ])
    assert.equal(isExplicitEmphasis(text), false);
});
