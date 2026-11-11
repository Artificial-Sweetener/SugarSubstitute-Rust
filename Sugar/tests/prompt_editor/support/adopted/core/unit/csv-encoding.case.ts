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
/** Independent storage spellings and parser-owned extents characterize the one canonical CSV encoder. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  prepareWildcardCsvEncoding,
  decodedRangeToSource,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { sourceRangeToDecoded } from '../../../../../../frontend/sugarprompt/core/src/document/value-mapping.ts';
import { parseWildcardCsv } from '../../../../../../frontend/sugarprompt/core/src/document/csv.ts';

for (const specimen of [
  { original: 'plain', value: 'changed', expected: 'changed', quoted: false },
  { original: '"plain"  ', value: 'a"b', expected: '"a""b"  ', quoted: true },
  { original: 'plain', value: 'a,b', expected: '"a,b"', quoted: true },
  { original: 'plain', value: 'a\r\nb', expected: '"a\r\nb"', quoted: true },
  { original: '"plain"', value: ' 😀 ', expected: '" 😀 "', quoted: true },
  { original: '"plain"', value: '', expected: '""', quoted: true },
]) {
  test('canonical CSV encoding retains quote policy, exact storage and parser-independent boundary expectations', () => {
    const cell = parseWildcardCsv(specimen.original).records[0]![0]!,
      encoded = prepareWildcardCsvEncoding(cell, specimen.value);
    assert.equal(encoded.text, specimen.expected);
    assert.equal(encoded.quoted, specimen.quoted);
    const parsed = parseWildcardCsv(specimen.expected).records[0]![0]!;
    assert.equal(parsed.value, specimen.value);
    for (let position = 0; position <= specimen.value.length; position++) {
      const before = specimen.value.charCodeAt(position - 1),
        after = specimen.value.charCodeAt(position),
        interior =
          (before >= 0xd800 && before <= 0xdbff && after >= 0xdc00 && after <= 0xdfff) ||
          (before === 13 && after === 10),
        absolute =
          parsed.characterRanges[position]?.from ??
          parsed.characterRanges.at(-1)?.to ??
          (specimen.quoted ? 1 : 0),
        decoded = { from: position, to: position },
        physical = { from: absolute, to: absolute };
      assert.deepEqual(decodedRangeToSource(encoded.mapping, decoded), interior ? null : physical);
      assert.deepEqual(sourceRangeToDecoded(encoded.mapping, physical), interior ? null : decoded);
    }
    assert.equal(encoded.mapping.sourceOffsets.source, specimen.expected);
    assert.equal(encoded.mapping.text, specimen.value);
  });
}

test('encoded quotes, astral interiors and physical CRLF seams retain the existing mapping rejections', () => {
  const cell = parseWildcardCsv('"x"').records[0]![0]!,
    encoded = prepareWildcardCsvEncoding(cell, '"😀\r\nx');
  assert.equal(encoded.text, '"""😀\r\nx"');
  assert.deepEqual(decodedRangeToSource(encoded.mapping, { from: 0, to: 1 }), {
    from: 1,
    to: 3,
  });
  assert.deepEqual(decodedRangeToSource(encoded.mapping, { from: 1, to: 3 }), {
    from: 3,
    to: 5,
  });
  assert.equal(decodedRangeToSource(encoded.mapping, { from: 2, to: 2 }), null);
  assert.equal(decodedRangeToSource(encoded.mapping, { from: 4, to: 4 }), null);
  assert.equal(encoded.mapping.sourceToDecoded[1], null);
});

test('forced quoting protects the empty-record seam without losing the empty decoded anchor', () => {
  const cell = parseWildcardCsv('plain').records[0]![0]!,
    encoded = prepareWildcardCsvEncoding(cell, '', true);
  assert.equal(encoded.text, '""');
  assert.equal(encoded.quoted, true);
  assert.deepEqual(decodedRangeToSource(encoded.mapping, { from: 0, to: 0 }), {
    from: 1,
    to: 1,
  });
});
