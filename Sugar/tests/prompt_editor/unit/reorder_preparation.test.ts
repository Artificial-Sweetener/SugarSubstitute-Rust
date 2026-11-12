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
/** Safeguard exact chip derivation and bound reads of canonical parsed span rosters. */
import {
  prepareDocumentPromptScopes,
  prepareDocumentValueReorders,
  serializeValueReorder,
  type PreparedDocumentPromptScopes,
} from '../../../frontend/sugarprompt/core/src/index.js';
import { prepareDocumentValues } from '../../../frontend/sugarprompt/core/src/document/values.js';
import { ReorderDerivationSpans } from '../../../frontend/sugarprompt/core/src/reorder/derivation-spans.js';

/** Prepare through the actual canonical parser; observation wraps only its immutable span arrays. */
function specimen(source: string) {
  const scopes = prepareDocumentPromptScopes(prepareDocumentValues(source, 'prompt'), {
    style: 'curly',
  });
  const scope = scopes.scopes[0];
  if (!scope) throw new Error('Missing canonical prompt scope');
  const reads = { emphasis: 0, lora: 0 };
  /** Count source-roster reads without implementing syntax, lookups, or chip policy in the test. */
  function observe<T>(items: readonly T[], kind: keyof typeof reads): readonly T[] {
    return new Proxy(items, {
      get(target, key, receiver) {
        if (typeof key === 'string' && /^\d+$/u.test(key)) reads[kind]++;
        return Reflect.get(target, key, receiver);
      },
    });
  }
  const observed = {
    ...scope,
    document: {
      ...scope.document,
      emphasisSpans: observe(scope.document.emphasisSpans, 'emphasis'),
      loraSpans: observe(scope.document.loraSpans, 'lora'),
    },
  };
  const input: PreparedDocumentPromptScopes = {
    ...scopes,
    scopes: [observed],
    byId: { [observed.id]: observed },
  };
  return { input, reads, original: scope.document };
}

test.each([
  '  prefix, ((red, blue:1.3), <lora:inside:0.5>:1.20), tail  ',
  'alpha\r\n\r\n<lora:first:1> text <lora:second:0.8>, (one,two:1.2),\r\n[SEP]\r\n omega, ',
  'quoted "a,b", {colors}, (nested (single:1.1):1.2), (ends,with,comma,:1.3), final',
])('preserve exact source, carrier ranges and original envelope identity: %s', (source) => {
  const f = specimen(source);
  const prepared = prepareDocumentValueReorders(f.input)?.[0];
  if (!prepared) throw new Error('Missing reorder preparation');
  const output = serializeValueReorder(prepared);
  expect(output?.text).toBe(source);
  for (const chip of prepared.chips) {
    expect(chip.text).toBe(source.slice(chip.contentRange.from, chip.contentRange.to));
    for (const envelope of chip.envelopes)
      expect(f.original.emphasisSpans.includes(envelope.span)).toBe(true);
  }
  expect(Object.isFrozen(prepared.chips)).toBe(true);
});

test('prepare many decorated segments with bounded canonical span-roster reads', () => {
  const count = 256;
  const source = Array.from(
    { length: count },
    (_, index) => `plain, (eyes${String(index)}:1.2), <lora:model${String(index)}:0.8>, tail`,
  ).join(', ');
  const f = specimen(source);
  const prepared = prepareDocumentValueReorders(f.input)?.[0];
  if (!prepared) throw new Error('Missing reorder preparation');
  expect(prepared.chips).toHaveLength(count * 4);
  expect(f.reads.emphasis).toBeLessThanOrEqual(count * 8);
  expect(f.reads.lora).toBeLessThanOrEqual(count * 8);
  expect(serializeValueReorder(prepared)?.text).toBe(source);
});

test('indexed lookups preserve exact identities, original order and contained-range boundaries', () => {
  const { original } = specimen('(one,two:1.2), <lora:first:1>, <lora:second:0.8>');
  const shell = original.emphasisSpans[0],
    first = original.loraSpans[0],
    second = original.loraSpans[1];
  if (!shell || !first || !second) throw new Error('Missing canonical span fixture');
  const spans = new ReorderDerivationSpans({
    ...original,
    emphasisSpans: [shell, { ...shell, weight: '9' }],
    loraSpans: [second, first],
  });
  expect(spans.emphasis(shell.outerRange)).toBe(shell);
  expect(
    spans.emphasis({ from: shell.outerRange.from, to: shell.outerRange.to - 1 }),
  ).toBeUndefined();
  const whole = { from: 0, to: original.sourceText.length };
  expect(spans.loras(whole)).toEqual([second, first]);
  expect(spans.loras(first.outerRange)).toEqual([first]);
  expect(spans.hasLora(first.outerRange)).toBe(true);
  expect(spans.hasLora({ from: first.outerRange.from, to: first.outerRange.to - 1 })).toBe(false);
  expect(spans.hasLora({ from: first.outerRange.from + 1, to: first.outerRange.to })).toBe(false);
  expect(spans.loras(shell.outerRange)).toEqual([]);
});

test('nested envelope validation reads the canonical roster once and rejects reconstructed spans', () => {
  const count = 256;
  const source = Array.from({ length: count }, () => '((one,two:1.2),three:1.3),tail').join(', ');
  const f = specimen(source);
  const prepared = prepareDocumentValueReorders(f.input)?.[0];
  if (!prepared) throw new Error('Missing reorder preparation');
  f.reads.emphasis = 0;
  expect(serializeValueReorder(prepared)?.text).toBe(source);
  expect(f.reads.emphasis).toBeLessThanOrEqual(count * 4);
  const chips = prepared.chips.map((chip, index) =>
    index === 0
      ? {
          ...chip,
          envelopes: chip.envelopes.map((envelope) => ({
            ...envelope,
            span: { ...envelope.span },
          })),
        }
      : chip,
  );
  expect(serializeValueReorder({ ...prepared, chips })).toBeNull();
});
