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
/** Preserve exact scalar, CRLF and decoded mapping behavior while bounding retained object count. */
import {
  prepareIdentityMapping,
  prepareSourceOffsets,
  prepareValueMapping,
  decodedRangeToSource,
  sourceRangeToDecoded,
} from '../../../frontend/sugarprompt/core/src/document/value-mapping.js';

test.each([
  { source: 'plain prompt', text: 'plain prompt', from: 0 },
  { source: 'prefix prompt suffix', text: 'prompt', from: 7 },
  { source: 'a🧭b\r\nc\rd\n', text: 'a🧭b\r\nc\rd\n', from: 0 },
  { source: 'prefix 🧭\r\ninside suffix', text: '🧭\r\ninside', from: 7 },
  { source: '\ud800x\udc00', text: '\ud800x\udc00', from: 0 },
  { source: 'other', text: '', from: 3 },
  { source: 'a\r\nb', text: 'a\n\rb', from: 0 },
])('identity and parser-extents mapping agree for every endpoint: %o', ({ source, text, from }) => {
  const offsets = prepareSourceOffsets(source);
  const identity = prepareIdentityMapping(offsets, text, from);
  const dense = prepareValueMapping(
    offsets,
    text,
    Array.from({ length: text.length }, (_, index) => ({
      from: from + index,
      to: from + index + 1,
    })),
    from,
  );
  expect(identity).toEqual(dense);
  for (let start = 0; start <= text.length; start++)
    for (let end = start; end <= text.length; end++) {
      expect(decodedRangeToSource(identity, { from: start, to: end })).toEqual(
        decodedRangeToSource(dense, { from: start, to: end }),
      );
      expect(sourceRangeToDecoded(identity, { from: from + start, to: from + end })).toEqual(
        sourceRangeToDecoded(dense, { from: from + start, to: from + end }),
      );
    }
  expect(identity.sourceOffsets).toBe(offsets);
  expect(Object.isFrozen(identity)).toBe(true);
  for (const value of Object.values(identity))
    if (value && typeof value === 'object') expect(Object.isFrozen(value)).toBe(true);
});

test.each([
  { source: 'a\r\nb', text: 'a\r', from: 0 },
  { source: 'a\r\nb', text: '\nb', from: 2 },
  { source: 'a🧭b', text: 'x', from: 2 },
  { source: 'plain', text: 'too long', from: 0 },
])(
  'reject identity extents with the same failure as the parser-extents path: %o',
  ({ source, text, from }) => {
    const offsets = prepareSourceOffsets(source);
    expect(() => prepareIdentityMapping(offsets, text, from)).toThrow();
    expect(() =>
      prepareValueMapping(
        offsets,
        text,
        Array.from({ length: text.length }, (_, index) => ({
          from: from + index,
          to: from + index + 1,
        })),
        from,
      ),
    ).toThrow();
  },
);

test('a large identity mapping retains bounded objects rather than a range object per character', () => {
  const source = 'plain prompt '.repeat(8000);
  const mapping = prepareIdentityMapping(prepareSourceOffsets(source), source, 0);
  const objects = new Set<object>();
  /** Count retained identity, including nested objects, without relying on private field names. */
  function visit(value: unknown): void {
    if (!value || typeof value !== 'object' || objects.has(value)) return;
    objects.add(value);
    const children: readonly unknown[] = Object.values(value);
    for (const child of children) visit(child);
  }
  visit(mapping);
  expect(objects.size).toBeLessThan(32);
});

test('full single-width source mappings share one frozen numeric identity table', () => {
  const source = 'plain\nprompt\rtext '.repeat(5000);
  const offsets = prepareSourceOffsets(source),
    mapping = prepareIdentityMapping(offsets, source, 0);
  const tables = [
    offsets.pythonToUtf16,
    offsets.utf16ToPython,
    mapping.pythonToDecoded,
    mapping.decodedToPython,
    mapping.decodedToSource,
    mapping.sourceToDecoded,
  ];
  expect(new Set(tables).size).toBe(1);
  expect(tables[0]).toHaveLength(source.length + 1);
  expect(tables[0]?.[source.length]).toBe(source.length);
  expect(Object.isFrozen(tables[0])).toBe(true);
});
