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
/** Independent Unicode and exact newline specimens prove coordinate and serialization contracts. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  codePointBoundaries,
  codePointToUtf16,
  utf16ToCodePoint,
} from '../../../../../../frontend/sugarprompt/core/src/offsets.ts';
import {
  decodeSource,
  documentToSource,
  encodeSource,
  sourceToDocument,
  sourcePositionsToDocument,
} from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';

test('explicit coordinate spaces cover astral, combining, ZWJ, selectors and unpaired surrogates', () => {
  const source = 'A🙂e\u0301\ufe0f👩‍💻\t\r\n\ud800';
  const boundaries = [0, 1, 3, 4, 5, 6, 8, 9, 11, 12, 13, 14, 15];
  assert.deepEqual(codePointBoundaries(source), boundaries);
  for (const [point, utf16] of boundaries.entries()) {
    assert.equal(codePointToUtf16(source, point), utf16);
    assert.equal(utf16ToCodePoint(source, utf16), point);
  }
  for (const position of [-1, 2, 7, 10, 16, 0.5, NaN])
    assert.throws(() => utf16ToCodePoint(source, position), RangeError);
  assert.throws(() => codePointToUtf16(source, -1), RangeError);
});

test('batch source mapping preserves unsorted repeated Unicode boundaries and rejects CRLF interiors', () => {
  const encoded = encodeSource('🙂\r\nx\ry\nz');
  assert.deepEqual(
    sourcePositionsToDocument(encoded.logical, encoded.endings, [9, 4, 2, 4, 0, 7]),
    [8, 3, 2, 3, 0, 6],
  );
  assert.deepEqual(sourcePositionsToDocument('', [], [0, 0]), [0, 0]);
  assert.deepEqual(sourcePositionsToDocument(encoded.logical, encoded.endings, []), []);
  for (const position of [3, -1, 11, 0.5, NaN, Infinity])
    assert.throws(
      () => sourcePositionsToDocument(encoded.logical, encoded.endings, [0, position]),
      RangeError,
    );
});

test('mixed newline serialization and every representable source/document boundary are exact', () => {
  for (const source of ['', '\r\n', 'a\r\nb\nc\rd\t🙂', '\r\n\r\n\r\n']) {
    const encoded = encodeSource(source);
    assert.equal(decodeSource(encoded.logical, encoded.endings), source);
    for (let position = 0; position <= encoded.logical.length; position++) {
      assert.equal(
        sourceToDocument(
          encoded.logical,
          encoded.endings,
          documentToSource(encoded.logical, encoded.endings, position),
        ),
        position,
      );
    }
  }
  assert.throws(() => sourceToDocument('a\nb', ['\r\n'], 2), /inside CRLF/u);
  assert.throws(() => decodeSource('a\nb', []), /Missing/u);
  assert.throws(() => decodeSource('a', ['\n']), /Invalid/u);
});

test('single-width newline mapping preserves endpoints and rejects malformed positions in every batch order', () => {
  for (const source of ['', 'a🙂b', 'a\nb\rc\n', '\r\nless'.replace('\r\n', '\n')]) {
    const encoded = encodeSource(source),
      positions = Object.freeze([
        source.length,
        0,
        ...Array.from({ length: source.length + 1 }, (_, i) => i),
        0,
      ]);
    assert.deepEqual(
      sourcePositionsToDocument(encoded.logical, encoded.endings, positions),
      positions,
    );
    for (const invalid of [-1, source.length + 1, 0.5, NaN, Infinity])
      for (const batch of [
        [invalid, 0],
        [0, invalid],
      ])
        assert.throws(
          () => sourcePositionsToDocument(encoded.logical, encoded.endings, batch),
          RangeError,
        );
  }
});
