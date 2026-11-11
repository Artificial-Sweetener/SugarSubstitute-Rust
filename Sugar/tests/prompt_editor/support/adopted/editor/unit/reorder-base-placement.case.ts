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
/** Actual passive paint and controlled viewport reads prove placement reuse, calibration and finite lease retirement. */
import assert from 'node:assert/strict';
import { sceneErrors } from '../../../../../../frontend/sugarprompt/editor/src/scene-errors.ts';
import test from 'node:test';
import { Decoration, type EditorView } from '@codemirror/view';
import { hideReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  SourceToken,
  SourceWidget,
} from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import { ReorderCollisionBase } from '../../../../../../frontend/sugarprompt/editor/src/reorder-collision-base.ts';
import type { ReorderBaseContext } from '../../../../../../frontend/sugarprompt/editor/src/reorder-base-context.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { fixture, key } from './reorder-session-fixture.ts';

/** Independent rich output exposes the actual nested DOM and its finite canonical line lease. */
class PlacementPaint extends SourceWidget {
  mounted = 0;
  released = 0;
  /** The canonical owner adopts this real child tree without replacing the shared widget boundary. */
  override toDOM(view: EditorView): HTMLElement {
    this.mounted++;
    const node = view.dom.ownerDocument.createElement('span'),
      child = node.ownerDocument.createElement('span');
    child.textContent = 'b';
    node.append(child);
    return node;
  }
  /** Each actual resource retirement is independently observable. */
  override destroy(_node: HTMLElement): void {
    this.released++;
  }
}

/** Real production mounts consume a fixed captured operand; only native viewport reads are controlled. */
function placementFixture() {
  const f = fixture(),
    base = new ReorderCollisionBase(f.view),
    fonts = new EventTarget();
  let fontAdds = 0,
    fontRemoves = 0;
  const add = fonts.addEventListener.bind(fonts),
    remove = fonts.removeEventListener.bind(fonts);
  fonts.addEventListener = (...args) => {
    fontAdds++;
    add(...args);
  };
  fonts.removeEventListener = (...args) => {
    fontRemoves++;
    remove(...args);
  };
  Object.defineProperty(f.view.dom.ownerDocument, 'fonts', { value: fonts });
  f.owner.keydown(key('Alt'));
  f.measure();
  f.task();
  const admitted = f.state.field(reorderPreview).value!,
    rich = new PlacementPaint(new SourceToken()),
    range = admitted.output.renderedRanges[1]!,
    canonical = Decoration.set([Decoration.replace({ widget: rich }).range(range.from, range.to)]),
    capture = {
      ...admitted.capture,
      projection: {
        canonical,
        visible: canonical,
        decoded: canonical,
        ordinary: canonical,
        sceneErrors: f.state.field(sceneErrors),
      },
    },
    hidden = hideReorderChip(capture.prepared, admitted.order, admitted.activeChip)!,
    input = {
      token: admitted.token,
      capture,
      order: admitted.order,
      hidden,
      held: admitted.activeChip,
      pointer: 9,
      coordinates: { x: 55, y: 20 },
    };
  assert.equal(base.prepare(input), true);
  let left = 10,
    top = 10;
  /** This reposition specimen keeps every translated measured frame inside its independent native viewport. */
  f.view.scrollDOM.getBoundingClientRect = () =>
    ({
      left: 0,
      right: 200,
      top: -100,
      bottom: 200,
      width: 200,
      height: 300,
    }) as DOMRect;
  const line = f.view.domAtPos(0).node as HTMLElement;
  line.getBoundingClientRect = () =>
    ({
      left,
      right: left + 100,
      top,
      bottom: top + 20,
      width: 100,
      height: 20,
    }) as DOMRect;
  return {
    f,
    base,
    fonts,
    rich,
    input,
    /** The native font subscription transport exposes acquisition and retirement without retaining callbacks. */
    get fontSubscriptions() {
      return { acquired: fontAdds, released: fontRemoves };
    },
    /** Native containing-line position changes independently of computed typography and held collision output. */
    move(x: number, y: number): ReorderBaseContext {
      left = x;
      top = y;
      return base.contextAt(input)!;
    },
    /** Complete owned resources are retired even after a failed test assertion. */
    destroy(): void {
      base.destroy();
      f.destroy();
    },
  };
}

/** Observe the actual production scene tree; no private owner state or synthetic renderer is consulted. */
function scene(f: ReturnType<typeof placementFixture>): {
  root: HTMLElement;
  paint: HTMLElement;
  edges: readonly ChildNode[];
} {
  const root = Array.from(f.f.view.dom.ownerDocument.body.childNodes).find((node) =>
      (node as HTMLElement).className.includes('sp-reorder-base-measurement'),
    ) as HTMLElement,
    line = root.childNodes[0]!.childNodes[0]!.childNodes[0]!,
    paint = line.childNodes[1] as HTMLElement;
  return { root, paint, edges: [line.childNodes[0]!, line.childNodes[2]!] };
}

/** Finite genuine read/write/read calls return the canonical owner's complete actual rectangle output. */
function measure(f: ReturnType<typeof placementFixture>, context: ReorderBaseContext) {
  const read = f.base.measure(f.input, context);
  assert.equal(read.kind, 'calibration');
  if (read.kind !== 'calibration') throw new Error('placement-calibration-missing');
  assert.equal(f.base.align(read), true);
  const measured = f.base.measure(f.input, context);
  assert.equal(measured.kind, 'ready');
  if (measured.kind !== 'ready') throw new Error('placement-geometry-missing');
  assert.equal(f.base.current(f.input, context), measured.rects);
  return measured.rects;
}

test('forward and reverse placement retain the exact passive DOM, rich child and edge leases while remeasuring coordinates', () => {
  const f = placementFixture();
  try {
    const first = f.move(10, 10);
    f.base.mount(f.input, first);
    const initial = measure(f, first),
      retained = scene(f),
      child = retained.paint.childNodes[0]!.childNodes[0];
    assert.equal(f.rich.mounted, 1);
    for (const [left, top] of [
      [10, 90],
      [35, -20],
      [10, 10],
    ] as const) {
      const context = f.move(left, top);
      assert.equal(context.paintKey, first.paintKey);
      assert.equal(f.base.current(f.input, context), null);
      f.base.mount(f.input, context);
      const actual = scene(f);
      assert.equal(actual.root, retained.root);
      assert.equal(actual.paint, retained.paint);
      assert.deepEqual(actual.edges, retained.edges);
      assert.equal(actual.paint.childNodes[0]!.childNodes[0], child);
      assert.equal(f.rich.mounted, 1);
      assert.equal(f.rich.released, 0);
      assert.deepEqual(f.fontSubscriptions, { acquired: 2, released: 0 });
      const rects = measure(f, context);
      assert.deepEqual(
        rects,
        initial.map((rect) => ({
          ...rect,
          left: rect.left + left - 10,
          right: rect.right + left - 10,
          top: rect.top + top - 10,
          bottom: rect.bottom + top - 10,
        })),
      );
      assert.deepEqual(f.base.target(f.input, context, left + 45, top + 10), {
        kind: 'row',
        row: 0,
        insertion: 2,
      });
    }
    f.base.destroy();
    assert.equal(retained.root.isConnected, false);
    assert.equal(f.rich.released, 1);
    assert.deepEqual(f.fontSubscriptions, { acquired: 2, released: 2 });
    f.fonts.dispatchEvent(new Event('loadingdone'));
    assert.equal(f.base.measure(f.input, first).kind, 'unavailable');
  } finally {
    f.destroy();
  }
});
test('clipping-only native viewport renewal rejects old slots and reuses actual physical paint without another Range read', () => {
  const f = placementFixture();
  try {
    const first = f.move(10, 10);
    f.base.mount(f.input, first);
    const rects = measure(f, first),
      retained = scene(f),
      old = f.base.placements(f.input, first)!;
    f.f.view.scrollDOM.getBoundingClientRect = () =>
      ({
        left: 0,
        right: 200,
        top: -100,
        bottom: 15,
        width: 200,
        height: 115,
      }) as DOMRect;
    const fresh = f.move(10, 10);
    assert.equal(fresh.key, first.key);
    assert.equal(fresh.paintKey, first.paintKey);
    assert.equal(f.base.current(f.input, fresh), null);
    f.f.view.dom.ownerDocument.createRange = () => {
      throw new Error('clipping-reinspected-native-physical-output');
    };
    f.base.mount(f.input, fresh);
    assert.equal(scene(f).root, retained.root);
    assert.equal(scene(f).paint, retained.paint);
    const read = f.base.measure(f.input, fresh);
    assert.equal(read.kind, 'ready');
    if (read.kind !== 'ready') throw new Error('clipping-renewal-not-ready');
    assert.equal(read.rects, rects);
    const topology = f.base.placements(f.input, fresh)!;
    assert.notEqual(topology, old);
    assert.ok(topology.slots.every((slot) => slot.hit.bottom === 15));
    assert.equal(f.base.current(f.input, first), null);
    assert.equal(f.rich.mounted, 1);
    assert.equal(f.rich.released, 0);
    f.base.destroy();
    assert.equal(f.rich.released, 1);
    assert.equal(retained.root.isConnected, false);
  } finally {
    f.destroy();
  }
});
test('equal computed context cannot replace an invalid old native ancestor lease while retaining its scene', () => {
  const f = placementFixture();
  try {
    const line = f.f.view.domAtPos(0).node as HTMLElement,
      ancestor = () =>
        ({
          className: 'equal-parent',
          isConnected: true,
          parentElement: null,
          getAttribute: () => null,
          getRootNode: () => f.f.view.dom.ownerDocument,
        }) as unknown as HTMLElement;
    Object.defineProperty(line, 'parentElement', {
      value: ancestor(),
      configurable: true,
    });
    const first = f.move(10, 10);
    f.base.mount(f.input, first);
    measure(f, first);
    const retained = scene(f);
    Object.defineProperty(line, 'parentElement', { value: ancestor() });
    const fresh = f.move(10, 10);
    assert.equal(fresh.key, first.key);
    assert.equal(fresh.paintKey, first.paintKey);
    assert.equal(first.current(), false);
    assert.equal(fresh.current(), true);
    f.base.mount(f.input, fresh);
    assert.equal(scene(f).root === retained.root, false);
    assert.equal(retained.root.isConnected, false);
    assert.equal(f.rich.mounted, 2);
    assert.equal(f.rich.released, 1);
    measure(f, fresh);
    f.base.destroy();
    assert.equal(f.rich.released, 2);
  } finally {
    f.destroy();
  }
});

test('reposition calibration observes a moved and scaled external ancestor instead of translating cached coordinates', () => {
  const f = placementFixture();
  try {
    const first = f.move(10, 10);
    f.base.mount(f.input, first);
    measure(f, first);
    const retained = scene(f),
      context = f.move(80, 140);
    f.base.mount(f.input, context);
    let ancestorLeft = 40,
      ancestorTop = -30;
    retained.root.getBoundingClientRect = () =>
      ({
        left: ancestorLeft,
        top: ancestorTop,
        width: 80,
        height: 192,
      }) as DOMRect;
    const targets = Array.from(retained.paint.childNodes).filter(
      (node) => (node as HTMLElement).dataset.reorderChip !== undefined,
    ) as HTMLElement[];
    for (const [at, node] of targets.entries())
      node.getClientRects = () => {
        const left = ancestorLeft + Number.parseFloat(retained.root.style.left) * 0.8 + at * 30,
          top = ancestorTop + Number.parseFloat(retained.root.style.top) * 0.75;
        return [
          {
            left,
            right: left + 20,
            top,
            bottom: top + 20,
            width: 20,
            height: 20,
          },
        ] as unknown as DOMRectList;
      };
    const read = f.base.measure(f.input, context);
    assert.equal(read.kind, 'calibration');
    if (read.kind !== 'calibration') throw new Error('placement-scale-read-missing');
    assert.equal(read.left, 40);
    assert.equal(read.top, -30);
    assert.equal(read.scaleX, 0.8);
    assert.equal(read.scaleY, 0.75);
    assert.equal(f.base.align(read), true);
    assert.equal(retained.root.style.left, '50px');
    assert.equal(Number.parseFloat(retained.root.style.top), 170 / 0.75);
    assert.equal(
      retained.root.style.getPropertyValue?.('transform') ??
        Reflect.get(retained.root.style, 'transform'),
      'scale(1.25, 1.3333333333333333)',
    );
    const again = f.move(-10, 25);
    const ready = f.base.measure(f.input, context);
    assert.equal(ready.kind, 'ready');
    if (ready.kind !== 'ready') throw new Error('placement-scaled-geometry-missing');
    assert.deepEqual(ready.rects, [
      { kind: 'chip', index: 1, left: 80, right: 100, top: 140, bottom: 160 },
      { kind: 'chip', index: 2, left: 110, right: 130, top: 140, bottom: 160 },
    ]);
    f.base.mount(f.input, again);
    ancestorLeft = -20;
    ancestorTop = 50;
    const reverse = f.base.measure(f.input, again);
    assert.equal(reverse.kind, 'calibration');
    if (reverse.kind !== 'calibration') throw new Error('placement-reverse-read-missing');
    assert.equal(reverse.left, -20);
    assert.equal(reverse.top, 50);
    assert.equal(f.base.align(read), false);
    assert.equal(f.base.align(reverse), true);
    assert.equal(retained.root.style.left, '12.5px');
    assert.equal(Number.parseFloat(retained.root.style.top), -25 / 0.75);
    const reversed = f.base.measure(f.input, again);
    assert.equal(reversed.kind, 'ready');
    if (reversed.kind !== 'ready') throw new Error('placement-reversed-geometry-missing');
    assert.deepEqual(reversed.rects, [
      { kind: 'chip', index: 1, left: -10, right: 10, top: 25, bottom: 45 },
      { kind: 'chip', index: 2, left: 20, right: 40, top: 25, bottom: 45 },
    ]);
    assert.equal(scene(f).paint, retained.paint);
    assert.equal(f.rich.mounted, 1);
  } finally {
    f.destroy();
  }
});

for (const invalidation of ['font', 'shape', 'input', 'paint', 'root'] as const)
  test(`placement refuses retained ${invalidation} invalidation and recreates finite actual paint`, () => {
    const f = placementFixture();
    try {
      const first = f.move(10, 10);
      f.base.mount(f.input, first);
      measure(f, first);
      const retained = scene(f);
      let context = f.move(10, 50),
        input = f.input;
      if (invalidation === 'font') f.fonts.dispatchEvent(new Event('loadingdone'));
      if (invalidation === 'shape') context = { ...context, paintKey: 'changed-layout' };
      if (invalidation === 'input') input = { ...input };
      if (invalidation === 'paint') retained.paint.remove();
      if (invalidation === 'root') retained.root.remove();
      f.base.mount(input, context);
      const fresh = scene(f);
      assert.notEqual(fresh.root, retained.root);
      assert.notEqual(fresh.paint, retained.paint);
      assert.equal(retained.root.isConnected, false);
      assert.equal(f.rich.mounted, 2);
      assert.equal(f.rich.released, 1);
      f.base.destroy();
      assert.equal(fresh.root.isConnected, false);
      assert.equal(f.rich.released, 2);
    } finally {
      f.destroy();
    }
  });

test('placement write failure releases retained resources and keeps primary and cleanup cause order', () => {
  const f = placementFixture();
  try {
    const first = f.move(10, 10);
    f.base.mount(f.input, first);
    measure(f, first);
    const retained = scene(f),
      primary = new Error('placement-write-failed'),
      cleanup = new Error('placement-retirement-failed'),
      release = f.rich.destroy.bind(f.rich);
    retained.root.style.setProperty = () => {
      throw primary;
    };
    f.rich.destroy = (node) => {
      release(node);
      throw cleanup;
    };
    assert.throws(
      () => f.base.mount(f.input, f.move(10, 70)),
      (cause) =>
        cause instanceof AggregateError &&
        cause.errors[0] === primary &&
        cause.errors[1] === cleanup,
    );
    assert.equal(retained.root.isConnected, false);
    assert.equal(f.rich.released, 1);
    assert.equal(f.base.current(f.input, first), null);
    f.base.destroy();
    assert.equal(f.rich.released, 1);
  } finally {
    f.destroy();
  }
});
