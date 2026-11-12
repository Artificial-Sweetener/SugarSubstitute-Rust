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
import { withReorderPaintMaterialization } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-materialization.ts';
/** Positive chip paint and zero-flow gap caret geometry have distinct native contracts. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { EditorView } from '@codemirror/view';
import { physicalFixture } from './reorder-physical-fixture.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
import { prepareSparseReorderGrab } from '../../../../../../frontend/sugarprompt/editor/src/reorder-sparse-grab-paint.ts';
import { materializeReorderGrab } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-materialization.ts';

test('a zero-width inline chip fragment cannot veto its complete positive paint and real caret output', () => {
  const f = physicalFixture(),
    positive = { ...f.box, width: 20, height: 20 };
  f.target.node.getClientRects = () =>
    [
      { ...positive, left: 100, right: 100, width: 0, top: 70, bottom: 90 },
      positive,
    ] as unknown as DOMRectList;
  const rects = f.owner.measure([f.target])!;
  assert.deepEqual(rects, [f.box]);
  const physical = f.owner.physical([f.target], rects)!;
  assert.equal(physical.length, 1);
  assert.deepEqual(physical[0]!.rect, {
    ...f.box,
    left: 5,
    right: 35,
    top: 0,
    bottom: 40,
  });
  f.block();
  assert.equal(f.owner.physical([f.target], rects), physical);
  f.owner.destroy();
});

test('positive chip paint with no matching native caret band still refuses the complete physical mapping', () => {
  const f = physicalFixture();
  f.target.node.getClientRects = () =>
    [{ ...f.box, width: 20, height: 20, top: 70, bottom: 90 }] as unknown as DOMRectList;
  const rects = f.owner.measure([f.target])!;
  assert.equal(rects.length, 1);
  assert.equal(f.owner.physical([f.target], rects), null);
  f.owner.destroy();
});

test('an invalid native fragment cannot disappear behind a valid positive peer or reuse cached geometry', () => {
  const f = physicalFixture(),
    valid = { ...f.box, width: 20, height: 20 };
  assert.ok(f.owner.measure([f.target]));
  f.target.node.getClientRects = () =>
    [valid, { ...valid, left: NaN, right: NaN, width: 0 }] as unknown as DOMRectList;
  assert.equal(f.owner.measure([f.target]), null);
  f.owner.destroy();
});

test('zero-width nonchip geometry remains a real native caret target', () => {
  const f = physicalFixture(),
    target = { ...f.target, kind: 'gap' as const };
  f.target.node.getClientRects = () =>
    [{ ...f.box, right: f.box.left, width: 0, height: 20 }] as unknown as DOMRectList;
  const rects = f.owner.measure([target])!;
  assert.deepEqual(rects, [{ ...f.box, kind: 'gap', right: f.box.left }]);
  f.owner.destroy();
});

/** Only independently controlled DOM fragment transport varies; original complete paint/materialization owners remain live. */
function sparse() {
  const f = motionPaintFixture(),
    root = f.chips[0]!;
  const view = {
    // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
    ...f.view,
    contentDOM: { contains: (node: unknown) => node === f.line },
  } as unknown as EditorView;
  return { f, root, view };
}
for (const phantomFirst of [true, false])
  test(`sparse complete paint admits exactly one positive native fragment with phantom first=${phantomFirst}`, () => {
    const { f, root, view } = sparse(),
      rect = root.getBoundingClientRect(),
      // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
      phantom = { ...rect, right: rect.left, width: 0 };
    root.getClientRects = () => (phantomFirst ? [phantom, rect] : [rect, phantom]);
    const plan = prepareSparseReorderGrab(view, [root as unknown as HTMLElement], 0, {
      x: 25,
      y: 25,
    });
    assert.ok(plan);
    assert.equal(Object.isFrozen(plan.copies[0]!.rect), true);
    assert.equal(Object.isFrozen(plan.clips[0]!.rect), true);
    for (const name of ['left', 'right', 'top', 'bottom', 'width', 'height']) {
      Object.defineProperty(rect, name, {
        get() {
          throw new Error('native-rectangle-read-after-preparation');
        },
      });
    }
    const paint = withReorderPaintMaterialization(
      view.dom.ownerDocument,
      (materialization) =>
        materializeReorderGrab(
          view,
          materialization,
          plan.geometry,
          plan.prepared,
          plan.copies,
          plan.clips,
        ),
      (paint) => paint?.node.remove(),
    );
    assert.ok(paint);
    assert.equal(paint.node.textContent, root.textContent);
    assert.equal(paint.geometry.grabX, 5);
    assert.equal(paint.geometry.grabY, 5);
    paint.node.remove();
    assert.equal(root.removed, false);
    assert.equal(f.frames.size, 0);
  });
for (const mode of ['two-positive', 'nonfinite-empty'] as const)
  test(`sparse ${mode} fragments refuse before allocating destination paint`, () => {
    const { f, root, view } = sparse(),
      rect = root.getBoundingClientRect();
    root.getClientRects = () => [
      rect,
      mode === 'two-positive'
        ? // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
          { ...rect, left: rect.left + 20, right: rect.right + 20 }
        : // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
          { ...rect, left: NaN, right: NaN, width: 0 },
    ];
    const allocations = f.allocations;
    assert.equal(
      prepareSparseReorderGrab(view, [root as unknown as HTMLElement], 0, {
        x: 25,
        y: 25,
      }),
      null,
    );
    assert.equal(f.allocations, allocations);
    assert.equal(root.removed, false);
  });
