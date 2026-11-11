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
/** Real Core output origins are checked against independent exact hard-line and native document operands. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection } from '@codemirror/state';
import { SourceWidget } from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import { isSceneMarkerWidget } from '../../../../../../frontend/sugarprompt/editor/src/scene-projection.ts';
import { RegionWidget } from '../../../../../../frontend/sugarprompt/editor/src/region-widget.ts';
import {
  dropReorderChip,
  hideReorderChip,
  serializeValueReorder,
  serializeReorderPreview,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { SourceLineOrigins } from '../../../../../../frontend/sugarprompt/editor/src/source-line-origins.ts';
import { captureReorder, reorderState } from '../transaction/reorder-fixture.ts';
import { projectionState } from '../transaction/projection-state.ts';
import {
  initializeNativeStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { LoraWidget } from '../../../../../../frontend/sugarprompt/editor/src/lora-widget.ts';
import { projectReorderOutput } from '../../../../../../frontend/sugarprompt/editor/src/reorder-projection.ts';
import { preparedReorderSourceLines } from '../../../../../../frontend/sugarprompt/editor/src/source-line-origins.ts';

for (const source of [
  'a😀, b\r\n\r\nc\rd\ne',
  '(first\nsecond:1.25), peer, final',
  'first, אבג\n\nthird, last',
]) {
  test('indexed canonical origins retain every exact hard line through hidden and complete transforms', () => {
    const capture = captureReorder(reorderState(source).state),
      prepared = capture.prepared,
      index = new SourceLineOrigins(prepared.scope, capture.projection),
      hidden = hideReorderChip(prepared, prepared.initial, capture.active);
    assert.ok(hidden);
    const moved = dropReorderChip(prepared, hidden, capture.active, {
      kind: 'row',
      row: 0,
      insertion: 1,
    });
    assert.ok(moved);
    for (const output of [
      serializeValueReorder(prepared),
      serializeReorderPreview(prepared, hidden),
      serializeValueReorder(prepared, moved),
    ]) {
      assert.ok(output);
      const lines = index.lines(output);
      assert.deepEqual(
        lines.map((line) => output.text.slice(line.range.from, line.range.to)),
        output.text.split(/\r\n|\r|\n/),
      );
      for (const line of lines) {
        let end = 0;
        for (const piece of line.pieces) {
          assert.equal(piece.outputRange.from, end);
          assert.ok(piece.outputRange.to >= end);
          end = piece.outputRange.to;
          if (piece.kind === 'unchanged' || piece.kind === 'copied')
            assert.equal(
              prepared.scope.document.sourceText.slice(
                piece.sourceRange.from,
                piece.sourceRange.to,
              ),
              output.text.slice(
                line.range.from + piece.outputRange.from,
                line.range.from + piece.outputRange.to,
              ),
            );
        }
        assert.equal(end, line.range.to - line.range.from);
      }
    }
  });
}

test('unchanged hard-line origin identities survive a different target without including absolute proposed offsets', () => {
  const capture = captureReorder(reorderState('a, b, c\n\nretained, tail').state),
    prepared = capture.prepared,
    index = new SourceLineOrigins(prepared.scope, capture.projection),
    initial = serializeValueReorder(prepared)!;
  const hidden = hideReorderChip(prepared, prepared.initial, capture.active)!;
  const moved = dropReorderChip(prepared, hidden, capture.active, {
    kind: 'row',
    row: 0,
    insertion: 2,
  })!;
  const next = serializeValueReorder(prepared, moved)!;
  assert.notEqual(initial.text, next.text);
  const before = index.lines(initial),
    after = index.lines(next);
  assert.equal(before.at(-1)!.identity, after.at(-1)!.identity);
  assert.notEqual(before[0]!.identity, after[0]!.identity);
});

test('foreign captured source identity cannot reuse a prepared newline index', () => {
  const first = captureReorder(reorderState('a, b').state).prepared,
    second = captureReorder(reorderState('a, b').state).prepared;
  assert.throws(
    () => new SourceLineOrigins(first.scope, null).lines(serializeValueReorder(second)!),
    { message: 'source-line-origin-source-mismatch' },
  );
});

test('a large single hard line retains complete recipes behind compact stable captured identities', () => {
  const source = Array.from({ length: 512 }, (_value, at) => `chip${at}`).join(', ');
  const capture = captureReorder(reorderState(source).state),
    output = serializeValueReorder(capture.prepared)!;
  const index = new SourceLineOrigins(capture.prepared.scope, capture.projection);
  const first = index.lines(output);
  assert.equal(first.length, 1);
  assert.ok(first[0]!.pieces.length >= 512);
  assert.ok(Number.isSafeInteger(first[0]!.identity));
  assert.equal(index.lines(output)[0]!.identity, first[0]!.identity);
  const targetOrigins = Array.from({ length: 512 }, () => [first[0]!.identity]);
  assert.ok(JSON.stringify(targetOrigins).length <= 512 * 5 + 1);
  assert.equal(output.text.slice(first[0]!.range.from, first[0]!.range.to), source);
});

test('canonical multiline LoRA paint suppresses covered source breaks without splitting its genuine widget', () => {
  let state = initializeNativeStructure(
    projectionState('before, <lora:model\nname:0.50>, after\nlast'),
  );
  state = state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
  const capture = captureReorder(state),
    output = serializeValueReorder(capture.prepared)!;
  assert.ok(capture.projection);
  const widgets: LoraWidget[] = [];
  capture.projection.decoded.between(0, output.text.length, (_from, _to, value) => {
    if (value.spec.widget instanceof LoraWidget) widgets.push(value.spec.widget);
  });
  assert.equal(widgets.length, 1);
  assert.equal(widgets[0]!.lineBreaks, 0);
  const lines = preparedReorderSourceLines(capture, output);
  assert.equal(lines.length, 2);
  assert.equal(lines[0]!.range.to, output.text.lastIndexOf('\n'));
  assert.equal(lines[1]!.range.from, output.text.lastIndexOf('\n') + 1);
  assert.equal(preparedReorderSourceLines(capture, output), lines);
  let projected = 0;
  projectReorderOutput(capture.projection, output).between(
    0,
    output.text.length,
    (_from, _to, value) => {
      if (value.spec.widget === widgets[0]) projected++;
    },
  );
  assert.equal(projected, 1);
  assert.deepEqual(
    lines.map((line) => line.widgetBreaks),
    [0, 0],
  );
});

/** Actual pinned DecorationSet callbacks are grouped across native range sets; the occupancy index must order them before cursor/binary discovery. */
test('mixed Scene and regional widgets retain exact six hard lines despite unordered native decoration callbacks', () => {
  const source = 'shared\n**One\n[SEP|Face]\nfirst, second, third, fourth\n**Two\nlast',
    initial = initializeNativeStructure(
      projectionState(source).update({ selection: EditorSelection.cursor(24) }).state,
    ),
    state = initial.update({
      effects: installProjection.of(prepareProjection(initial, prepareNativeStructure(initial))),
    }).state,
    capture = captureReorder(state, 24),
    output = serializeValueReorder(capture.prepared);
  assert.ok(capture.projection);
  assert.ok(output);
  const callbacks: number[][] = [];
  capture.projection.decoded.between(0, source.length, (from, to, value) => {
    const widget: unknown = value.spec.widget;
    if (!(widget instanceof SourceWidget)) return;
    assert.ok(isSceneMarkerWidget(widget) || widget instanceof RegionWidget);
    callbacks.push([from, to, isSceneMarkerWidget(widget) ? 0 : 1, widget.lineBreaks]);
  });
  assert.deepEqual(callbacks, [
    [7, 9, 0, 0],
    [53, 55, 0, 0],
    [13, 23, 1, 0],
  ]);
  const lines = preparedReorderSourceLines(capture, output);
  assert.deepEqual(
    lines.map((line) => [line.range.from, line.range.to]),
    [
      [0, 6],
      [7, 12],
      [13, 23],
      [24, 52],
      [53, 58],
      [59, 63],
    ],
  );
  assert.deepEqual(
    lines.map((line) => line.widgetBreaks),
    [0, 0, 0, 0, 0, 0],
  );
  assert.equal(preparedReorderSourceLines(capture, output), lines);
  assert.equal(output.text, source);
});
