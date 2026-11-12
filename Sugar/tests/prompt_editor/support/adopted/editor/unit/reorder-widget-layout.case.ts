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
/** Genuine WidgetType callbacks retain canonical line paint and measured output without claiming browser flow proof. */
import assert from 'node:assert/strict';
import { sceneErrors } from '../../../../../../frontend/sugarprompt/editor/src/scene-errors.ts';
import test from 'node:test';
import { Transaction } from '@codemirror/state';
import { Decoration, type EditorView } from '@codemirror/view';
import { undoDepth } from '@codemirror/commands';
import {
  dropReorderChip,
  hideReorderChip,
  type ReorderState,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  installReorderPreview,
  reorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { measuredReorderWidget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { ReorderWidget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { ReorderMountedLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-layout.ts';
import { ReorderCollisionBase } from '../../../../../../frontend/sugarprompt/editor/src/reorder-collision-base.ts';
import {
  SourceWidget,
  SourceToken,
} from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import { paintFixture } from './reorder-marker-fixture.ts';
import { fixture, key, pointer, dragPointer } from './reorder-session-fixture.ts';
import { drain } from './reorder-marker-fixture.ts';

/** Inspect the actual external tree emitted by the production widget, without reconstructing its rendering rule. */
interface PaintedNode {
  readonly dataset: Readonly<Record<string, string>>;
  readonly childNodes: readonly PaintedNode[];
}

/** Only the shared rich widget's external mount boundary is observed; production cache and renderer retain its nested literal DOM. */
class NestedPaintWidget extends SourceWidget {
  mounted = 0;
  released = 0;
  /** Build an actual nested source-backed paint subtree for the canonical renderer to adopt. */
  override toDOM(view: EditorView): HTMLElement {
    this.mounted++;
    const root = view.dom.ownerDocument.createElement('span'),
      child = root.ownerDocument.createElement('span');
    child.textContent = 'b';
    root.append(child);
    return root;
  }
  /** The real line cache must retire this mounted shared widget exactly once. */
  override destroy(_dom: HTMLElement): void {
    this.released++;
  }
}

for (const detached of ['root', 'paint'] as const)
  for (const frontier of ['before-read', 'before-write', 'before-task'] as const)
    test(`a warmed pointer release refuses detached ${detached} collision geometry ${frontier} before renewing finite native phases`, () => {
      const f = fixture();
      try {
        f.owner.pointerdown(pointer());
        drain(f);
        dragPointer(f.owner, pointer());
        drain(f);
        const scene = Array.from(f.view.dom.ownerDocument.body.childNodes) as HTMLElement[],
          base = scene.find((node) => node.className.includes('sp-reorder-base-measurement'))!;
        assert.ok(base?.isConnected);
        const paint = base.childNodes[0]!.childNodes[0]!.childNodes[0]!
          .childNodes[1] as HTMLElement;
        if (frontier === 'before-read') (detached === 'root' ? base : paint).remove();
        f.owner.pointerup(pointer(1, 20, 55));
        const pending = f.jobs.shift()!,
          read = pending.read(f.view);
        assert.ok(read && typeof read === 'object');
        assert.equal(Reflect.get(read, 'needsBase'), frontier === 'before-read');
        assert.deepEqual(
          Reflect.get(read, 'target'),
          frontier === 'before-read' ? null : { kind: 'row', row: 0, insertion: 2 },
        );
        assert.equal(readExactSource(f.state), 'a, b, c');
        assert.equal(undoDepth(f.state), 0);
        if (frontier === 'before-write') (detached === 'root' ? base : paint).remove();
        pending.write!(read, f.view);
        if (frontier === 'before-task') (detached === 'root' ? base : paint).remove();
        if (frontier !== 'before-read') f.task();
        assert.equal(readExactSource(f.state), 'a, b, c');
        assert.equal(undoDepth(f.state), 0);
        drain(f);
        assert.equal(readExactSource(f.state), 'b, c, a');
        assert.equal(undoDepth(f.state), 1);
        assert.equal(base.isConnected, false);
        assert.equal(paint.isConnected, false);
        assert.equal(f.state.field(reorderPreview).value, null);
        assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
      } finally {
        f.destroy();
      }
    });

test('detached cold calibration refuses alignment and retires the actual pending pointer release', () => {
  const f = fixture();
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    dragPointer(f.owner, pointer());
    f.owner.pointerup(pointer(1, 20, 55));
    f.measure();
    f.task(); // Actual cold admission.
    f.measure();
    f.task(); // Target mount and deferred calibration request.
    const pending = f.jobs.shift()!,
      read = pending.read(f.view);
    assert.ok(read && typeof read === 'object');
    assert.equal(Reflect.get(read, 'kind'), 'calibration');
    (Reflect.get(read, 'root') as HTMLElement).remove();
    pending.write!(read, f.view);
    drain(f);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
  } finally {
    f.destroy();
  }
});

test('passive collision mounts retain live invisible-held proposal geometry and release distinct rich DOM leases', () => {
  const f = fixture();
  let live: ReorderWidget | null = null,
    root: HTMLElement | null = null;
  const base = new ReorderCollisionBase(f.view);
  const fonts = new EventTarget();
  Object.defineProperty(f.view.dom.ownerDocument, 'fonts', { value: fonts });
  try {
    f.owner.keydown(key('Alt'));
    f.measure();
    f.task();
    const admitted = f.state.field(reorderPreview).value!,
      rich = new NestedPaintWidget(new SourceToken()),
      sourceRange = admitted.output.renderedRanges[1]!,
      canonical = Decoration.set([
        Decoration.replace({ widget: rich }).range(sourceRange.from, sourceRange.to),
      ]),
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
      order = dropReorderChip(capture.prepared, hidden, admitted.activeChip, {
        kind: 'row',
        row: 0,
        insertion: 1,
      })!,
      preview = prepareReorderPreview({ ...admitted, capture, order }, 'pointer')!;
    live = new ReorderWidget(preview, null);
    root = live.toDOM(f.view);
    root.ownerDocument.body.append(root);
    const before = measuredReorderWidget(f.view, preview)!;
    assert.deepEqual(
      before.filter((rect) => rect.kind === 'chip').map((rect) => rect.index),
      [1, 0, 2],
    );
    const held = (
      root as unknown as {
        childNodes: { className: string; dataset: Record<string, string> }[];
      }
    ).childNodes.find((node) => node.dataset.reorderChip === '0')!;
    assert.ok(held.className.includes('sp-reorder-held'));
    const input = {
      token: admitted.token,
      capture,
      order,
      hidden,
      held: 0,
      pointer: 9,
      coordinates: { x: 55, y: 50 },
    };
    assert.equal(base.prepare(input), true);
    const context = base.contextAt(input)!;
    assert.ok(context);
    base.mount(input, context);
    const calibration = base.measure(input, context);
    assert.equal(calibration.kind, 'calibration');
    if (calibration.kind !== 'calibration') throw new Error('base-calibration-fixture');
    assert.equal(base.align(calibration), true);
    const measured = base.measure(input, context);
    assert.equal(measured.kind, 'ready');
    if (measured.kind !== 'ready') throw new Error('base-measure-fixture');
    const collision = measured.rects;
    assert.deepEqual(
      collision.filter((rect) => rect.kind === 'chip').map((rect) => rect.index),
      [1, 2],
    );
    assert.equal(base.current(input, context), collision);
    fonts.dispatchEvent(new Event('loadingdone'));
    assert.equal(base.current(input, context), null);
    const refreshed = base.measure(input, context);
    assert.equal(refreshed.kind, 'ready');
    if (refreshed.kind !== 'ready') throw new Error('base-font-measure-fixture');
    assert.equal(base.current(input, context), refreshed.rects);
    assert.deepEqual(refreshed.rects, collision);
    assert.deepEqual(base.target(input, context, 55, 20), {
      kind: 'row',
      row: 0,
      insertion: 2,
    });
    assert.equal(measuredReorderWidget(f.view, preview), before);
    assert.equal(rich.mounted, 2);
    const scene = root.ownerDocument.body.childNodes as unknown as HTMLElement[];
    const owned = scene.find((node) => node.className.includes('sp-reorder-base-measurement'))!;
    const passive = owned.childNodes[0]!.childNodes[0]!.childNodes[0]!.childNodes[1] as HTMLElement;
    passive.remove();
    assert.equal(owned.isConnected, true);
    assert.equal(passive.isConnected, false);
    assert.equal(base.current(input, context), null);
    assert.equal(base.target(input, context, 55, 20), null);
    assert.equal(base.measure(input, context).kind, 'unavailable');
    owned.remove();
    assert.equal(base.current(input, context), null);
    const paintCause = new Error('passive-rich-retirement'),
      rootCause = new Error('passive-root-retirement'),
      retire = rich.destroy.bind(rich),
      detach = owned.remove.bind(owned);
    rich.destroy = (node) => {
      retire(node);
      throw paintCause;
    };
    owned.remove = () => {
      throw rootCause;
    };
    try {
      assert.throws(
        () => base.destroy(),
        (cause) =>
          cause instanceof AggregateError &&
          cause.errors.length === 2 &&
          cause.errors[0] === rootCause &&
          cause.errors[1] === paintCause,
      );
    } finally {
      rich.destroy = retire;
      owned.remove = detach;
    }
    fonts.dispatchEvent(new Event('loadingdone'));
    assert.equal(base.measure(input, context).kind, 'unavailable');
    assert.equal(rich.released, 1);
    assert.equal(measuredReorderWidget(f.view, preview), before);
    live.destroy(root);
    live = null;
    assert.equal(rich.released, 2);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    base.destroy();
    if (live && root) live.destroy(root);
    root?.remove();
    f.destroy();
  }
});

test('native detach and revisit preserve every nested rich child and the original shared mount', () => {
  const { f } = paintFixture();
  try {
    f.owner.keydown(key('Alt'));
    f.measure();
    f.task();
    const admitted = f.state.field(reorderPreview).value!,
      widget = new NestedPaintWidget(new SourceToken());
    const sourceRange = admitted.output.renderedRanges[1]!,
      decoration = Decoration.set([
        Decoration.replace({ widget }).range(sourceRange.from, sourceRange.to),
      ]);
    const capture = {
      ...admitted.capture,
      projection: {
        canonical: decoration,
        visible: decoration,
        decoded: decoration,
        ordinary: decoration,
        sceneErrors: { sourceIdentity: null, keys: new Set<string>() },
      },
    };
    const hidden = hideReorderChip(capture.prepared, admitted.order, admitted.activeChip)!;
    const moved = dropReorderChip(capture.prepared, hidden, admitted.activeChip, {
      kind: 'row',
      row: 0,
      insertion: 1,
    })!;
    const prepare = (order: ReorderState, hiding: boolean) =>
      new ReorderWidget(
        prepareReorderPreview({ ...admitted, capture, order }, hiding ? 'collision' : 'complete')!,
        null,
      );
    const first = prepare(hidden, true),
      root = first.toDOM(f.view);
    root.ownerDocument.body.append(root);
    const chip = (root as unknown as PaintedNode).childNodes.find(
      (node) => node.dataset.reorderChip === '1',
    )!;
    const line = chip.childNodes[0]!,
      beforeHint = line.childNodes[0]!,
      rich = line.childNodes[1]!,
      afterHint = line.childNodes[2]!,
      literal = rich.childNodes[0]!;
    assert.equal((beforeHint as unknown as HTMLElement).localName, 'wbr');
    assert.equal((afterHint as unknown as HTMLElement).localName, 'wbr');
    assert.equal((literal as unknown as HTMLElement).textContent, 'b');
    const literalNode = literal as unknown as HTMLElement,
      nativeLayout = new ReorderMountedLayout(f.view),
      target = [
        {
          node: literalNode,
          kind: 'chip' as const,
          index: 1,
          leases: [],
          origins: [],
        },
      ];
    assert.equal(literalNode.isConnected, true);
    const measured = nativeLayout.measure(target);
    assert.ok(measured);
    (line as unknown as HTMLElement).remove();
    assert.equal(literalNode.isConnected, false);
    assert.deepEqual(line.childNodes, [beforeHint, rich, afterHint]);
    assert.deepEqual(rich.childNodes, [literal]);
    assert.equal(nativeLayout.measure(target), null);
    assert.equal(widget.mounted, 1);
    const next = prepare(moved, false);
    assert.equal(next.updateDOM(root, f.view), true);
    const retained = (root as unknown as PaintedNode).childNodes.find(
      (node) => node.dataset.reorderChip === '1',
    )!.childNodes[0]!;
    assert.equal(retained, line);
    assert.deepEqual(retained.childNodes, [beforeHint, rich, afterHint]);
    assert.deepEqual(rich.childNodes, [literal]);
    assert.equal((literal as unknown as HTMLElement).textContent, 'b');
    assert.equal((literal as unknown as { removed: boolean }).removed, false);
    assert.equal(literalNode.isConnected, true);
    assert.equal(nativeLayout.measure(target), measured);
    assert.equal(widget.mounted, 1);
    assert.equal(widget.released, 0);
    next.destroy(root);
    assert.equal(widget.released, 1);
    assert.deepEqual(rich.childNodes, [literal]);
    assert.equal((literal as unknown as { removed: boolean }).removed, true);
    assert.equal(literalNode.isConnected, false);
    assert.equal(nativeLayout.measure(target), null);
    nativeLayout.destroy();
    root.remove();
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('native target revisits retain the exact canonical paint nodes and context-current measured output', () => {
  const f = fixture();
  try {
    f.owner.keydown(key('Alt'));
    f.measure();
    f.task();
    const admitted = f.state.field(reorderPreview).value!;
    const before = f.state,
      source = readExactSource(before);
    const hidden = hideReorderChip(admitted.capture.prepared, admitted.order, admitted.activeChip)!;
    const moved = dropReorderChip(admitted.capture.prepared, hidden, admitted.activeChip, {
      kind: 'row',
      row: 0,
      insertion: 1,
    })!;
    assert.ok(hidden);
    assert.ok(moved);
    /** Publication uses the genuine captured input and canonical serializer; the native draw boundary owns DOM changes. */
    const publish = (order: ReorderState, hiding: boolean) => {
      const next = prepareReorderPreview(
        {
          token: admitted.token,
          capture: admitted.capture,
          activeChip: admitted.activeChip,
          order,
        },
        hiding ? 'collision' : 'complete',
      )!;
      assert.ok(next);
      f.view.dispatch({
        effects: installReorderPreview.of(next),
        annotations: Transaction.addToHistory.of(false),
      });
      f.draw();
      return next;
    };
    const first = publish(hidden, true),
      root = f.paintedRoot!;
    const chip = (root as unknown as PaintedNode).childNodes.find(
      (node) => node.dataset.reorderChip === '1',
    )!;
    const actualPaint = chip.childNodes[0]!;
    const firstRects = measuredReorderWidget(f.view, first)!;
    assert.deepEqual(
      firstRects.map((rect) => rect.index),
      [1, 2],
    );
    assert.equal(measuredReorderWidget(f.view, first), firstRects);
    const placed = publish(moved, false);
    assert.equal(f.paintedRoot, root);
    assert.equal(
      (root as unknown as PaintedNode).childNodes.find((node) => node.dataset.reorderChip === '1')!
        .childNodes[0],
      actualPaint,
    );
    assert.equal(measuredReorderWidget(f.view, first), null);
    assert.deepEqual(
      measuredReorderWidget(f.view, placed)!.map((rect) => rect.index),
      [1, 0, 2],
    );
    const revisit = publish(hidden, true);
    assert.equal(f.paintedRoot, root);
    assert.equal(
      (root as unknown as PaintedNode).childNodes.find((node) => node.dataset.reorderChip === '1')!
        .childNodes[0],
      actualPaint,
    );
    const reused = measuredReorderWidget(f.view, revisit)!;
    assert.equal(reused, firstRects);
    assert.deepEqual(reused, firstRects);
    assert.equal(measuredReorderWidget(f.view, revisit), reused);
    assert.equal(f.state.doc, before.doc);
    assert.equal(f.state.selection.eq(before.selection), true);
    assert.equal(readExactSource(f.state), source);
    assert.equal(undoDepth(f.state), undoDepth(before));
    f.view.dispatch({
      effects: installReorderPreview.of(null),
      annotations: Transaction.addToHistory.of(false),
    });
    f.draw();
    assert.equal(measuredReorderWidget(f.view, revisit), null);
    assert.equal(f.paintedRoot, null);
  } finally {
    f.destroy();
  }
});
