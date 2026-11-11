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
/** Complete held-tree fallback preserves descendant overflow while retaining native wrapping and resource bounds. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { EditorView } from '@codemirror/view';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
import { prepareSparseReorderGrab } from '../../../../../../frontend/sugarprompt/editor/src/reorder-sparse-grab-paint.ts';
import { materializeReorderGrab } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-materialization.ts';
import { captureReorderGrabPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-paint.ts';
/** Actual original root/line methods remain the finite independently controlled native geometry boundary. */
function fixture() {
  const f = motionPaintFixture(),
    root = f.chips[0]!;
  const view = {
    // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
    ...f.view,
    contentDOM: {
      querySelectorAll: (selector: string) => (selector === '.cm-line' ? [f.line] : [root]),
      contains: (node: unknown) => node === f.line,
    },
  } as unknown as EditorView;
  return { f, root, view, press: { x: 25, y: 25 } };
}
test('offscreen peer line overflow retains complete held paint and reads every selected style before allocating', () => {
  const { f, root, view, press } = fixture();
  f.line.append(f.text('x'.repeat(16385)));
  const before = f.allocations,
    read = f.document.defaultView.getComputedStyle;
  f.document.defaultView.getComputedStyle = (node) => {
    assert.equal(f.allocations, before);
    return read(node);
  };
  const paint = captureReorderGrabPaint(view, 0, press);
  assert.ok(paint);
  assert.equal(paint.geometry.grabX, 5);
  assert.equal(paint.geometry.grabY, 5);
  assert.equal(paint.node.textContent, root.textContent);
  assert.equal(paint.node.childNodes.length, 1);
  assert.equal(root.removed, false);
  paint.node.remove();
  assert.equal(f.frames.size, 0);
});
for (const mode of ['wrapped', 'node-budget', 'text-budget'] as const)
  test(`unsupported ${mode} selected paint refuses before any destination allocation`, () => {
    const { f, root, view, press } = fixture();
    if (mode === 'wrapped') {
      const rect = root.getBoundingClientRect();
      // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
      root.getClientRects = () => [rect, { ...rect, left: rect.left + 20, right: rect.right + 20 }];
    } else if (mode === 'node-budget')
      for (let at = 0; at < 1024; ++at) root.append(f.document.createElement('span'));
    else root.append(f.text('x'.repeat(16385)));
    const allocations = f.allocations;
    assert.equal(prepareSparseReorderGrab(view, [root as unknown as HTMLElement], 0, press), null);
    assert.equal(f.allocations, allocations);
  });
for (const overhang of [1 / 65536, 1 / 4])
  test(`held descendant overhang ${overhang} retains exact copy origin and complete clipping geometry`, () => {
    const { f, root, view, press } = fixture();
    const child = f.document.createElement('span');
    Object.assign(child.box, root.box, {
      top: root.box.top - overhang,
      width: root.box.width + overhang,
    });
    root.append(child);
    const plan = prepareSparseReorderGrab(view, [root as unknown as HTMLElement], 0, press);
    assert.ok(plan);
    const original = root.getBoundingClientRect();
    assert.deepEqual(plan.copies[0]!.rect, {
      left: original.left,
      top: original.top,
    });
    assert.deepEqual(plan.clips[0]!.rect, {
      left: original.left,
      right: original.right + overhang,
      top: original.top - overhang,
      bottom: original.bottom,
    });
    assert.equal(plan.geometry.top, original.top - overhang);
    assert.equal(plan.geometry.width, original.width + overhang);
    assert.equal(root.removed, false);
  });
for (const failure of [null, undefined, new Error('controlled-sparse-copy-pixels')])
  test(`sparse media draw raw ${String(failure)} remains primary before proxy cleanup`, () => {
    const { f, root, view, press } = fixture(),
      create = f.document.createElement.bind(f.document),
      cleanup = new Error('controlled-sparse-root-removal');
    const canvas = create('canvas');
    Object.assign(canvas, { width: 3, height: 4 });
    Object.assign(canvas.box, root.box);
    root.append(canvas);
    f.document.createElement = (tag) => {
      const node = create(tag);
      if (tag === 'canvas')
        Object.assign(node, {
          getContext: () => ({
            drawImage() {
              // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
              throw failure;
            },
          }),
        });
      const remove = node.remove.bind(node);
      node.remove = () => {
        remove();
        if (node.className === 'sp-reorder-proxy') throw cleanup;
      };
      return node;
    };
    assert.throws(
      () => {
        const plan = prepareSparseReorderGrab(view, [root as unknown as HTMLElement], 0, press);
        assert.ok(plan);
        withReorderPaintMaterialization(
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
      },
      (cause) =>
        cause instanceof AggregateError &&
        cause.errors[0] === failure &&
        cause.errors[1] === cleanup,
    );
    assert.equal(root.removed, false);
    assert.equal(f.frames.size, 0);
  });

test('unrelated unavailable peer media does not veto valid complete held paint', () => {
  const { f, root, view, press } = fixture(),
    peer = f.document.createElement('img');
  Object.assign(peer, { complete: false, naturalWidth: 0, naturalHeight: 0 });
  f.line.append(peer);
  const paint = captureReorderGrabPaint(view, 0, press);
  assert.ok(paint);
  assert.equal(paint.node.textContent, root.textContent);
  assert.equal(paint.node.childNodes.length, 1);
  assert.equal(paint.geometry.grabX, 5);
  assert.equal(paint.geometry.grabY, 5);
  paint.node.remove();
  assert.equal(root.removed, false);
});
for (const invalid of ['unavailable', 'invalid-dimension'] as const)
  test(`held ${invalid} media still rejects before any style read or destination allocation`, () => {
    const { f, root, view, press } = fixture(),
      media = f.document.createElement('img');
    Object.assign(media, {
      complete: invalid !== 'unavailable',
      naturalWidth: invalid === 'unavailable' ? 0 : NaN,
      naturalHeight: 4,
    });
    root.append(media);
    const allocations = f.allocations;
    f.document.defaultView.getComputedStyle = () => {
      throw new Error('held-media-refusal-must-precede-style-read');
    };
    assert.equal(captureReorderGrabPaint(view, 0, press), null);
    assert.equal(f.allocations, allocations);
    assert.equal(root.removed, false);
  });
