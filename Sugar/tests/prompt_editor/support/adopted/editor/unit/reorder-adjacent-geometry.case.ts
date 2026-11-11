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
import { resolveReorderPlacement } from '../../../../../../frontend/sugarprompt/editor/src/reorder-placement-topology.ts';
import { literalPlacementTopology } from './reorder-placement-fixture.ts';
/** Vacant keyboard lanes use viewport hit centers and source adjacency while retaining measured pointer and marker anchors. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  hideReorderChip,
  dropReorderChip,
  serializeValueReorder,
  valueReorderLayout,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { captureReorder, reorderState } from '../transaction/reorder-fixture.ts';
import { reorderKeyboardTarget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-keyboard-geometry.ts';
import {
  reorderHeldSlot,
  type ReorderRect,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-geometry.ts';

/** Literal CSS-pixel fragments carry semantic identity; bounds are authored independently of keyboard hit centers. */
function laneFragment(
  kind: ReorderRect['kind'],
  index: number,
  bounds: readonly [left: number, right: number, top: number, bottom: number],
  blank?: number,
): ReorderRect {
  const [left, right, top, bottom] = bounds;
  return {
    kind,
    index,
    left,
    right,
    top,
    bottom,
    ...(blank === undefined ? {} : { blank }),
  };
}

for (const width of [37, 183, 701])
  for (const originKind of ['gap', 'partition'] as const)
    test(`vacant keyboard lane uses viewport width=${width}, kind=${originKind}`, () => {
      const capture = captureReorder(
          reorderState(originKind === 'gap' ? 'a\nb\nc' : 'b\n[SEP|Face]\nc').state,
        ),
        held = originKind === 'gap' ? 1 : 0,
        order = capture.prepared.initial,
        hidden = hideReorderChip(capture.prepared, order, held)!,
        geometry = {
          viewport: { left: -17, right: width - 17, top: 0, bottom: 150 },
          rects: [
            laneFragment(originKind, 0, [-12, -11, 40, 60], 0),
            laneFragment('chip', originKind === 'gap' ? 2 : 1, [-12, -2, 70, 90]),
            ...(originKind === 'gap' ? [laneFragment('chip', 0, [-12, width - 27, 10, 30])] : []),
          ],
        },
        result = reorderKeyboardTarget(
          order,
          hidden,
          held,
          literalPlacementTopology(valueReorderLayout(hidden), geometry),
          'ArrowDown',
          null,
        );
      assert.ok(result);
      assert.deepEqual(result.target, {
        kind: 'row',
        row: originKind === 'gap' ? 1 : 0,
        insertion: 1,
      });
      const dropped = dropReorderChip(capture.prepared, hidden, held, result.target)!;
      assert.equal(
        serializeValueReorder(capture.prepared, dropped)!.text,
        originKind === 'gap' ? 'a\n\nc, b' : '\n[SEP|Face]\nc, b',
      );
      assert.deepEqual(reorderHeldSlot(order, hidden, held, geometry.rects), {
        left: -12,
        right: -10,
        top: 40,
        bottom: 60,
      });
      assert.deepEqual(
        resolveReorderPlacement(
          literalPlacementTopology(valueReorderLayout(hidden), {
            rects: geometry.rects,
            viewport: geometry.viewport,
          }),
          { x: -11.5, y: 50 },
          null,
          null,
        )?.target ?? null,
        originKind === 'gap'
          ? { kind: 'gap', gap: 0, blank: 0 }
          : { kind: 'partition', partition: 0 },
      );
      if (originKind === 'gap') {
        const up = reorderKeyboardTarget(
          order,
          hidden,
          held,
          literalPlacementTopology(valueReorderLayout(hidden), geometry),
          'ArrowUp',
          null,
        )!;
        assert.ok(up);
        const before = dropReorderChip(capture.prepared, hidden, held, up.target)!;
        assert.equal(serializeValueReorder(capture.prepared, before)!.text, 'b, a\n\nc');
      }
    });

for (const preferred of [0, 75, 190])
  test(`adjacent wrapped lane retains preferred-column candidate, column=${preferred}`, () => {
    const capture = captureReorder(reorderState('a\nb\nc, d, e').state),
      order = capture.prepared.initial,
      hidden = hideReorderChip(capture.prepared, order, 1)!,
      geometry = {
        viewport: { left: 0, right: 200, top: 0, bottom: 180 },
        rects: [
          laneFragment('chip', 0, [10, 30, 10, 30]),
          laneFragment('gap', 0, [10, 11, 40, 60], 0),
          laneFragment('chip', 2, [10, 30, 70, 90]),
          laneFragment('chip', 3, [100, 140, 70, 90]),
          laneFragment('chip', 4, [10, 30, 100, 120]),
        ],
      },
      result = reorderKeyboardTarget(
        order,
        hidden,
        1,
        literalPlacementTopology(valueReorderLayout(hidden), geometry),
        'ArrowDown',
        preferred,
      )!;
    assert.ok(result);
    const dropped = dropReorderChip(capture.prepared, hidden, 1, result.target)!;
    assert.equal(
      serializeValueReorder(capture.prepared, dropped)!.text,
      preferred === 190 ? 'a\n\nc, d, b, e' : 'a\n\nc, b, d, e',
    );
    assert.equal(result.preferredX, preferred === 190 ? 160 : 70);
  });

for (const direction of ['ArrowUp', 'ArrowDown'] as const)
  test(`nearest genuine blank ordinal remains a destination for ${direction}`, () => {
    const capture = captureReorder(reorderState('a\n\nb\n\nc').state),
      order = capture.prepared.initial,
      hidden = hideReorderChip(capture.prepared, order, 1)!,
      geometry = {
        viewport: { left: -50, right: 500, top: 0, bottom: 180 },
        rects: [
          laneFragment('chip', 0, [-40, 0, 10, 30]),
          ...[0, 1, 2].map((blank) => ({
            kind: 'gap' as const,
            index: 0,
            blank,
            left: -40,
            right: -39,
            top: 40 + blank * 30,
            bottom: 60 + blank * 30,
          })),
          laneFragment('chip', 2, [-40, 0, 130, 150]),
        ],
      },
      result = reorderKeyboardTarget(
        order,
        hidden,
        1,
        literalPlacementTopology(valueReorderLayout(hidden), geometry),
        direction,
        null,
      )!;
    assert.ok(result);
    const dropped = dropReorderChip(capture.prepared, hidden, 1, result.target)!;
    assert.equal(
      serializeValueReorder(capture.prepared, dropped)!.text,
      direction === 'ArrowUp' ? 'a\nb,\n\n\nc' : 'a\n\n\nb,\nc',
    );
    assert.equal(result.preferredX, 225);
  });

for (const width of [67, 431])
  test(`empty-partition destination retains a full viewport hit center, width=${width}`, () => {
    const capture = captureReorder(reorderState('a, b\n[SEP|Face]\n').state),
      order = capture.prepared.initial,
      hidden = hideReorderChip(capture.prepared, order, 0)!,
      geometry = {
        viewport: { left: -23, right: width - 23, top: 0, bottom: 120 },
        rects: [
          laneFragment('chip', 1, [-13, -3, 10, 30]),
          laneFragment('partition', 1, [-13, -12, 70, 90]),
        ],
      },
      result = reorderKeyboardTarget(
        order,
        hidden,
        0,
        literalPlacementTopology(valueReorderLayout(hidden), geometry),
        'ArrowDown',
        null,
      )!;
    assert.ok(result);
    const dropped = dropReorderChip(capture.prepared, hidden, 0, result.target)!;
    assert.equal(serializeValueReorder(capture.prepared, dropped)!.text, 'b\n[SEP|Face]\na');
    assert.equal(result.preferredX, width / 2 - 23);
    const vacant = hideReorderChip(capture.prepared, dropped, 0)!;
    assert.deepEqual(reorderHeldSlot(dropped, vacant, 0, geometry.rects), {
      left: -13,
      right: -11,
      top: 70,
      bottom: 90,
    });
  });

test('corrected Up occurrence stays in the chosen adjacent wrapped lane', () => {
  const capture = captureReorder(reorderState('a, x\nb\nc').state),
    order = capture.prepared.initial,
    hidden = hideReorderChip(capture.prepared, order, 2)!,
    geometry = {
      viewport: { left: 0, right: 200, top: 0, bottom: 160 },
      rects: [
        laneFragment('chip', 0, [10, 30, 10, 30]),
        laneFragment('chip', 1, [10, 30, 40, 60]),
        laneFragment('gap', 0, [10, 11, 70, 90], 0),
        laneFragment('chip', 3, [10, 30, 100, 120]),
      ],
    },
    result = reorderKeyboardTarget(
      order,
      hidden,
      2,
      literalPlacementTopology(valueReorderLayout(hidden), geometry),
      'ArrowUp',
      null,
    )!;
  assert.ok(result);
  const dropped = dropReorderChip(capture.prepared, hidden, 2, result.target)!;
  assert.equal(serializeValueReorder(capture.prepared, dropped)!.text, 'a, b, x\n\nc');
  assert.equal(result.preferredX, 10);
});
