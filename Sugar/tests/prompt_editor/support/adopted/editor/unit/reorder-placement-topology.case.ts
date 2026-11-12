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
/** Independent physical operands characterize donor bands, actual anchors and finite prepared topology. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  hideReorderChip,
  valueReorderLayout,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { captureReorder, reorderState } from '../transaction/reorder-fixture.ts';
import {
  prepareReorderPlacementTopology,
  resolveReorderPlacement,
  type ReorderPhysicalFragment,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-placement-topology.ts';
/** Core alone supplies semantic insertion ordinals; physical fragment inputs are literal native-coordinate specimens. */
function layout(source = 'a, b, c') {
  const capture = captureReorder(reorderState(source).state);
  return valueReorderLayout(hideReorderChip(capture.prepared, capture.prepared.initial, 0)!);
}
const viewport = { left: 0, right: 200, top: 0, bottom: 200 };
/** Each supplied baseline identity is independent of the rich painted height. */
function chip(
  index: number,
  left: number,
  right: number,
  top: number,
  bottom: number,
  line = 0,
  leading = left,
  trailing = right,
): ReorderPhysicalFragment {
  return {
    rect: { kind: 'chip', index, left, right, top, bottom },
    line: { identity: line, top: 10 + line * 30, bottom: 30 + line * 30 },
    leading,
    trailing,
  };
}
test('full viewport bands select insertion between native centers and actual anchors', () => {
  const topology = prepareReorderPlacementTopology(
    layout(),
    [chip(1, 10, 30, 10, 30), chip(2, 40, 60, 10, 30)],
    viewport,
  )!;
  assert.deepEqual(
    topology.slots.map((slot) => [slot.target, slot.hit.left, slot.hit.right, slot.anchor]),
    [
      [{ kind: 'row', row: 0, insertion: 0 }, 0, 20, 10],
      [{ kind: 'row', row: 0, insertion: 1 }, 20, 50, 40],
      [{ kind: 'row', row: 0, insertion: 2 }, 50, 200, 60],
    ],
  );
  assert.deepEqual(resolveReorderPlacement(topology, { x: 35, y: 20 }, null, null)?.target, {
    kind: 'row',
    row: 0,
    insertion: 1,
  });
});
test('nearest vertical interval precedes wide upper-line horizontal proximity', () => {
  const topology = prepareReorderPlacementTopology(
    layout('a, b\nc'),
    [chip(1, 0, 200, 10, 30), chip(2, 0, 60, 40, 60, 1)],
    viewport,
  )!;
  assert.deepEqual(resolveReorderPlacement(topology, { x: 180, y: 65 }, null, null)?.target, {
    kind: 'row',
    row: 1,
    insertion: 1,
  });
});
test('minimum eight-pixel overlap prefers current target and retains its exact wrapped placement', () => {
  const topology = prepareReorderPlacementTopology(
    layout(),
    [chip(1, 10, 12, 10, 30), chip(2, 13, 15, 10, 30)],
    viewport,
  )!;
  const middle = topology.slots[1]!,
    last = topology.slots[2]!;
  assert.equal(middle.hit.right - middle.hit.left, 8);
  assert.equal(resolveReorderPlacement(topology, { x: 15, y: 20 }, middle.target, middle), middle);
  assert.equal(resolveReorderPlacement(topology, { x: 15, y: 20 }, last.target, last), last);
});
test('nearest-line and anchor ties preserve prepared order without active fallback priority', () => {
  const topology = prepareReorderPlacementTopology(
    layout(),
    [chip(1, 10, 30, 10, 30), chip(2, 10, 30, 40, 60, 1)],
    viewport,
  )!;
  const later = topology.slots[2]!;
  assert.equal(
    resolveReorderPlacement(topology, { x: 20, y: 35 }, later.target, later),
    topology.slots[0],
  );
});
test('wrapped occurrences retain one Core identity and independent physical line slots', () => {
  const topology = prepareReorderPlacementTopology(
    layout(),
    [chip(1, 100, 150, 10, 30), chip(1, 10, 60, 40, 60, 1), chip(2, 70, 90, 40, 60, 1)],
    viewport,
  )!;
  assert.deepEqual(
    topology.slots.map((slot) => [slot.lane, slot.target]),
    [
      [0, { kind: 'row', row: 0, insertion: 0 }],
      [0, { kind: 'row', row: 0, insertion: 1 }],
      [1, { kind: 'row', row: 0, insertion: 0 }],
      [1, { kind: 'row', row: 0, insertion: 1 }],
      [1, { kind: 'row', row: 0, insertion: 2 }],
    ],
  );
  assert.equal(resolveReorderPlacement(topology, { x: 180, y: 50 }, null, null), topology.slots[4]);
});
test('rich-height bands expand actual line extents and blank lanes have no unconditional priority', () => {
  const semantic = layout('a\n\nb'),
    gap = semantic.gaps.find((candidate) => candidate.blankOffsets.length)!;
  const blank: ReorderPhysicalFragment = {
    rect: {
      kind: 'gap',
      index: gap.index,
      blank: 0,
      left: 0,
      right: 2,
      top: 40,
      bottom: 60,
    },
    line: { identity: 1, top: 40, bottom: 60 },
    leading: 0,
    trailing: 0,
  };
  const rich = chip(1, 10, 30, 20, 80, 2);
  const topology = prepareReorderPlacementTopology(semantic, [rich, blank], viewport)!;
  const row = topology.slots.find((slot) => slot.target.kind === 'row')!;
  assert.deepEqual(row.line, { top: 20, bottom: 90 });
  assert.equal(resolveReorderPlacement(topology, { x: 10, y: 50 }, row.target, row), row);
  assert.equal(
    resolveReorderPlacement(
      topology,
      { x: 10, y: 50 },
      blank.rect.kind === 'gap' ? { kind: 'gap', gap: gap.index, blank: 0 } : null,
      null,
    )?.target.kind,
    'gap',
  );
});
test('RTL anchors use measured leading/trailing coordinates without substituting rectangle edges', () => {
  const topology = prepareReorderPlacementTopology(
    layout(),
    [chip(1, 10, 30, 10, 30, 0, 30, 10), chip(2, 40, 60, 10, 30, 0, 60, 40)],
    viewport,
  )!;
  assert.deepEqual(
    topology.slots.map((slot) => slot.anchor),
    [30, 60, 40],
  );
  assert.equal(resolveReorderPlacement(topology, { x: 41, y: 210 }, null, null), topology.slots[2]);
});
test('whole prepared snapshot refuses oversized or malformed inputs without partial target success', () => {
  const fragments = Array.from({ length: 8193 }, () => chip(1, 10, 30, 10, 30));
  assert.equal(prepareReorderPlacementTopology(layout(), fragments, viewport), null);
  assert.equal(
    prepareReorderPlacementTopology(layout(), [chip(1, 10, 30, 10, NaN)], viewport),
    null,
  );
  assert.equal(prepareReorderPlacementTopology(layout(), [], { ...viewport, right: NaN }), null);
  const topology = prepareReorderPlacementTopology(layout(), [], viewport)!;
  assert.equal(resolveReorderPlacement(topology, { x: 0, y: 0 }, null, null), null);
});

test('disjoint bidi rectangles of one chip in one native line produce one physical insertion pair', () => {
  const topology = prepareReorderPlacementTopology(
    layout(),
    [
      chip(1, 10, 20, 10, 30, 0, 20, 10),
      chip(1, 40, 50, 10, 30, 0, 50, 40),
      chip(2, 60, 80, 10, 30),
    ],
    viewport,
  )!;
  assert.deepEqual(
    topology.slots.map((slot) => [slot.target, slot.hit.left, slot.hit.right, slot.anchor]),
    [
      [{ kind: 'row', row: 0, insertion: 0 }, 0, 30, 20],
      [{ kind: 'row', row: 0, insertion: 1 }, 30, 70, 60],
      [{ kind: 'row', row: 0, insertion: 2 }, 70, 200, 80],
    ],
  );
});

test('large prepared lines retain exact slot outputs after the captured source and physical inputs become unreadable', () => {
  const canonical = layout(Array.from({ length: 513 }, (_value, at) => `tag${at}`).join(', '));
  let retired = false;
  const captured = new Proxy(canonical, {
    get(target, key, receiver) {
      if (retired) throw new Error('prepared-source-reread');
      return Reflect.get(target, key, receiver);
    },
  });
  const fragments = Array.from(
    { length: 512 },
    (_value, at) =>
      new Proxy(chip(at + 1, at * 3, at * 3 + 2, 10, 30), {
        get(target, key, receiver) {
          if (retired) throw new Error('prepared-physical-reread');
          return Reflect.get(target, key, receiver);
        },
      }),
  );
  const topology = prepareReorderPlacementTopology(captured, fragments, {
    left: 0,
    right: 1600,
    top: 0,
    bottom: 200,
  })!;
  assert.equal(topology.slots.length, 513);
  assert.deepEqual(
    topology.slots.map((slot) => slot.target),
    Array.from({ length: 513 }, (_value, insertion) => ({
      kind: 'row',
      row: 0,
      insertion,
    })),
  );
  const expected = resolveReorderPlacement(topology, { x: 1500, y: 20 }, null, null)!;
  retired = true;
  for (let motion = 0; motion < 128; motion++)
    assert.equal(
      resolveReorderPlacement(topology, { x: 1500, y: 20 }, expected.target, expected),
      expected,
    );
});
