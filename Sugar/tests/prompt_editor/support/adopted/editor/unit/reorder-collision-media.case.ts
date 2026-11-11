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
/** Actual canonical collision output avoids invisible raster copies; controlled DOM/geometry does not certify browser glyphs or acquisition latency. */
import assert from 'node:assert/strict';
import { sceneErrors } from '../../../../../../frontend/sugarprompt/editor/src/scene-errors.ts';
import test from 'node:test';
import { Decoration, type EditorView } from '@codemirror/view';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { hideReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  SourceWidget,
  SourceToken,
} from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import type { ReadonlyWidgetMedia } from '../../../../../../frontend/sugarprompt/editor/src/readonly-widget-media.ts';
import type { LoraRaster } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-contracts.ts';
import { ReorderCollisionBase } from '../../../../../../frontend/sugarprompt/editor/src/reorder-collision-base.ts';
import { currentReorderPaintTargets } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { captureReorder } from '../transaction/reorder-fixture.ts';
import { fixture } from './reorder-session-fixture.ts';
import { collisionFixture } from './reorder-collision-media-fixture.ts';

/** Inspect the actual connected production output, rather than reading private resource fields. */
function collisionRoot(f: ReturnType<typeof collisionFixture>): HTMLElement {
  const root = Array.from(f.f.view.dom.ownerDocument.body.childNodes).find(
    (node) =>
      node.nodeType === 1 &&
      (node as HTMLElement).className.includes('sp-reorder-base-measurement'),
  ) as HTMLElement | undefined;
  assert.ok(root);
  return root;
}

/** Genuine calibration and physical-slot observation are separate production read/write/read boundaries. */
function measure(
  f: ReturnType<typeof collisionFixture>,
  context: NonNullable<ReturnType<ReorderCollisionBase['contextAt']>>,
) {
  const calibration = f.base.measure(f.input, context);
  assert.equal(calibration.kind, 'calibration');
  if (calibration.kind !== 'calibration') assert.fail('collision-calibration-required');
  assert.equal(f.base.align(calibration), true);
  const ready = f.base.measure(f.input, context);
  assert.equal(ready.kind, 'ready');
  if (ready.kind !== 'ready') assert.fail('collision-physical-output-required');
  assert.equal(f.base.current(f.input, context), ready.rects);
  assert.ok(f.base.placements(f.input, context));
  return ready.rects;
}

test('actual hidden45-ready canonical LoRAs retain chrome and physical output without any new raster dimension/paint/canvas work', () => {
  const f = collisionFixture();
  try {
    const source = readExactSource(f.f.state),
      selection = f.f.state.selection,
      reads = f.reads,
      liveNodes = f.live.querySelectorAll('.sp-lora-media'),
      targets = currentReorderPaintTargets(f.f.view);
    assert.equal(liveNodes.length, 45);
    assert.equal(new Set(f.borrowed.map((value) => value.canvas)).size, 45);
    assert.ok(f.borrowed.every((value) => value.raster === f.raster));
    assert.equal(f.base.prepare(f.input), true);
    const context = f.base.contextAt(f.input);
    assert.ok(context);
    f.base.mount(f.input, context);
    const root = collisionRoot(f);
    assert.equal(root.style.opacity, '0');
    assert.equal(root.contentEditable, 'false');
    assert.equal(root.querySelectorAll('.sp-lora-chip').length, 90);
    assert.equal(root.querySelectorAll('[data-media="ready"]').length, 45);
    assert.equal(root.querySelectorAll('.sp-lora-backing').length, 90);
    assert.equal(root.querySelectorAll('.sp-lora-media').length, 0);
    assert.equal(root.querySelectorAll('[data-reorder-chip]').length, 179);
    const rects = measure(f, context),
      descriptors = root.querySelectorAll('.sp-lora-backing');
    assert.ok(rects.some((rect) => rect.kind === 'chip'));
    assert.equal(f.reads, reads);
    assert.equal(f.borrowed.length, 45);
    assert.equal(f.canvases.length, 45);
    f.base.mount(f.input, context);
    assert.equal(collisionRoot(f), root);
    assert.equal(f.base.measure(f.input, context).kind, 'ready');
    assert.deepEqual(root.querySelectorAll('.sp-lora-backing'), descriptors);
    const line = f.f.view.domAtPos(0).node as HTMLElement,
      nativeRect = line.getBoundingClientRect.bind(line);
    // Native containing-line movement renews placement while retaining the actual collision chrome and never-hydrated media owner.
    line.getBoundingClientRect = () => {
      const box = nativeRect();
      return {
        // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
        ...box,
        left: box.left + 15,
        right: box.right + 15,
        top: box.top + 20,
        bottom: box.bottom + 20,
      } as DOMRect;
    };
    const moved = f.base.contextAt(f.input);
    assert.ok(moved);
    assert.equal(moved.paintKey, context.paintKey);
    assert.notEqual(moved.key, context.key);
    f.base.mount(f.input, moved);
    measure(f, moved);
    assert.equal(collisionRoot(f), root);
    assert.deepEqual(root.querySelectorAll('.sp-lora-backing'), descriptors);
    assert.equal(f.reads, reads);
    assert.equal(f.canvases.length, 45);
    assert.equal(f.borrowed.length, 45);
    assert.equal(currentReorderPaintTargets(f.f.view), targets);
    assert.deepEqual(f.live.querySelectorAll('.sp-lora-media'), liveNodes);
    assert.equal(f.reads, reads);
    assert.equal(f.disposals, 0);
    f.base.destroy();
    assert.equal(root.isConnected, false);
    assert.equal(readExactSource(f.f.state), source);
    assert.equal(f.f.state.selection, selection);
    assert.equal(undoDepth(f.f.state), 0);
    assert.equal(redoDepth(f.f.state), 0);
  } finally {
    f.destroy();
  }
});

for (const cleanupFails of [false, true])
  test(`failed canonical readonly construction retires deferred media and preserves raw${cleanupFails ? ' plus cleanup' : ''} causes`, () => {
    const f = fixture(),
      base = new ReorderCollisionBase(f.view),
      primary = new Error('controlled-collision-render'),
      cleanup = new Error('controlled-collision-remove');
    let owner: ReadonlyWidgetMedia | null = null,
      backing: HTMLElement | null = null,
      root: HTMLElement | null = null,
      removals = 0;
    const raster: LoraRaster = {
      width: 256,
      height: 256,
      /** Geometry-only collision registration never paints prepared pixels. */
      paint() {
        assert.fail('collision-probe-raster-paint');
      },
      /** Borrowed pixels never transfer disposal authority to the collision base. */
      dispose() {
        assert.fail('collision-probe-raster-disposal');
      },
    };
    /** This external canonical widget observes the actual readonly media argument and fails after registering a borrowed raster. */
    class FailingPaint extends SourceWidget {
      /** Eager fallback would violate the requested geometry-only constructor path. */
      override toDOM(_view: EditorView): HTMLElement {
        assert.fail('collision-eager-widget-path');
      }
      /** Registration precedes the raw external rendering failure and must be retired even before transfer. */
      override toReadonlyDOM(view: EditorView, media: ReadonlyWidgetMedia): HTMLElement {
        owner = media;
        backing = view.dom.ownerDocument.createElement('span');
        media.defer(backing, raster);
        throw primary;
      }
    }
    const capture = captureReorder(f.state),
      canonical = Decoration.set([
        Decoration.replace({
          widget: new FailingPaint(new SourceToken()),
        }).range(3, 4),
      ]),
      captured = {
        ...capture,
        projection: {
          canonical,
          visible: canonical,
          decoded: canonical,
          ordinary: canonical,
          sceneErrors: f.state.field(sceneErrors),
        },
      },
      order = capture.prepared.initial,
      hidden = hideReorderChip(captured.prepared, order, 0);
    assert.ok(hidden);
    const input = {
        token: {},
        capture: captured,
        order,
        hidden,
        held: 0,
        pointer: 7,
        coordinates: { x: 25, y: 20 },
      },
      document = f.view.dom.ownerDocument,
      create = document.createElement.bind(document);
    document.createElement = ((name: string) => {
      const node = create(name),
        remove = node.remove.bind(node);
      node.remove = () => {
        remove();
        if (!node.className.includes('sp-reorder-base-measurement')) return;
        root = node;
        removals++;
        base.destroy();
        if (cleanupFails) throw cleanup;
      };
      return node;
    }) as typeof create;
    try {
      const selection = f.state.selection;
      assert.equal(base.prepare(input), true);
      const context = base.contextAt(input);
      assert.ok(context);
      assert.throws(
        () => base.mount(input, context),
        (cause) =>
          cleanupFails
            ? cause instanceof AggregateError &&
              cause.errors[0] === primary &&
              cause.errors[1] === cleanup
            : cause === primary,
      );
      assert.ok(owner);
      assert.ok(backing);
      assert.ok(root);
      const retired: ReadonlyWidgetMedia = owner,
        parent: HTMLElement = backing,
        removed: HTMLElement = root;
      assert.throws(() => retired.defer(parent, raster), {
        message: 'readonly-media-registration-retired',
      });
      assert.equal(removed.isConnected, false);
      assert.equal(removals, 1);
      assert.equal(base.current(input, context), null);
      base.destroy();
      assert.equal(removals, 1);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
      assert.equal(redoDepth(f.state), 0);
    } finally {
      base.destroy();
      f.destroy();
    }
  });

test('genuine middle CSV-cell collision shares never-hydrated media with both canonical same-line edges', () => {
  const f = collisionFixture(true);
  try {
    const source = readExactSource(f.f.state),
      selection = f.f.state.selection;
    assert.equal(f.base.prepare(f.input), true);
    const context = f.base.contextAt(f.input);
    assert.ok(context);
    assert.ok(context.prefix.to > context.prefix.from);
    assert.ok(context.suffix.to > context.suffix.from);
    f.base.mount(f.input, context);
    const root = collisionRoot(f),
      line = root.childNodes[0]!.childNodes[0]!.childNodes[0] as HTMLElement;
    const prefix = line.childNodes[0] as HTMLElement,
      suffix = line.childNodes[2] as HTMLElement;
    assert.equal(prefix.querySelectorAll('.sp-lora-chip').length, 1);
    assert.equal(suffix.querySelectorAll('.sp-lora-chip').length, 1);
    assert.equal(prefix.querySelectorAll('[data-media="ready"]').length, 1);
    assert.equal(suffix.querySelectorAll('[data-media="ready"]').length, 1);
    assert.equal(root.querySelectorAll('.sp-lora-media').length, 0);
    assert.equal(f.canvases.length, 0);
    assert.equal(f.borrowed.length, 0);
    assert.equal(f.reads, 0);
    measure(f, context);
    f.base.destroy();
    assert.equal(root.isConnected, false);
    assert.equal(f.disposals, 0);
    assert.equal(readExactSource(f.f.state), source);
    assert.equal(f.f.state.selection, selection);
    assert.equal(undoDepth(f.f.state), 0);
    assert.equal(redoDepth(f.f.state), 0);
  } finally {
    f.destroy();
  }
});
