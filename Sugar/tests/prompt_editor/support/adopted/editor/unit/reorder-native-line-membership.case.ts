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
/** Actual native font-line observations distinguish blank carriers from visible paint and nested rich extents. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { MountedReorderTarget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-target.ts';
import { readReorderPhysicalFragments } from '../../../../../../frontend/sugarprompt/editor/src/reorder-placement-measurement.ts';
import { prepareReorderPlacementTopology } from '../../../../../../frontend/sugarprompt/editor/src/reorder-placement-topology.ts';
import { reorderKeyboardTarget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-keyboard-geometry.ts';
import {
  hideReorderChip,
  valueReorderLayout,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { captureReorder, reorderState } from '../transaction/reorder-fixture.ts';
import { physicalFixture } from './reorder-physical-fixture.ts';

/** Only native Range endpoints and parent-line identity are supplied; production grouping and Core rules remain real. */
function nativeTarget(
  kind: 'chip' | 'gap',
  index: number,
  blank: number,
  left: number,
  top: number,
  parent: Element,
) {
  const f = physicalFixture(),
    root = f.target.node,
    marker = f.inner as unknown as HTMLElement;
  Object.defineProperty(f.text, 'isConnected', { value: true });
  Object.defineProperty(marker, 'isConnected', { value: true });
  Object.defineProperty(marker, 'ownerDocument', { value: root.ownerDocument });
  root.closest = (() => parent) as Element['closest'];
  marker.closest = root.closest;
  root.ownerDocument.createRange = () => {
    let node: Node = root,
      offset = 0;
    return {
      setStart(next: Node, at: number): void {
        node = next;
        offset = at;
      },
      collapse(): void {},
      getClientRects(): DOMRectList {
        const smallerFont = kind === 'chip' && node === (f.text as unknown as Node);
        return [
          {
            left: left + (offset ? 20 : 0),
            top: top + (smallerFont ? 4 : 0),
            bottom: top + (smallerFont ? 16 : 20),
          },
        ] as unknown as DOMRectList;
      },
    } as Range;
  };
  marker.getClientRects = () =>
    [
      {
        left: left - 2,
        right: left + 22,
        top: top - 6,
        bottom: top + 26,
        width: 24,
        height: 32,
      },
    ] as unknown as DOMRectList;
  return {
    f,
    target: {
      ...f.target,
      node: kind === 'chip' ? root : marker,
      kind,
      index,
      ...(kind === 'gap' ? { blank } : {}),
    } as MountedReorderTarget,
    rect: {
      kind,
      index,
      ...(kind === 'gap' ? { blank } : {}),
      left,
      right: left + (kind === 'gap' ? 12 : 20),
      top,
      bottom: top + (kind === 'gap' ? 15 : 20),
    },
  };
}
test('EOF blank marker shares actual body font-line membership and ArrowRight crosses the first surviving chip', () => {
  const parent = { isConnected: true } as Element,
    cases = [
      nativeTarget('gap', 0, 0, 0, 0, parent),
      nativeTarget('gap', 0, 1, 0, 30, parent),
      nativeTarget('chip', 1, 0, 10, 30, parent),
      nativeTarget('chip', 2, 0, 40, 30, parent),
    ];
  try {
    const physical = readReorderPhysicalFragments(
      cases.map((x) => x.target),
      cases.map((x) => x.rect),
    )!;
    assert.equal(physical[1]!.line.identity, physical[2]!.line.identity);
    assert.equal(physical[2]!.line.identity, physical[3]!.line.identity);
    assert.notEqual(physical[0]!.line.identity, physical[1]!.line.identity);
    assert.deepEqual([physical[1]!.line.top, physical[1]!.line.bottom], [30, 50]);
    assert.deepEqual([physical[1]!.rect.top, physical[1]!.rect.bottom], [30, 45]);
    assert.deepEqual([physical[2]!.rect.top, physical[2]!.rect.bottom], [24, 56]);
    const capture = captureReorder(reorderState('a\nb, c').state),
      order = capture.prepared.initial,
      hidden = hideReorderChip(capture.prepared, order, 0)!,
      topology = prepareReorderPlacementTopology(valueReorderLayout(hidden), physical, {
        left: 0,
        right: 200,
        top: 0,
        bottom: 200,
      })!;
    assert.equal(
      topology.slots.some((s) => s.target.kind === 'gap' && s.target.blank === 1),
      false,
    );
    assert.deepEqual(
      reorderKeyboardTarget(order, hidden, 0, topology, 'ArrowRight', null)?.target,
      { kind: 'row', row: 0, insertion: 1 },
    );
  } finally {
    cases.forEach((x) => x.f.owner.destroy());
  }
});
test('different native parent lines never merge at equal font bounds', () => {
  const a = nativeTarget('chip', 1, 0, 10, 30, {
      isConnected: true,
    } as Element),
    b = nativeTarget('chip', 2, 0, 40, 30, { isConnected: true } as Element);
  try {
    const physical = readReorderPhysicalFragments([a.target, b.target], [a.rect, b.rect])!;
    assert.notEqual(physical[0]!.line.identity, physical[1]!.line.identity);
  } finally {
    a.f.owner.destroy();
    b.f.owner.destroy();
  }
});
test('warmed font-line membership refuses former association when the same targets move to an equivalent native parent', () => {
  const firstParent = { isConnected: true } as Element,
    nextParent = { isConnected: true } as Element,
    a = nativeTarget('chip', 1, 0, 10, 30, firstParent),
    b = nativeTarget('chip', 2, 0, 40, 30, firstParent),
    targets = [a.target, b.target],
    rects = [a.rect, b.rect];
  try {
    const original = a.f.owner.physical(targets, rects)!;
    const document = a.target.node.ownerDocument,
      read = document.createRange,
      refuse = () => {
        throw new Error('controlled-native-range-read');
      };
    assert.equal(original[0]!.line.identity, original[1]!.line.identity);
    document.createRange = refuse;
    assert.equal(a.f.owner.physical(targets, rects), original);
    b.target.node.closest = (() => nextParent) as Element['closest'];
    assert.throws(() => a.f.owner.physical(targets, rects), /controlled-native-range-read/u);
    document.createRange = read;
    const renewed = a.f.owner.physical(targets, rects)!;
    assert.notEqual(renewed, original);
    assert.notEqual(renewed[0]!.line.identity, renewed[1]!.line.identity);
    document.createRange = refuse;
    assert.equal(a.f.owner.physical(targets, rects), renewed);
    Object.defineProperty(nextParent, 'isConnected', { value: false });
    assert.equal(a.f.owner.physical(targets, rects), null);
    assert.equal(readReorderPhysicalFragments(targets, rects), null);
  } finally {
    a.f.owner.destroy();
    b.f.owner.destroy();
  }
});
