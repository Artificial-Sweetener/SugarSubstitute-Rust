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
/** Carrier validation compares the same canonical DOM-preorder roles as its original Scene, after independent unchanged native-paint admission. */
import {
  admitReorderTargetLayout,
  admitReorderLineLayout,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-layout-admission.ts';
import assert from 'node:assert/strict';
import test from 'node:test';
import { usingAtomicPaintFixture, atomicScenePaintSource } from './reorder-atomic-paint-fixture.ts';
import { ReorderMotionPeerCarriers } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-peer-carriers.ts';
import { admitNativeReorderPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-paint.ts';
import { nativeReorderFootprint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-footprint.ts';
import type { PaintNode } from './reorder-paint-node-fixture.ts';

/** Literal native roles have deliberately different sibling rectangles; no output of either traversal supplies these operands. */
function roleBox(node: PaintNode): {
  left: number;
  top: number;
  width: number;
  height: number;
} {
  if (node.localName === 'wbr' || node.className === 'sp-lora-subtitle')
    return { left: 20, top: 20, width: 0, height: 0 };
  switch (node.className) {
    case 'sp-lora-initial':
      return { left: 22, top: 21, width: 5, height: 10 };
    case 'sp-lora-title':
      return { left: 30, top: 22, width: 17, height: 12 };
    case 'sp-lora-weight':
      return { left: 49, top: 23, width: 9, height: 14 };
    case 'sp-weight-value':
      return { left: 50, top: 24, width: 7, height: 12 };
    default:
      return { left: 20, top: 20, width: 40, height: 20 };
  }
}

for (const displacement of ['none', 'right-one', 'down-one'] as const)
  test(`canonical sibling roles ${displacement === 'none' ? 'retain all exact copied rectangles' : 'reject an actual copy-only one-pixel movement'} (${displacement})`, () =>
    usingAtomicPaintFixture((f) => {
      const root = f.roots[0]!,
        original = root.firstElementChild!;
      const lease = f.f.document.createElement('span'),
        before = f.f.document.createElement('wbr'),
        after = f.f.document.createElement('wbr');
      root.replaceChildren(lease);
      lease.append(before, original, after);
      for (const node of [root, ...root.querySelectorAll('*')])
        Object.assign(node.box, roleBox(node));
      const source = atomicScenePaintSource(f, 0);
      const literal = [
        [20, 60, 20, 40], // original outer inline
        [20, 60, 20, 40], // actual borrowed line span
        [20, 60, 20, 40], // canonical LoRA island
        [20, 60, 20, 40], // backing
        [20, 60, 20, 40], // ready native canvas
        [22, 27, 21, 31], // initial
        [30, 47, 22, 34], // title
        [49, 58, 23, 37], // weight cell
        [50, 57, 24, 36], // numeric value
      ];
      assert.deepEqual(
        source.rects.map((rect) => [rect.left, rect.right, rect.top, rect.bottom]),
        literal,
      );
      const paintNodes = admitNativeReorderPaint([root as unknown as HTMLElement]);
      assert.ok(paintNodes);
      const reverse = nativeReorderFootprint(root as unknown as HTMLElement, paintNodes, 0);
      assert.ok(reverse);
      assert.notDeepEqual(
        reverse.map((rect) => [rect.left, rect.right, rect.top, rect.bottom]),
        literal,
      );
      const create = f.f.document.createElement.bind(f.f.document);
      f.f.document.createElement = (name) => {
        const node = create(name);
        node.getBoundingClientRect = () => {
          const box = roleBox(node),
            dx = displacement === 'right-one' && node.className === 'sp-lora-title' ? 1 : 0,
            dy = displacement === 'down-one' && node.className === 'sp-lora-title' ? 1 : 0;
          return {
            ...box,
            left: box.left + dx,
            top: box.top + dy,
            x: box.left + dx,
            y: box.top + dy,
            right: box.left + dx + box.width,
            bottom: box.top + dy + box.height,
          } as DOMRect;
        };
        return node;
      };
      const owner = ReorderMotionPeerCarriers.prepare(
        f.view,
        f.f.line as unknown as HTMLElement,
        [source],
        1,
        1,
        () => true,
      );
      assert.ok(owner);
      try {
        assert.equal(
          owner.instantiate(() => true),
          true,
        );
        const copy = f.f.line.querySelector('.sp-reorder-motion-peer')!.firstElementChild!;
        assert.ok(admitNativeReorderPaint([copy as unknown as HTMLElement]));
        const nodes = admitReorderLineLayout([copy as unknown as HTMLElement]);
        assert.ok(nodes);
        const actual = nativeReorderFootprint(copy as unknown as HTMLElement, nodes, 0);
        assert.ok(actual);
        const expected = literal.map((rect) => rect.slice());
        if (displacement === 'right-one') {
          expected[6]![0]! += 1;
          expected[6]![1]! += 1;
        }
        if (displacement === 'down-one') {
          expected[6]![2]! += 1;
          expected[6]![3]! += 1;
        }
        assert.deepEqual(
          actual.map((rect) => [rect.left, rect.right, rect.top, rect.bottom]),
          expected,
        );
        assert.equal(
          owner.read([source], () => true, admitReorderTargetLayout),
          displacement === 'none' ? 'ready' : 'discard',
        );
        assert.deepEqual(
          source.rects.map((rect) => [rect.left, rect.right, rect.top, rect.bottom]),
          literal,
        );
      } catch (cause) {
        f.causes.push(cause);
      }
      try {
        owner.destroy();
      } catch (cause) {
        f.causes.push(cause);
      }
    }));
