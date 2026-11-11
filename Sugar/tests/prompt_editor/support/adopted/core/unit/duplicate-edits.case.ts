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
/** Independently expected authored output qualifies single-occurrence cleanup and exact excess transfer. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  codePointBoundaries,
  utf16ToCodePoint,
} from '../../../../../../frontend/sugarprompt/core/src/offsets.ts';
import { parsePrompt } from '../../../../../../frontend/sugarprompt/core/src/prompt/parser.ts';
import { ConditioningTopology } from '../../../../../../frontend/sugarprompt/core/src/prompt/conditioning.ts';
import { prepareDuplicateSegments } from '../../../../../../frontend/sugarprompt/core/src/diagnostics/duplicate-segments.ts';
import { prepareDuplicateEdit } from '../../../../../../frontend/sugarprompt/core/src/diagnostics/duplicate-edits.ts';

/** Prepare actual source-bound findings and execute a genuine selected finding through the pure edit owner. */
function edit(source: string, action: 'remove' | 'emphasize-first') {
  const document = parsePrompt(source);
  const prepared = prepareDuplicateSegments(document, {
    kind: 'conditioning',
    topology: new ConditioningTopology('independent', document.regions),
  });
  const finding = prepared.findings[0];
  assert.ok(finding);
  return prepareDuplicateEdit(prepared, source, finding, action);
}

test('remove owns one neighboring comma and horizontal padding while conserving all hard endings', () => {
  for (const [source, expected] of [
    ['cat, cat, dog', 'cat, dog'],
    ['"cat, dog", "cat, dog"', '"cat, dog"'],
    ['cat, dog, cat', 'cat, dog'],
    ['cat\r\ncat\rnext\n', 'cat\r\n\rnext\n'],
    ['🦊, dog, 🦊\r\n', '🦊, dog\r\n'],
  ]) {
    const result = edit(source!, 'remove');
    assert.equal(result.status, 'ready');
    if (result.status === 'ready') assert.equal(result.output.text, expected);
  }
});

test('emphasize first adds pinned0.10 and positive excess once with only final half-even rounding', () => {
  for (const [source, expected] of [
    ['cat, cat', '(cat:1.10)'],
    ['(cat:1.20), cat', '(cat:1.30)'],
    ['cat, (cat:1.30)', '(cat:1.40)'],
    ['(cat:1.20), (cat:0.80)', '(cat:1.30)'],
    ['red_hat, RED hat', '(red_hat:1.10)'],
    ['(cat:1.004), (cat:1.004)', '(cat:1.11)'],
    ['🦊\r\n(🦊:1.40)\n', '(🦊:1.50)\r\n\n'],
  ]) {
    const result = edit(source!, 'emphasize-first');
    assert.equal(result.status, 'ready');
    if (result.status === 'ready') {
      assert.equal(result.output.text, expected);
      assert.equal(result.output.boundaryPositions.length, source!.length + 1);
      assert.equal(Object.isFrozen(result.edits), true);
    }
  }
});

test('stale source and copied or foreign finding operands refuse without producing source', () => {
  const document = parsePrompt('cat, cat');
  const prepared = prepareDuplicateSegments(document, {
    kind: 'conditioning',
    topology: new ConditioningTopology('independent', document.regions),
  });
  const finding = prepared.findings[0]!;
  assert.deepEqual(prepareDuplicateEdit(prepared, 'dog, dog', finding, 'remove'), {
    status: 'rejected',
    reason: 'stale',
  });
  assert.deepEqual(prepareDuplicateEdit(prepared, document.sourceText, { ...finding }, 'remove'), {
    status: 'rejected',
    reason: 'foreign',
  });
  const other = prepareDuplicateSegments(document, prepared.ownership);
  assert.deepEqual(
    prepareDuplicateEdit(prepared, document.sourceText, other.findings[0]!, 'remove'),
    { status: 'rejected', reason: 'foreign' },
  );
});

test('explicit retained content correspondence maps every valid scalar/endings boundary and collapses removed nested shells', () => {
  for (const [source, contentFrom, contentTo] of [
    ['a🦊b, a🦊b', 0, 4],
    ['(a🦊b:1.20), a🦊b', 1, 5],
    ['((a🦊b:1.20):1.30), a🦊b', 2, 6],
    ['🦊, 🦊\r\nnext\rtail\n', 0, 2],
  ] as const) {
    const result = edit(source, 'emphasize-first');
    assert.equal(result.status, 'ready');
    if (result.status !== 'ready') continue;
    const map = result.output.boundaryPositions;
    for (const boundary of codePointBoundaries(source)) {
      if (source[boundary - 1] === '\r' && source[boundary] === '\n') continue;
      assert.doesNotThrow(() => utf16ToCodePoint(result.output.text, map[boundary]!));
      assert.equal(
        result.output.text[map[boundary]! - 1] === '\r' &&
          result.output.text[map[boundary]!] === '\n',
        false,
      );
    }
    for (const boundary of codePointBoundaries(source).filter(
      (value) => value >= contentFrom && value < contentTo,
    ))
      assert.equal(map[boundary], 1 + boundary - contentFrom);
    assert.equal(map[0], 1);
    assert.equal(map.at(-1), result.output.text.length);
    if (contentFrom > 0) {
      assert.equal(map[contentTo], 1 + contentTo - contentFrom);
      for (let boundary = 0; boundary < contentFrom; boundary++) assert.equal(map[boundary], 1);
    }
  }
  const exact = edit('a🦊b, a🦊b', 'emphasize-first');
  assert.equal(exact.status, 'ready');
  if (exact.status === 'ready') assert.equal(exact.output.boundaryPositions[3], 4);
});
