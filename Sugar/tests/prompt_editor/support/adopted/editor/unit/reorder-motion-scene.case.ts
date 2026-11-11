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
/** Canonical motion uses the actual widget painter and native calibration; literal transport does not certify browser layout. */
import assert from 'node:assert/strict';
import type { ReorderMotionSelection } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-scene.ts';
/** This low-level geometry proof deliberately selects every observed peer; Presentation separately proves changed-only policy. */
const selectAll: ReorderMotionSelection = (geometry) =>
  Array.from(
    new Map(
      geometry.map((rect) => [
        rect.index,
        {
          id: rect.index,
          kind: 'peer' as const,
          from: { left: rect.left, top: rect.top },
          to: { left: rect.left, top: rect.top },
        },
      ]),
    ).values(),
  );
import test from 'node:test';
import { EditorSelection } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { ReadonlyWidgetMedia } from '../../../../../../frontend/sugarprompt/editor/src/readonly-widget-media.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

test('actual canonical paint calibrates once and reuses the same root/output across warm proposals and numeric frames', () => {
  const f = motionPresentationFixture();
  let stage: ReorderPassiveStage | null = null;
  try {
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared);
    stage = new ReorderPassiveStage(f.view, prepared);
    const child = stage.mount(prepared);
    assert.ok(child);
    f.paint.admitLayer(stage);
    const first = child.read(selectAll);
    assert.equal(first.kind, 'placement');
    if (first.kind !== 'placement') assert.fail('native placement unavailable');
    child.place(first);
    const window = f.view.dom.ownerDocument.defaultView;
    assert.ok(window);
    const getComputedStyle = window.getComputedStyle,
      selectedRoots = new Set<Element>(
        Array.from(child.node.querySelectorAll<HTMLElement>('[data-reorder-chip]')).filter(
          (node) => node.dataset.reorderChip !== '0',
        ),
      ),
      unnecessaryRead = new Error('unused-scene-root-style-read');
    let selected = false;
    /** Partial-scene native admissions precede selection; constructing its selected peer outputs requires no later root style read. */
    window.getComputedStyle = (node, pseudo) => {
      if (selected && selectedRoots.has(node)) throw unnecessaryRead;
      return getComputedStyle.call(window, node, pseudo);
    };
    let ready: ReturnType<typeof child.read>;
    try {
      ready = child.read((geometry) => {
        selected = true;
        return selectAll(geometry);
      });
      assert.equal(selected, true);
      assert.equal(selectedRoots.size, 2);
      const selectedRoot = selectedRoots.values().next().value;
      assert.ok(selectedRoot);
      assert.throws(
        () => window.getComputedStyle(selectedRoot),
        (cause) => cause === unnecessaryRead,
      );
    } finally {
      window.getComputedStyle = getComputedStyle;
    }
    assert.equal(ready.kind, 'ready');
    if (ready.kind !== 'ready') assert.fail('native output unavailable');
    child.reveal();
    const scene = ready.scene;
    assert.deepEqual(
      scene.geometry,
      [
        { kind: 'chip', index: 1, left: 20, right: 40, top: 20, bottom: 40 },
        { kind: 'chip', index: 2, left: 100, right: 120, top: 20, bottom: 40 },
      ].flatMap((rect) => [rect, rect]),
    );
    assert.deepEqual(scene.targets, [
      {
        id: 1,
        kind: 'peer',
        from: { left: 20, top: 20 },
        to: { left: 20, top: 20 },
      },
      {
        id: 2,
        kind: 'peer',
        from: { left: 100, top: 20 },
        to: { left: 100, top: 20 },
      },
    ]);
    assert.equal(stage.node.parentElement, f.paint.layerMount);
    assert.equal(f.view.contentDOM.contains(scene.node), false);
    assert.deepEqual(
      scene.chips.map((chip) => chip.id),
      [1, 2],
    );
    const root = scene.node,
      roots = scene.chips.map((chip) => chip.roots[0]!.node),
      reads = f.paint.styleReads,
      allocations = f.paint.allocations;
    scene.paint({
      progress: 0.5,
      positions: [
        { id: 1, left: 30, top: 50 },
        { id: 2, left: 60, top: 20 },
      ],
    });
    assert.equal(scene.node, root);
    assert.equal(scene.chips[0]!.roots[0]!.node, roots[0]);
    assert.equal(roots[0]!.style.left, '10px');
    assert.equal(roots[0]!.style.top, '30px');
    scene.paint({
      progress: 1,
      positions: [
        { id: 1, left: 60, top: 20 },
        { id: 2, left: 100, top: 20 },
      ],
    });
    assert.equal(f.paint.styleReads, reads);
    assert.equal(f.paint.allocations, allocations);
    const calibration = stage.measured(),
      second = stage.mount(prepared);
    assert.ok(second);
    const warm = second.read(selectAll);
    assert.equal(warm.kind, 'ready');
    assert.equal(stage.measured(), calibration);
    assert.equal(stage.node.parentElement, f.paint.layerMount);
    assert.notEqual(second.node, root);
    assert.equal(root.isConnected, true);
    scene.destroy();
    for (const node of roots) {
      assert.equal(node!.style.left, '');
      assert.equal(node!.style.top, '');
      assert.equal(node!.style.position, '');
      assert.equal(node!.style.visibility, '');
      assert.equal(node!.dataset.reorderMotionChip, undefined);
    }
    assert.equal(second.node.isConnected, true);
    assert.equal(stage.node.isConnected, true);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    stage?.destroy();
    f.destroy();
  }
});
test('directional selection retirement rejects canonical reads and releases all external children without source/history change', () => {
  const f = motionPresentationFixture();
  let stage: ReorderPassiveStage | null = null;
  try {
    const prepared = preparePassiveMotion(f.view, null);
    assert.ok(prepared);
    stage = new ReorderPassiveStage(f.view, prepared);
    const child = stage.mount(prepared);
    assert.ok(child);
    f.dispatch({ selection: EditorSelection.range(4, 1) });
    assert.equal(stage.current(), false);
    const allocations = f.paint.allocations;
    assert.equal(stage.mount(prepared), null);
    assert.equal(f.paint.allocations, allocations);
    assert.deepEqual(child.read(selectAll), { kind: 'unavailable' });
    stage.destroy();
    assert.equal(stage.node.isConnected, false);
    assert.equal(child.node.isConnected, false);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    stage?.destroy();
    f.destroy();
  }
});
test('canonical mount reentry cannot replace a newer pending request or revive a retired generation', () => {
  const f = motionPresentationFixture();
  try {
    const create = f.paint.document.createElement.bind(f.paint.document);
    let entered = false;
    f.paint.document.createElement = (name) => {
      const node = create(name);
      if (!entered && name === 'div') {
        entered = true;
        f.presentation.cancel();
      }
      return node;
    };
    f.prepare();
    f.deliver();
    assert.equal(entered, true);
    assert.equal(f.frames.size, 0);
    assert.equal(f.state.selection.main.anchor, 0);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
    assert.equal(f.view.dom.querySelectorAll('.sp-reorder-motion-paint').length, 0);
  } finally {
    f.destroy();
  }
});

test('retired child reuses complete native paint without discarding gesture calibration', () => {
  const f = motionPresentationFixture();
  let stage: ReorderPassiveStage | null = null;
  try {
    const selection = f.state.selection,
      prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared);
    stage = new ReorderPassiveStage(f.view, prepared);
    const first = stage.mount(prepared);
    assert.ok(first);
    const originalNode = first.node,
      originalPaint = originalNode.querySelector('.sp-reorder-preview');
    assert.ok(originalPaint);
    first.destroy();
    assert.equal(first.current(), false);
    assert.equal(first.retained(), false);
    assert.equal(originalNode.dataset.reorderMotionProgress, undefined);
    assert.equal(originalNode.style.clip, 'rect(0px, 0px, 0px, 0px)');
    assert.ok(originalNode.parentElement === stage.node);
    assert.equal(originalNode.isConnected, true);
    assert.equal(originalNode.querySelectorAll('[data-reorder-motion-chip]').length, 0);
    f.paint.admitLayer(stage);
    const placement = stage.readPlacement(prepared);
    assert.ok(placement);
    stage.place(prepared, placement);
    const calibration = stage.measured(),
      second = stage.mount(prepared);
    assert.ok(second);
    assert.notEqual(second, first);
    assert.equal(second.node, originalNode);
    assert.equal(second.node.querySelector('.sp-reorder-preview'), originalPaint);
    const read = second.read(selectAll);
    assert.equal(read.kind, 'ready');
    assert.equal(stage.measured(), calibration);
    assert.equal(stage.node.parentElement, f.paint.layerMount);
    assert.equal(first.current(), false);
    assert.equal(first.retained(), false);
    assert.equal(second.current(), true);
    assert.equal(second.retained(), true);
    first.destroy();
    assert.equal(second.node.isConnected, true);
    assert.equal(second.current(), true);
    assert.equal(second.retained(), true);
    assert.equal(stage.measured(), calibration);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(f.state.selection, selection);
    assert.equal(undoDepth(f.state), 0);
  } finally {
    stage?.destroy();
    f.destroy();
  }
});

for (const scalar of [
  'scrollTop',
  'scrollLeft',
  'scrollX',
  'scrollY',
  'innerWidth',
  'innerHeight',
  'devicePixelRatio',
] as const)
  test(`native READ rejects silently changed viewport paint before publication: ${scalar}`, () => {
    const f = motionPresentationFixture();
    let stage: ReorderPassiveStage | null = null;
    try {
      const prepared = preparePassiveMotion(f.view, 0);
      assert.ok(prepared);
      stage = new ReorderPassiveStage(f.view, prepared);
      const child = stage.mount(prepared);
      assert.ok(child);
      f.paint.admitLayer(stage);
      const placement = child.read(selectAll);
      assert.equal(placement.kind, 'placement');
      if (placement.kind !== 'placement') assert.fail('native-calibration-required');
      child.place(placement);
      const target =
        scalar === 'scrollTop' || scalar === 'scrollLeft'
          ? f.view.scrollDOM
          : f.view.dom.ownerDocument.defaultView;
      assert.ok(target);
      Reflect.set(target, scalar, 17);
      // Source/mount leases stay cheap; the next actual native READ owns scalar recertification.
      assert.equal(child.current(), true);
      assert.equal(child.retained(), true);
      assert.equal(child.read(selectAll).kind, 'unavailable');
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
    } finally {
      stage?.destroy();
      f.destroy();
    }
  });

for (const retired of [false, true])
  for (const primary of [
    null,
    undefined,
    new Error('controlled-canonical-render-failure'),
  ] as const)
    for (const cleanupFails of [false, true])
      test(`canonical mount preserves unexpected raw render failure retired=${retired} primary=${String(primary)} cleanup=${cleanupFails}`, () => {
        const f = motionPresentationFixture(),
          cleanup = new Error('controlled-canonical-child-retirement');
        let stage: ReorderPassiveStage | null = null;
        try {
          const prepared = preparePassiveMotion(f.view, 0);
          assert.ok(prepared);
          stage = new ReorderPassiveStage(f.view, prepared);
          const allocated = stage,
            create = f.paint.document.createElement.bind(f.paint.document);
          let removals = 0;
          f.paint.document.createElement = (name) => {
            const node = create(name),
              remove = node.remove.bind(node);
            node.remove = () => {
              remove();
              if (node.className === 'sp-reorder-motion-paint') {
                removals++;
                if (cleanupFails) throw cleanup;
              }
            };
            return node;
          };
          const append = allocated.node.append.bind(allocated.node);
          allocated.node.append = (...nodes) => {
            append(...nodes);
            if (retired) allocated.destroy();
            // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
            throw primary;
          };
          let failed = false,
            actual: unknown;
          try {
            stage.mount(prepared);
          } catch (cause) {
            failed = true;
            actual = cause;
          }
          assert.equal(failed, true);
          if (cleanupFails) {
            assert.ok(actual instanceof AggregateError);
            assert.deepEqual(actual.errors, [primary, cleanup]);
          } else assert.equal(actual, primary);
          assert.equal(removals, 1);
          assert.equal(f.view.dom.querySelectorAll('.sp-reorder-motion-paint').length, 0);
          assert.equal(readExactSource(f.state), 'a, b, c');
          assert.equal(undoDepth(f.state), 0);
          assert.equal(f.frames.size, 0);
        } finally {
          stage?.destroy();
          f.destroy();
        }
      });

for (const cleanupFails of [false, true])
  test(`post-construction child-current admission keeps raw primary and retires untransferred native output, cleanup=${cleanupFails}`, () => {
    const f = motionPresentationFixture(),
      primary = new Error('controlled-child-current-admission'),
      cleanup = new Error('controlled-child-current-cleanup');
    const preflight = ReadonlyWidgetMedia.prototype.preflightAll;
    let stage: ReorderPassiveStage | null = null,
      hydrated = false;
    try {
      const prepared = preparePassiveMotion(f.view, 0);
      assert.ok(prepared);
      stage = new ReorderPassiveStage(f.view, prepared);
      let reads = 0,
        removed = 0;
      /** Actual successful empty hydration arms the post-construction admission frontier. */
      ReadonlyWidgetMedia.prototype.preflightAll = function () {
        const result = preflight.call(this);
        if (result.kind !== 'ready') return result;
        return {
          ...result,
          plan: {
            ...result.plan,
            /** Preserve the real media freshness check before observing later child admission. */
            hydrate() {
              const accepted = result.plan.hydrate();
              if (accepted) hydrated = true;
              return accepted;
            },
          },
        };
      };
      const admission = {
        ...prepared,
        context: {
          ...prepared.context,
          /** Slot preflight accepts completed hydration; the final child admission is the failing external lease frontier. */
          current() {
            if (hydrated && ++reads === 2) throw primary;
            return prepared.context.current();
          },
        },
      };
      const create = f.paint.document.createElement.bind(f.paint.document);
      f.paint.document.createElement = (tag) => {
        const node = create(tag),
          remove = node.remove.bind(node);
        node.remove = () => {
          remove();
          if (node.className === 'sp-reorder-motion-paint') {
            removed++;
            if (cleanupFails) throw cleanup;
          }
        };
        return node;
      };
      const owner = stage;
      assert.throws(
        () => owner.mount(admission),
        (cause) =>
          cleanupFails
            ? cause instanceof AggregateError &&
              cause.errors[0] === primary &&
              cause.errors[1] === cleanup
            : cause === primary,
      );
      assert.equal(reads, 2);
      assert.equal(removed, 1);
      assert.equal(f.view.dom.querySelectorAll('.sp-reorder-motion-paint').length, 0);
      assert.equal(f.state.field(reorderPreview).mask, null);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
    } finally {
      ReadonlyWidgetMedia.prototype.preflightAll = preflight;
      stage?.destroy();
      f.destroy();
    }
  });
