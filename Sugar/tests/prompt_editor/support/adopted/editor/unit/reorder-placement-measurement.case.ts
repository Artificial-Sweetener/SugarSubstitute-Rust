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
/** Literal native DOM boundary transport proves cold geometry admission and actual retained output, never browser layout parity. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { readReorderPhysicalFragments } from '../../../../../../frontend/sugarprompt/editor/src/reorder-placement-measurement.ts';
import type { MountedReorderTarget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-target.ts';
import { physicalFixture } from './reorder-physical-fixture.ts';
test('native caret endpoints preserve RTL while actual rich descendants expand the same physical band', () => {
  const f = physicalFixture(),
    physical = readReorderPhysicalFragments([f.target], [f.box])!;
  assert.deepEqual(physical, [
    {
      rect: { ...f.box, left: 5, right: 35, top: 0, bottom: 40 },
      line: { identity: 0, top: 10, bottom: 30 },
      leading: 30,
      trailing: 10,
    },
  ]);
  f.owner.destroy();
});
test('wrapped descendant rectangles expand only their actual native caret line', () => {
  const f = physicalFixture(),
    root = f.target.node,
    text = root.childNodes[0]!,
    inner = root.childNodes[1] as HTMLElement,
    later = { ...f.box, top: 70, bottom: 90 };
  inner.getClientRects = () =>
    [
      { left: 5, right: 35, top: 0, bottom: 40, width: 30, height: 40 },
      { left: 7, right: 37, top: 60, bottom: 100, width: 30, height: 40 },
    ] as unknown as DOMRectList;
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
        const box = node === text ? (offset < 3 ? f.box : later) : offset < 2 ? f.box : later;
        return [
          { left: offset === 0 ? 30 : 10, top: box.top, bottom: box.bottom },
        ] as unknown as DOMRectList;
      },
    } as Range;
  };
  const physical = readReorderPhysicalFragments([f.target], [f.box, later])!;
  assert.deepEqual(
    physical.map(({ rect }) => [rect.left, rect.right, rect.top, rect.bottom]),
    [
      [5, 35, 0, 40],
      [7, 37, 60, 100],
    ],
  );
  assert.deepEqual(
    physical.map(({ line }) => line.identity),
    [0, 1],
  );
  f.owner.destroy();
});
test('warmed measured output reuses the actual physical array without another native Range inspection', () => {
  const f = physicalFixture(),
    rects = f.owner.measure([f.target])!,
    physical = f.owner.physical([f.target], rects)!;
  f.block();
  assert.equal(f.owner.physical([f.target], rects), physical);
  f.resize();
  const fresh = f.owner.measure([f.target])!;
  assert.notEqual(fresh, rects);
  assert.throws(() => f.owner.physical([f.target], fresh), /controlled-native-range-read/);
  f.unblock();
  const renewed = f.owner.physical([f.target], fresh)!;
  assert.notEqual(renewed, physical);
  assert.deepEqual(renewed, physical);
  f.detach();
  assert.equal(f.owner.physical([f.target], fresh), null);
  f.owner.destroy();
  assert.equal(f.owner.physical([f.target], fresh), null);
});
test('oversized or detached cold paint refuses before native Range measurement and never truncates geometry', () => {
  const f = physicalFixture();
  f.block();
  f.oversize();
  assert.equal(readReorderPhysicalFragments([f.target], [f.box]), null);
  f.owner.destroy();
  const detached = physicalFixture();
  detached.block();
  detached.detach();
  assert.equal(readReorderPhysicalFragments([detached.target], [detached.box]), null);
  detached.owner.destroy();
});

test('actual font invalidation replaces physical caret output even with identical outer rectangle operands', () => {
  const f = physicalFixture(),
    rects = f.owner.measure([f.target])!,
    first = f.owner.physical([f.target], rects)!;
  f.block();
  f.owner.invalidate();
  const renewed = f.owner.measure([f.target])!;
  assert.notEqual(renewed, rects);
  assert.deepEqual(renewed, rects);
  assert.throws(() => f.owner.physical([f.target], renewed), /controlled-native-range-read/);
  f.unblock();
  const next = f.owner.physical([f.target], renewed)!;
  assert.notEqual(next, first);
  assert.deepEqual(next, first);
  f.owner.destroy();
});
test('admitted adversarial wrap refuses the whole snapshot before unbounded association work', () => {
  const f = physicalFixture(),
    text = f.target.node.childNodes[0] as Text,
    inner = f.target.node.childNodes[1] as HTMLElement;
  text.data = 'x'.repeat(8192);
  Object.defineProperty(text, 'length', { value: 8192 });
  let actualRectangleReads = 0;
  const rects = Array.from({ length: 8192 }, () => ({
    ...f.box,
    get top(): number {
      actualRectangleReads++;
      if (actualRectangleReads > 40000) throw new Error('unbounded-native-band-association');
      return 10;
    },
  }));
  inner.getClientRects = () =>
    Array.from({ length: 8192 }, () => ({
      left: 5,
      right: 35,
      top: 0,
      bottom: 40,
      width: 30,
      height: 40,
    })) as unknown as DOMRectList;
  assert.equal(readReorderPhysicalFragments([f.target], rects), null);
  assert.ok(actualRectangleReads > 8192);
  assert.ok(actualRectangleReads <= 32784);
  f.owner.destroy();
  const fallback = physicalFixture();
  assert.ok(readReorderPhysicalFragments([fallback.target], [fallback.box]));
  fallback.owner.destroy();
});
test('admitted wide rich siblings cannot hide quadratic parent-offset inspection', () => {
  const f = physicalFixture(),
    root = f.target.node;
  let actualChildReads = 0;
  const siblings = Array.from(
    { length: 8190 },
    () =>
      ({
        nodeType: 1,
        childNodes: [],
        parentNode: root,
        getClientRects: () => [{ left: 5, right: 35, top: 0, bottom: 40, width: 30, height: 40 }],
      }) as unknown as Node,
  );
  const children = new Proxy([root.childNodes[0]!, ...siblings], {
    get(target, property, receiver): unknown {
      if (typeof property === 'string' && /^\d+$/u.test(property)) {
        actualChildReads++;
        if (actualChildReads > 50000) throw new Error('unbounded-native-parent-association');
      }
      return Reflect.get(target, property, receiver);
    },
  });
  Object.defineProperty(root, 'childNodes', { value: children });
  assert.equal(readReorderPhysicalFragments([f.target], [f.box]), null);
  assert.ok(actualChildReads > 8191);
  assert.ok(actualChildReads <= 40960);
  f.owner.destroy();
});
test('empty targets use the existing native Text caret when collapsed Element boundaries have no rectangles', () => {
  const f = physicalFixture(),
    root = f.target.node,
    text = root.childNodes[0] as Text,
    marker = root.childNodes[1] as HTMLElement,
    target: MountedReorderTarget = {
      ...f.target,
      node: marker,
      kind: 'gap',
      index: 0,
      blank: 0,
    },
    rect = {
      ...f.box,
      kind: 'gap' as const,
      index: 0,
      blank: 0,
      left: 100,
      right: 112,
      top: 70,
      bottom: 90,
    };
  text.data = '\u200b';
  Object.defineProperty(text, 'length', { value: 1 });
  Object.defineProperty(text, 'isConnected', {
    value: true,
    configurable: true,
  });
  Object.defineProperty(marker, 'isConnected', { get: () => root.isConnected });
  Object.defineProperty(marker, 'ownerDocument', { value: root.ownerDocument });
  let observed: Node | null = null,
    offset = -1,
    blocked = false;
  root.ownerDocument.createRange = () => {
    if (blocked) throw new Error('unexpected-empty-caret-remeasurement');
    return {
      setStart(node: Node, at: number): void {
        observed = node;
        offset = at;
      },
      collapse(): void {},
      getClientRects: () =>
        observed?.nodeType === 3
          ? ([{ left: 57.125, top: 70, bottom: 90 }] as unknown as DOMRectList)
          : ([] as unknown as DOMRectList),
    } as Range;
  };
  const physical = f.owner.physical([target], [rect])!;
  assert.equal(observed, text);
  assert.equal(offset, 0);
  assert.equal(physical[0]!.leading, 57.125);
  assert.equal(physical[0]!.trailing, 57.125);
  blocked = true;
  const rects = [rect];
  blocked = false;
  const warmed = f.owner.physical([target], rects)!;
  blocked = true;
  assert.equal(f.owner.physical([target], rects), warmed);
  Object.defineProperty(text, 'isConnected', { value: false });
  assert.equal(f.owner.physical([target], rects), null);
  assert.equal(readReorderPhysicalFragments([target], rects), null);
  const replacement = {
    nodeType: 3,
    parentNode: root,
    isConnected: true,
    childNodes: [],
    data: '\u200b',
    length: 1,
  } as unknown as Text;
  Object.defineProperty(root, 'childNodes', {
    value: [replacement, marker],
    configurable: true,
  });
  assert.throws(() => f.owner.physical([target], rects), /unexpected-empty-caret-remeasurement/u);
  blocked = false;
  const renewed = f.owner.physical([target], rects)!;
  assert.notEqual(renewed, warmed);
  assert.equal(observed, replacement);
  assert.deepEqual(renewed, warmed);
  Object.defineProperty(root, 'childNodes', { value: [marker] });
  assert.equal(f.owner.physical([target], rects), null);
  f.owner.destroy();
});
