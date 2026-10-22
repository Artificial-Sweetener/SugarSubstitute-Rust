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
/** Shared native copy materialization preserves physical outline clipping under independent Stage axes and default viewport placement. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { atomicPaintFixture } from './reorder-atomic-paint-fixture.ts';
import { admitNativeReorderPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-paint.ts';
import { prepareNativeReorderPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-preparation.ts';
import { materializeReorderGrab } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-materialization.ts';
import { reorderGrabGeometry } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-geometry.ts';

for (const kind of ['stage', 'viewport'] as const)
  test(`${kind} complete native copy uses independent literal clip origins, anisotropic scales and nonzero outline offsets`, () => {
    const f = atomicPaintFixture();
    let paint: ReturnType<typeof materializeReorderGrab> = null;
    try {
      const node = f.roots[0]! as unknown as HTMLElement;
      const nodes = admitNativeReorderPaint([node]);
      assert.ok(nodes);
      const prepared = prepareNativeReorderPaint(f.view, [node], nodes, 'atomic-grab');
      assert.ok(prepared);
      const geometry = reorderGrabGeometry(
        [
          {
            kind: 'chip',
            index: 0,
            left: 30,
            right: 130,
            top: 40,
            bottom: 100,
          },
        ],
        0,
        { x: 40, y: 50 },
      );
      assert.ok(geometry);
      paint = withReorderPaintMaterialization(
        f.view.dom.ownerDocument,
        (materialization) =>
          materializeReorderGrab(
            f.view,
            materialization,
            geometry,
            prepared,
            [
              {
                node,
                kind: 'fragment',
                rect: { left: 34, top: 46 },
                width: 20,
                height: 12,
                scaleX: 4,
                scaleY: 1.5,
              },
            ],
            [
              {
                rect: { left: 36, right: 114, top: 49, bottom: 88 },
                outlineX: 4,
                outlineY: 6,
              },
            ],
            kind === 'stage'
              ? { kind: 'stage', left: 10, top: 20, scaleX: 2, scaleY: 3 }
              : { kind: 'viewport' },
          ),
        (paint) => paint?.node.remove(),
      );
      assert.ok(paint);
      const proxy = paint.node,
        copy = proxy.firstElementChild as HTMLElement;
      assert.equal(
        proxy.className,
        kind === 'stage' ? 'sp-reorder-motion-peer' : 'sp-reorder-proxy',
      );
      assert.equal(proxy.style.position, kind === 'stage' ? 'absolute' : 'fixed');
      assert.equal(
        proxy.style.clipPath,
        kind === 'stage' ? 'path("M1 1h43v17h-43Z")' : 'path("M2 3h86v51h-86Z")',
      );
      assert.equal(proxy.style.width, kind === 'stage' ? '50px' : '100px');
      assert.equal(proxy.style.height, kind === 'stage' ? '20px' : '60px');
      assert.equal(copy.style.left, kind === 'stage' ? '2px' : '4px');
      assert.equal(copy.style.top, kind === 'stage' ? '2px' : '6px');
      assert.equal(copy.style.transform, kind === 'stage' ? 'scale(2, 0.5)' : 'scale(4, 1.5)');
      if (kind === 'stage') {
        assert.equal(proxy.style.left, '10px');
        assert.equal(proxy.style.top, '6.666666666666667px');
      }
      assert.equal(proxy.querySelectorAll('canvas').length, 1);
      assert.equal(proxy.querySelectorAll('.sp-lora-weight').length, 1);
      assert.equal(f.draws.length, 1);
      assert.equal(f.draws[0]!.source, f.canvases[0]);
    } catch (cause) {
      f.causes.push(cause);
    }
    try {
      paint?.node.remove();
    } catch (cause) {
      f.causes.push(cause);
    }
    f.destroy();
  });
