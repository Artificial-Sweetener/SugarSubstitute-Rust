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
/** Sole-parser nested segment metadata keeps existing grammar/parity output while avoiding reorder reparsing. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { parsePrompt } from '../../../../../../../frontend/sugarprompt/core/src/prompt/parser.ts';

test('only direct scanned commas populate each valid emphasis and frozen absolute ranges', () => {
  const document = parsePrompt('(🦊, "a,b", (c,d:1.2), {x,y}, \\(q,r\\), tail:0.80)');
  const outer = document.emphasisSpans[0]!;
  assert.deepEqual(
    outer.segments.map((segment) => segment.text),
    ['🦊', '"a,b"', '(c,d:1.2)', '{x,y}', '\\(q,r\\)', 'tail'],
  );
  assert.deepEqual(
    document.emphasisSpans[1]!.segments.map((segment) => segment.text),
    ['c', 'd'],
  );
  for (const span of document.emphasisSpans)
    for (const segment of span.segments) {
      assert.equal(
        document.sourceText.slice(segment.contentRange.from, segment.contentRange.to),
        segment.text,
      );
      assert.equal(Object.isFrozen(segment.contentRange), true);
      assert.equal(Object.isFrozen(segment), true);
    }
  assert.equal(Object.isFrozen(outer.segments), true);
});

test('invalid weights publish no nested metadata while final comma whitespace remains owned once', () => {
  assert.equal(parsePrompt('(a,b:not-weight)').emphasisSpans.length, 0);
  const source = '(a,\r\n b,\t :1.20)';
  const span = parsePrompt(source).emphasisSpans[0]!;
  assert.deepEqual(
    span.segments.map((segment) => segment.text),
    ['a', 'b'],
  );
  const separator = span.segments[1]!.separatorRange!;
  assert.equal(source.slice(separator.from, separator.to), ',\t ');
  assert.equal(separator.to, span.contentRange.to);
});

test('direct CRLF is one ending while quoted, escaped, literal and nested endings stay protected', () => {
  const source =
    'root\r\n"quoted\ntext"\r\\(literal\rtext\\)\n{wild\ncard}, (a\r\nb, "q\rr", (c\nd:1):1.2)';
  const document = parsePrompt(source);
  assert.deepEqual(
    document.segments[0]!.hardBreaks.map((range) => source.slice(range.from, range.to)),
    ['\r\n', '\r', '\n'],
  );
  const outer = document.emphasisSpans[0]!;
  assert.deepEqual(
    outer.segments[0]!.hardBreaks.map((range) => source.slice(range.from, range.to)),
    ['\r\n'],
  );
  assert.deepEqual(outer.segments[1]!.hardBreaks, []);
  assert.deepEqual(outer.segments[2]!.hardBreaks, []);
  assert.deepEqual(
    document.emphasisSpans[1]!.segments[0]!.hardBreaks.map((range) =>
      source.slice(range.from, range.to),
    ),
    ['\n'],
  );
  assert.equal(Object.isFrozen(document.segments[0]!.hardBreaks), true);
  assert.deepEqual(parsePrompt('one\\\r\ntwo').segments[0]!.hardBreaks, []);
  assert.deepEqual(parsePrompt('one\\\ntwo').segments[0]!.hardBreaks, []);
});
