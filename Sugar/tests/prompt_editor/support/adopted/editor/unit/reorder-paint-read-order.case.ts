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
import { captureNativePaint } from './reorder-native-paint-fixture.ts';
import { withReorderPaintMaterialization } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-materialization.ts';
/** Whole native snapshots resolve original CSS before allocating any destination paint. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { captureReorderMotionGrab } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-scene.ts';
import { reorderGrabGeometry } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-geometry.ts';
import {
  admitNativeReorderPaint,
  copyNativeReorderPaint,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-paint.ts';
import { prepareNativeReorderPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-preparation.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

for (const capture of ['native-paint', 'paused-grab'] as const) {
  test(`complete ${capture} resolves every original style before destination allocation`, () => {
    const f = motionPaintFixture();
    const scene = capture === 'paused-grab' ? captureNativePaint(f) : null;
    if (capture === 'paused-grab') {
      assert.ok(scene);
      assert.ok(f.document.body);
      f.document.body.append(scene as unknown as typeof f.line);
    }
    const baseline = f.allocations,
      original = f.document.defaultView.getComputedStyle;
    let reads = 0;
    f.document.defaultView.getComputedStyle = (node) => {
      assert.equal(f.allocations, baseline, 'computed-style-after-destination-allocation');
      const style = original(node);
      return {
        ...style,
        getPropertyValue(name: string) {
          assert.equal(f.allocations, baseline, 'computed-property-after-destination-allocation');
          ++reads;
          return style.getPropertyValue(name);
        },
      };
    };
    let captured: { readonly node: HTMLElement } | null = null;
    try {
      if (scene) {
        const geometry = reorderGrabGeometry(
          [
            {
              kind: 'chip',
              index: 1,
              left: 60,
              right: 80,
              top: 20,
              bottom: 40,
            },
          ],
          1,
          { x: 65, y: 25 },
        );
        assert.ok(geometry);
        captured = captureReorderMotionGrab(f.view, { node: scene }, geometry);
      } else
        captured = (() => {
          const node = captureNativePaint(f);
          return node ? { node } : null;
        })();
      assert.ok(captured);
      assert.ok(reads >= 144);
      assert.ok(f.allocations > baseline);
    } finally {
      captured?.node.remove();
      scene?.remove();
    }
  });
}

/** Immutable prepared output remains the sole materialization input after all original getters become unavailable. */
test('prepared whole paint reuses literal styles, descendants and copy identities without original field reads', () => {
  const f = motionPaintFixture(),
    admitted = admitNativeReorderPaint([f.line as unknown as HTMLElement]);
  assert.ok(admitted);
  const prepared = prepareNativeReorderPaint(f.view, [f.line as unknown as Node], admitted, 'copy');
  assert.ok(prepared);
  assert.equal(f.allocations, 0);
  const styleReads = f.styleReads,
    firstChip = f.chips[0]!;
  f.document.defaultView.getComputedStyle = () => {
    throw new Error('live-style-read-after-preparation');
  };
  for (const node of admitted) {
    for (const field of [
      'nodeType',
      'childNodes',
      'textContent',
      'className',
      'namespaceURI',
      'localName',
    ]) {
      Object.defineProperty(node, field, {
        get() {
          throw new Error('original-field-read-after-preparation');
        },
      });
    }
    Object.defineProperty(node, 'getAttribute', {
      value() {
        throw new Error('original-attribute-read-after-preparation');
      },
    });
  }
  const copies = new WeakMap<Node, Node>(),
    copy = withReorderPaintMaterialization(
      f.view.dom.ownerDocument,
      (materialization) =>
        copyNativeReorderPaint(
          f.view,
          f.line as unknown as Node,
          prepared,
          materialization,
          copies,
        ),
      // The copied root is the fixture's literal element transport, rather than an arbitrary DOM Node.
      (output) => (output as unknown as typeof f.line | null)?.remove(),
    ) as unknown as typeof f.line;
  assert.ok(copy);
  assert.equal(copy.textContent, 'abc');
  assert.equal(copy.style.font, '14px serif');
  assert.equal(copy.style['pointer-events'], 'none');
  assert.equal(copy.contentEditable, 'inherit');
  assert.equal(copies.get(f.line as unknown as Node), copy);
  assert.equal(copies.get(firstChip as unknown as Node), copy.childNodes[0]);
  assert.equal(f.styleReads, styleReads);
  copy.remove();
});

/** Failed preparation retains its raw cause and never creates partially prepared destination resources. */
test('style read failure before materialization allocates no destination', () => {
  const f = motionPaintFixture(),
    cause = new Error('controlled-preparation-style-read');
  f.document.defaultView.getComputedStyle = () => {
    throw cause;
  };
  assert.throws(
    () => captureNativePaint(f),
    (error) => error === cause,
  );
  assert.equal(f.allocations, 0);
});

/** A raw detached snapshot is not a mounted paused Scene and cannot authorize a new grab. */
test('paused grab refuses an unmounted native snapshot without destination allocation', () => {
  const f = motionPaintFixture(),
    failures: unknown[] = [];
  let scene: HTMLElement | null = null;
  try {
    scene = captureNativePaint(f);
    assert.ok(scene);
    assert.equal(scene.isConnected, false);
    const baseline = f.allocations,
      geometry = reorderGrabGeometry(
        [{ kind: 'chip', index: 1, left: 60, right: 80, top: 20, bottom: 40 }],
        1,
        { x: 65, y: 25 },
      );
    assert.ok(geometry);
    assert.equal(captureReorderMotionGrab(f.view, { node: scene }, geometry), null);
    assert.equal(f.allocations, baseline);
    assert.equal(f.line.textContent, 'abc');
  } catch (cause) {
    failures.push(cause);
  }
  try {
    scene?.remove();
  } catch (cause) {
    failures.push(cause);
  }
  try {
    f.destroyLayer();
  } catch (cause) {
    failures.push(cause);
  }
  if (failures.length === 1) throw failures[0];
  if (failures.length) throw new AggregateError(failures, 'detached-grab-proof-cleanup');
});
