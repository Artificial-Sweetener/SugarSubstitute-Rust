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
/** Independent source specimens prove fixed-step mutations, exact rounding and ownership of untouched bytes. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { parsePrompt } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  mutateEmphasisWeight,
  mutateLoraWeight,
  wrapPlainEmphasisWeight,
} from '../../../../../../frontend/sugarprompt/core/src/prompt/weight-mutations.ts';

test('plain emphasis steps wrap the exact selection and existing neutral shells unwrap without altering surroundings', () => {
  const source = '🙂\r\ncat\rnext\n';
  for (const [direction, expected] of [
    ['increase', '1.05'],
    ['decrease', '0.95'],
  ] as const) {
    const wrapped = mutateEmphasisWeight(
      parsePrompt(source),
      { from: 4, to: 7 },
      { kind: 'step', direction },
    );
    assert.equal(wrapped.text, `🙂\r\n(cat:${expected})\rnext\n`);
    assert.deepEqual(wrapped.selectionRange, { from: 5, to: 8 });
    const unwrapped = mutateEmphasisWeight(parsePrompt(wrapped.text), wrapped.selectionRange, {
      kind: 'step',
      direction: direction === 'increase' ? 'decrease' : 'increase',
    });
    assert.equal(unwrapped.text, source);
    assert.deepEqual(unwrapped.selectionRange, { from: 4, to: 7 });
    assert.equal(Object.isFrozen(wrapped) && Object.isFrozen(wrapped.selectionRange), true);
  }
});

test('exact shell targeting changes only the intended nested shell and signed steps cross zero', () => {
  const soleChild = parsePrompt('((cat:1.20):1.10)');
  assert.equal(
    mutateEmphasisWeight(soleChild, soleChild.emphasisSpans[1]!.outerRange, {
      kind: 'step',
      direction: 'increase',
    }).text,
    '((cat:1.25):1.10)',
  );
  const document = parsePrompt('((cat:1.20) dog:1.10), tail');
  const inner = document.emphasisSpans[1]!;
  for (const range of [inner.contentRange, inner.outerRange]) {
    const updated = mutateEmphasisWeight(document, range, {
      kind: 'step',
      direction: 'increase',
    });
    assert.equal(updated.text, '((cat:1.25) dog:1.10), tail');
    assert.deepEqual(updated.selectionRange, inner.contentRange);
  }
  for (const [weight, direction, expected] of [
    ['0.00', 'decrease', '-0.05'],
    ['-0.05', 'increase', '0.00'],
    ['1.005', 'increase', '1.06'],
    ['1.015', 'increase', '1.06'],
    ['1.0000001', 'decrease', '0.95'],
    ['-.0500001', 'increase', '-0.00'],
  ] as const) {
    const input = parsePrompt(`(cat:${weight})`);
    assert.equal(
      mutateEmphasisWeight(input, input.emphasisSpans[0]!.contentRange, {
        kind: 'step',
        direction,
      }).text,
      `(cat:${expected})`,
    );
  }
});

test('exact numeric entry uses half-even rounding, keeps neutral plain source, and does not coerce malformed numbers', () => {
  const document = parsePrompt('cat');
  for (const [weight, expected] of [
    ['1', 'cat'],
    ['1.005', 'cat'],
    ['1.015', '(cat:1.02)'],
    ['-.005', '(cat:-0.00)'],
    ['+１２.３', '(cat:12.30)'],
  ]) {
    assert.equal(
      mutateEmphasisWeight(document, { from: 0, to: 3 }, { kind: 'set', weight: weight! }).text,
      expected,
    );
  }
  for (const weight of ['', 'NaN', 'Infinity', '1e2', ' 1', '1 ', '--1'])
    assert.throws(
      () => mutateEmphasisWeight(document, { from: 0, to: 3 }, { kind: 'set', weight }),
      TypeError,
    );
  assert.deepEqual(
    mutateEmphasisWeight(document, { from: 1, to: 1 }, { kind: 'step', direction: 'increase' }),
    { text: 'cat', selectionRange: { from: 1, to: 1 }, neutral: false },
  );
});

test('LoRA first-weight edits retain spacing, second weight, block weights, model name and all neighboring bytes', () => {
  const document = parsePrompt('🙂\r\n<lora:folder/model: +.80: -.665:block>, tail');
  const span = document.loraSpans[0]!;
  const updated = mutateLoraWeight(document, span.outerRange, {
    kind: 'step',
    direction: 'decrease',
  })!;
  assert.equal(updated.text, '🙂\r\n<lora:folder/model: 0.75: -.665:block>, tail');
  assert.deepEqual(updated.selectionRange, {
    from: span.firstWeightRange.from,
    to: span.firstWeightRange.from + 4,
  });
  assert.equal(mutateLoraWeight(document, { from: 4, to: 5 }, { kind: 'set', weight: '2' }), null);
  assert.equal(
    mutateLoraWeight(document, span.outerRange, {
      kind: 'set',
      weight: '1.00',
    })!.text,
    '🙂\r\n<lora:folder/model: 1.00: -.665:block>, tail',
  );
});

test('large valid decimals have exact arithmetic independent of the donor Decimal context', () => {
  const weight = '1234567890123456789012345678901234567890.12';
  const document = parsePrompt(`(cat:${weight})`);
  assert.equal(
    mutateEmphasisWeight(document, document.emphasisSpans[0]!.contentRange, {
      kind: 'step',
      direction: 'increase',
    }).text,
    '(cat:1234567890123456789012345678901234567890.17)',
  );
});

test('a neutral inner layer retains its identity even when its content coincides with an enclosing shell', () => {
  const document = parsePrompt('((cat:1.05):1.10)');
  const neutral = mutateEmphasisWeight(document, document.emphasisSpans[1]!.contentRange, {
    kind: 'step',
    direction: 'decrease',
  });
  assert.equal(neutral.text, '(cat:1.10)');
  assert.equal(neutral.neutral, true);
  const restored = wrapPlainEmphasisWeight(neutral.text, neutral.selectionRange, {
    kind: 'step',
    direction: 'decrease',
  });
  assert.equal(restored.text, '((cat:0.95):1.10)');
  assert.equal(restored.neutral, false);
});

test('weight source boundaries reject inverted, noninteger, out-of-source, scalar and CRLF splits', () => {
  const document = parsePrompt('🙂\r\ncat');
  for (const range of [
    { from: 1, to: 4 },
    { from: 0, to: 1 },
    { from: 3, to: 4 },
    { from: 4, to: 3 },
    { from: -1, to: 3 },
    { from: 0, to: 8 },
    { from: 0.5, to: 4 },
  ]) {
    assert.throws(
      () =>
        mutateEmphasisWeight(document, range, {
          kind: 'step',
          direction: 'increase',
        }),
      RangeError,
    );
    assert.throws(
      () =>
        mutateLoraWeight(document, range, {
          kind: 'step',
          direction: 'increase',
        }),
      RangeError,
    );
  }
});
