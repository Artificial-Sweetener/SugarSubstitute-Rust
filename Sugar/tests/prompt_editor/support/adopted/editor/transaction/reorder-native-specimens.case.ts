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
/** Explicit native specimen operands qualify canonical issuance and history without browser geometry. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undo, redo, undoDepth } from '@codemirror/commands';
import {
  moveReorderChip,
  hideReorderChip,
  dropReorderChip,
  valueReorderLayout,
  serializeValueReorder,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { captureReorder, reorderState, transactionState } from './reorder-fixture.ts';
import { prepareReorderCommit } from '../../../../../../frontend/sugarprompt/editor/src/reorder-command.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  encodeSource,
  sourceToDocument,
  documentToSource,
} from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';

/** Exact-source UTF-16 endpoints are converted by the canonical newline owner, never by a second mapping. */
function logical(source: string, position: number): number {
  const encoded = encodeSource(source);
  return sourceToDocument(encoded.logical, encoded.endings, position);
}

for (const [ordinal, specimen] of [
  {
    source: '🙂<lora:one:1><lora:two:0.5>, {forest}, tail',
    expected: '<lora:one:1><lora:two:0.5>, {forest}, tail, 🙂',
    chips: ['🙂', '<lora:one:1>', '<lora:two:0.5>', '{forest}', 'tail'],
    before: [2, 0],
    after: [46, 44],
  },
  {
    source: '(a,b:1), tail',
    expected: '(b:1), tail, (a:1)',
    chips: ['(a', 'b:1)', 'tail'],
    before: [2, 1],
    after: [15, 14],
  },
  {
    source: '{first\r\nsecond}, tail',
    expected: 'tail, {first\r\nsecond}',
    chips: ['{first\r\nsecond}', 'tail'],
    before: [6, 1],
    after: [12, 7],
  },
].entries()) {
  test(`structured native operand has literal projected chips and one mapped transaction, specimen=${ordinal}`, () => {
    const { state } = reorderState(specimen.source, specimen.before[0]!, specimen.before[1]!),
      capture = captureReorder(state),
      initial = serializeValueReorder(capture.prepared, capture.prepared.initial)!;
    assert.deepEqual(
      capture.prepared.initial.order.map((id) => {
        const range = initial.renderedRanges[id]!;
        return initial.text.slice(range.from, range.to);
      }),
      specimen.chips,
    );
    const order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
        kind: 'row',
        row: 0,
        insertion: capture.prepared.chips.length - 1,
      })!,
      result = prepareReorderCommit(state, capture, order);
    assert.equal(result.status, 'ready');
    assert.equal(result.edit.transactions.length, 1);
    const native = transactionState(state);
    native.dispatch(result.edit.transactions[0]!);
    assert.equal(readExactSource(native.state) === specimen.expected, true);
    assert.deepEqual(
      [native.state.selection.main.anchor, native.state.selection.main.head],
      specimen.after,
    );
    assert.equal(undoDepth(native.state), 1);
    assert.equal(undo(native), true);
    assert.equal(readExactSource(native.state) === specimen.source, true);
    assert.deepEqual(
      [native.state.selection.main.anchor, native.state.selection.main.head],
      specimen.before,
    );
    assert.equal(redo(native), true);
    assert.equal(readExactSource(native.state) === specimen.expected, true);
  });
}

for (const ending of ['\n', '\r\n']) {
  for (const side of [0, 1]) {
    test(`each native regional blank operand preserves exact ending and directional history, ending=${ending.length}, side=${side}`, () => {
      const source = ['a', '', '[SEP|Face]', '', 'b, c'].join(ending),
        expected = (
          side === 0 ? ['a', 'c,', '[SEP|Face]', '', 'b'] : ['a', '', '[SEP|Face]', 'c,', 'b']
        ).join(ending),
        { state } = reorderState(
          source,
          logical(source, source.length),
          logical(source, source.length - 1),
        ),
        capture = captureReorder(state),
        hidden = hideReorderChip(capture.prepared, capture.prepared.initial, 2)!;
      assert.ok(hidden);
      assert.deepEqual(
        valueReorderLayout(hidden).gaps.map((gap) => [
          gap.index,
          gap.partition,
          gap.blankOffsets.length,
        ]),
        [
          [0, 0, 1],
          [1, 1, 1],
        ],
      );
      const order = dropReorderChip(capture.prepared, hidden, 2, {
          kind: 'gap',
          gap: side,
          blank: 0,
        })!,
        result = prepareReorderCommit(state, capture, order);
      assert.equal(result.status, 'ready');
      const native = transactionState(state);
      native.dispatch(result.edit.transactions[0]!);
      assert.equal(readExactSource(native.state) === expected, true);
      const encoded = encodeSource(expected),
        from =
          side === 0
            ? 1 + ending.length
            : 1 + ending.length * 2 + '[SEP|Face]'.length + ending.length;
      assert.deepEqual(
        [native.state.selection.main.anchor, native.state.selection.main.head].map((position) =>
          documentToSource(encoded.logical, encoded.endings, position),
        ),
        [from + 1, from],
      );
      assert.equal(undoDepth(native.state), 1);
      assert.equal(undo(native), true);
      assert.equal(readExactSource(native.state) === source, true);
      assert.deepEqual(
        [native.state.selection.main.anchor, native.state.selection.main.head],
        [logical(source, source.length), logical(source, source.length - 1)],
      );
      assert.equal(redo(native), true);
      assert.equal(readExactSource(native.state) === expected, true);
    });
  }
}
