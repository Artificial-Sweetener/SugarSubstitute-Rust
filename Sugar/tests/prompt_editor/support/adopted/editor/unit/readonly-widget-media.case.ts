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
/** Actual canonical LoRA paint and the finite readonly media owner are exercised through a literal external DOM transport. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { ReadonlyWidgetMedia } from '../../../../../../frontend/sugarprompt/editor/src/readonly-widget-media.ts';
import { LoraWidget } from '../../../../../../frontend/sugarprompt/editor/src/lora-widget.ts';
import { SourceToken } from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import type { LoraRaster } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-contracts.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

/** Prepared pixels are borrowed without acquisition; this fixture records actual canvas identities and disposal authority. */
function rasterFixture(width = 16, height = 8) {
  const painted: HTMLCanvasElement[] = [];
  let disposed = 0;
  const raster: LoraRaster = {
    width,
    height,
    /** Observe the exact canvas receiving already-prepared pixels. */
    paint(canvas) {
      painted.push(canvas);
    },
    /** The readonly owner must never retire another owner's prepared raster. */
    dispose() {
      disposed++;
    },
  };
  return { raster, painted, disposals: () => disposed };
}

/** Construct the normal canonical descriptor, with one immutable prepared raster shared across authored repeats. */
function widget(raster: LoraRaster): LoraWidget {
  return new LoraWidget(
    new SourceToken(),
    {
      name: 'fixture',
      resolution: 'found',
      model: {
        promptName: 'fixture',
        backendValue: 'fixture.safetensors',
        basename: 'fixture',
      },
      title: 'Prepared title',
      subtitle: null,
      triggerWords: [],
      thumbnail: { status: 'unavailable' },
      modelPageUrl: null,
    },
    '0.70',
    'none',
    { status: 'ready', raster },
  );
}

test('180 canonical readonly LoRAs preserve native chrome and hydrate only selected viewport groups from the exact prepared raster', () => {
  const f = motionPaintFixture(),
    media = new ReadonlyWidgetMedia(),
    raster = rasterFixture(),
    previous = Object.getOwnPropertyDescriptor(globalThis, 'document');
  Object.defineProperty(globalThis, 'document', {
    configurable: true,
    value: f.document,
  });
  try {
    const descriptor = widget(raster.raster),
      native = descriptor.toDOM(f.view);
    assert.equal(raster.painted.length, 1);
    const roots = Array.from({ length: 180 }, () => descriptor.toReadonlyDOM(f.view, media));
    for (const root of roots) {
      f.view.contentDOM.append(root);
      assert.equal(root.textContent, native.textContent);
      assert.equal(root.className, native.className);
      assert.equal(root.dataset.media, 'ready');
      assert.equal(root.childNodes[0]!.childNodes.length, 0);
    }
    assert.equal(raster.painted.length, 1);
    const visible = [roots[2]!, roots[87]!, roots[179]!],
      plan = media.prepare(visible);
    assert.ok(plan && !plan.empty);
    assert.equal(raster.painted.length, 1);
    assert.equal(plan.hydrate(), true);
    assert.equal(plan.hydrate(), true);
    assert.equal(raster.painted.length, 4);
    for (const root of roots)
      assert.equal(root.childNodes[0]!.childNodes.length, visible.includes(root) ? 1 : 0);
    const created = raster.painted.slice(1);
    assert.equal(new Set(created).size, 3);
    media.destroy();
    for (const canvas of created)
      assert.deepEqual([canvas.width, canvas.height, canvas.isConnected], [0, 0, false]);
    assert.equal(plan.hydrate(), false);
    assert.equal(raster.disposals(), 0);
  } finally {
    media.destroy();
    if (previous) Object.defineProperty(globalThis, 'document', previous);
    else Reflect.deleteProperty(globalThis, 'document');
  }
});

for (const sizes of [
  [1025, 1024, 1],
  [1024, 1024, 5],
  [0, 1, 1],
  [1.5, 2, 1],
] as const)
  test(`readonly media refuses invalid/oversized complete batches before allocating pixels: ${sizes.join('x')}`, () => {
    const f = motionPaintFixture(),
      media = new ReadonlyWidgetMedia(),
      raster = rasterFixture(sizes[0], sizes[1]);
    const roots = Array.from({ length: sizes[2] }, () =>
      f.view.dom.ownerDocument.createElement('span'),
    );
    for (const root of roots) {
      f.view.contentDOM.append(root);
      media.defer(root, raster.raster);
    }
    const allocations = f.allocations;
    assert.equal(media.prepare(roots), null);
    assert.equal(f.allocations, allocations);
    assert.equal(raster.painted.length, 0);
    media.destroy();
    assert.equal(raster.disposals(), 0);
  });

test('the exact 4MP boundary hydrates four 1MP rasters and retired or detached plans cannot allocate', () => {
  const f = motionPaintFixture(),
    media = new ReadonlyWidgetMedia(),
    raster = rasterFixture(1024, 1024);
  const roots = Array.from({ length: 4 }, () => f.view.dom.ownerDocument.createElement('span'));
  for (const root of roots) {
    f.view.contentDOM.append(root);
    media.defer(root, raster.raster);
  }
  const plan = media.prepare(roots);
  assert.ok(plan);
  assert.equal(plan.hydrate(), true);
  assert.equal(raster.painted.length, 4);
  media.destroy();
  assert.equal(plan.hydrate(), false);
  assert.equal(raster.disposals(), 0);
  const detached = new ReadonlyWidgetMedia(),
    root = f.view.dom.ownerDocument.createElement('span');
  detached.defer(root, raster.raster);
  const unavailable = detached.prepare([root]);
  assert.ok(unavailable);
  assert.equal(unavailable.hydrate(), false);
  assert.equal(raster.painted.length, 4);
  detached.destroy();
});

test('unexpected raster failures retain their original cause and release partial native buffers', () => {
  const f = motionPaintFixture(),
    media = new ReadonlyWidgetMedia(),
    cause = new Error('controlled-paint-failure');
  const root = f.view.dom.ownerDocument.createElement('span');
  f.view.contentDOM.append(root);
  let canvas: HTMLCanvasElement | null = null;
  media.defer(root, {
    width: 16,
    height: 8,
    /** The real materializer must preserve an unexpected external paint cause. */
    paint(value) {
      canvas = value;
      throw cause;
    },
    /** Borrowed prepared pixels have a separate lifetime owner. */
    dispose() {
      assert.fail('borrowed-raster-disposal');
    },
  });
  const plan = media.prepare([root]);
  assert.ok(plan);
  assert.throws(
    () => plan.hydrate(),
    (error) => error === cause,
  );
  media.destroy();
  assert.ok(canvas);
  const retained = canvas as HTMLCanvasElement;
  assert.deepEqual([retained.width, retained.height], [0, 0]);
});

test('source or resource lease invalidation inside raster paint stops the admitted batch before the next allocation', () => {
  const f = motionPaintFixture();
  let current = true,
    painted = 0;
  const media = new ReadonlyWidgetMedia(() => current),
    roots = Array.from({ length: 3 }, () => f.view.dom.ownerDocument.createElement('span'));
  const canvases: HTMLCanvasElement[] = [];
  for (const root of roots) {
    f.view.contentDOM.append(root);
    media.defer(root, {
      width: 16,
      height: 8,
      /** Synchronous host/profile supersession is observable before another canvas can allocate. */
      paint(canvas) {
        canvases.push(canvas);
        painted++;
        current = false;
      },
      /** This borrowed raster remains owned by the existing preparation pool. */
      dispose() {
        assert.fail('borrowed-raster-disposal');
      },
    });
  }
  const plan = media.prepare(roots);
  assert.ok(plan);
  const allocations = f.allocations;
  assert.equal(plan.hydrate(), false);
  assert.equal(painted, 1);
  assert.equal(f.allocations, allocations + 1);
  assert.ok(roots.every((root) => root.childNodes.length === 0));
  media.destroy();
  assert.deepEqual(
    canvases.map((canvas) => [canvas.width, canvas.height]),
    [[0, 0]],
  );
});

for (const frontier of ['create', 'className', 'width', 'height', 'attribute', 'append'] as const)
  test(`synchronous retirement at native ${frontier} cannot paint or retain an untransferred buffer`, () => {
    const f = motionPaintFixture(),
      media = new ReadonlyWidgetMedia(),
      raster = rasterFixture(),
      doc = f.view.dom.ownerDocument,
      root = doc.createElement('span');
    f.view.contentDOM.append(root);
    media.defer(root, raster.raster);
    const plan = media.prepare([root]);
    assert.ok(plan);
    const create = doc.createElement.bind(doc),
      canvases: HTMLCanvasElement[] = [];
    Reflect.set(doc, 'createElement', (tag: string) => {
      const element = create(tag);
      if (tag === 'canvas') {
        const canvas = element as HTMLCanvasElement;
        canvases.push(canvas);
        if (frontier === 'create') media.destroy();
        else if (frontier === 'className' || frontier === 'width' || frontier === 'height') {
          let value: string | number = frontier === 'className' ? '' : 0,
            entered = false;
          Object.defineProperty(canvas, frontier, {
            configurable: true,
            /** Literal property transport permits constructor/setter retirement during a native operation. */
            get: () => value,
            /** Finish the original native write after reentrant retirement to expose stale-buffer resurrection. */
            set(next: string | number) {
              if (!entered) {
                entered = true;
                media.destroy();
              }
              value = next;
            },
          });
        } else if (frontier === 'attribute') {
          const attribute = canvas.setAttribute.bind(canvas);
          canvas.setAttribute = (name, value) => {
            media.destroy();
            attribute(name, value);
          };
        } else {
          const append = root.append.bind(root);
          root.append = (...nodes) => {
            media.destroy();
            append(...nodes);
          };
        }
      }
      return element;
    });
    assert.equal(plan.hydrate(), false);
    assert.equal(canvases.length, 1);
    assert.equal(raster.painted.length, frontier === 'append' ? 1 : 0);
    for (const canvas of canvases)
      assert.deepEqual([canvas.width, canvas.height, canvas.isConnected], [0, 0, false]);
    assert.equal(plan.hydrate(), false);
    assert.equal(raster.disposals(), 0);
    media.destroy();
  });

test('admitted dimensions are frozen and an in-flight or failed hydration attempt cannot allocate twice', () => {
  const f = motionPaintFixture(),
    media = new ReadonlyWidgetMedia(),
    root = f.view.dom.ownerDocument.createElement('span');
  f.view.contentDOM.append(root);
  const canvases: HTMLCanvasElement[] = [];
  let widthReads = 0;
  let plan: ReturnType<ReadonlyWidgetMedia['prepare']> = null;
  media.defer(root, {
    /** Only READ admission may inspect borrowed intrinsic dimensions. */
    get width() {
      widthReads++;
      return widthReads === 1 ? 16 : 1048576;
    },
    height: 8,
    /** Reentrant hydration is rejected before touching a second canvas. */
    paint(canvas) {
      canvases.push(canvas);
      assert.ok(plan);
      assert.equal(plan.hydrate(), false);
    },
    /** Prepared raster ownership remains separate. */
    dispose() {
      assert.fail('borrowed-raster-disposal');
    },
  });
  plan = media.prepare([root]);
  assert.ok(plan);
  assert.equal(plan.hydrate(), true);
  assert.equal(plan.hydrate(), true);
  assert.equal(canvases.length, 1);
  assert.equal(widthReads, 1);
  assert.equal(canvases[0]!.width, 16);
  media.destroy();
});

test('width release failure cannot skip height release and the original paint cause remains first', () => {
  const f = motionPaintFixture(),
    media = new ReadonlyWidgetMedia(),
    root = f.view.dom.ownerDocument.createElement('span');
  f.view.contentDOM.append(root);
  const primary = new Error('controlled-primary'),
    cleanup = new Error('controlled-width-release');
  let heightReleases = 0;
  media.defer(root, {
    width: 16,
    height: 8,
    /** The external setter fails only during cleanup; the independent height attempt remains required. */
    paint(canvas) {
      Object.defineProperty(canvas, 'width', {
        set: () => {
          throw cleanup;
        },
      });
      Object.defineProperty(canvas, 'height', {
        set: () => {
          heightReleases++;
        },
      });
      throw primary;
    },
    /** No borrowed preparation lifetime is transferred. */
    dispose() {
      assert.fail('borrowed-raster-disposal');
    },
  });
  const plan = media.prepare([root]);
  assert.ok(plan);
  assert.throws(
    () => plan.hydrate(),
    (cause) =>
      cause instanceof AggregateError && cause.errors[0] === primary && cause.errors[1] === cleanup,
  );
  assert.equal(heightReleases, 1);
  assert.equal(plan.hydrate(), false);
  media.destroy();
});
