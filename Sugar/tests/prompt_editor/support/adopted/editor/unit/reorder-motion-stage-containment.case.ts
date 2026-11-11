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
/** Real stage/Pending lifetime consumes controlled native rectangles; this boundary does not certify native containment, glyph pixels or performance. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { reorderMotionTargets } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-targets.ts';
import type { ReorderMotionSelection } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-scene.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

/** Actual scene observations get independent translated origins so both complete groups must remain admitted beyond the calibration height. */
const selectChanged: ReorderMotionSelection = (geometry) =>
  reorderMotionTargets(
    geometry.map((rect) => ({
      ...rect,
      left: rect.left + 10,
      right: rect.right + 10,
    })),
    geometry,
    null,
  );

test('the native marker retains the same Stage with intervening owned editor siblings', () => {
  const f = motionPresentationFixture();
  let stage: ReorderPassiveStage | null = null;
  try {
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared);
    stage = new ReorderPassiveStage(f.view, prepared);
    const sibling = f.paint.document.createElement('span');
    f.paint.editor.insertBefore(sibling, f.paint.scroller);
    assert.equal(stage.current(), true);
    assert.ok(stage.mount(prepared));
  } finally {
    stage?.destroy();
    f.destroy();
  }
});

for (const grabbed of [false, true])
  test(`primary cursor output preserves the same captured Stage when grabbed=${grabbed}`, () => {
    const f = motionPresentationFixture();
    let stage: ReorderPassiveStage | null = null;
    const causes: unknown[] = [];
    try {
      if (grabbed) f.paint.editor.setAttribute('data-reorder-grab', 'true');
      const prepared = preparePassiveMotion(f.view, 0);
      assert.ok(prepared);
      stage = new ReorderPassiveStage(f.view, prepared);
      assert.equal(stage.current(), true);
      const node = stage.node;
      if (grabbed) f.paint.editor.removeAttribute('data-reorder-grab');
      else f.paint.editor.setAttribute('data-reorder-grab', 'true');
      assert.equal(stage.current(), true);
      assert.equal(stage.node, node);
      assert.equal(node.isConnected, true);
      f.paint.editor.setAttribute('data-host-appearance', 'changed');
      assert.equal(stage.current(), false);
      f.paint.editor.removeAttribute('data-host-appearance');
      assert.equal(stage.current(), true);
    } catch (cause) {
      causes.push(cause);
    }
    for (const release of [() => stage?.destroy(), () => f.destroy()])
      try {
        release();
      } catch (cleanup) {
        causes.push(cleanup);
      }
    if (causes.length === 1) throw causes[0];
    if (causes.length)
      throw new AggregateError(causes, 'Primary cursor Stage proof retirement failed');
  });

for (const mutation of [
  'stage-before',
  'stage-parent',
  'scroller-parent',
  'scroller-replacement',
  'callback-order',
] as const)
  test(`exact native layer ownership rejects ${mutation} without new publication`, () => {
    const f = motionPresentationFixture();
    let stage: ReorderPassiveStage | null = null,
      armed = false;
    try {
      const captured = preparePassiveMotion(f.view, 0);
      assert.ok(captured);
      const prepared = {
        ...captured,
        context: {
          ...captured.context,
          appearanceCurrent: () => {
            const accepted = captured.context.appearanceCurrent();
            if (armed && stage)
              f.paint.editor.insertBefore(
                stage.node as unknown as typeof f.paint.scroller,
                f.paint.scroller,
              );
            return accepted;
          },
        },
      };
      stage = new ReorderPassiveStage(f.view, prepared);
      const child = stage.mount(prepared);
      assert.ok(child);
      const source = readExactSource(f.state),
        selection = f.state.selection;
      if (mutation === 'callback-order') armed = true;
      else if (mutation === 'stage-before')
        f.paint.editor.insertBefore(
          stage.node as unknown as typeof f.paint.scroller,
          f.paint.scroller,
        );
      else if (mutation === 'stage-parent')
        f.paint.content.insertBefore(stage.node as unknown as typeof f.paint.scroller, null);
      else {
        if (mutation === 'scroller-replacement') {
          const replacement = f.paint.document.createElement('div');
          replacement.className = 'cm-scroller';
          f.paint.editor.insertBefore(replacement, f.paint.scroller);
        }
        assert.ok(f.paint.document.body);
        f.paint.document.body.insertBefore(f.paint.scroller, null);
      }
      const allocations = f.paint.allocations;
      assert.equal(stage.current(), false);
      assert.equal(stage.mount(prepared), null);
      assert.equal(child.read(selectChanged).kind, 'unavailable');
      assert.equal(f.paint.allocations, allocations);
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
      assert.equal(redoDepth(f.state), 0);
      assert.equal(f.paint.editor.hasAttribute('data-reorder-motion'), false);
    } finally {
      stage?.destroy();
      f.destroy();
    }
  });

for (const top of [300, 400])
  test(`fixed256 calibration retains complete canonical peer output at nativeY${top} inside the actual440 viewport`, () => {
    const f = motionPresentationFixture(),
      create = f.paint.document.createElement.bind(f.paint.document);
    let stage: ReorderPassiveStage | null = null;
    Object.assign(f.paint.scroller.box, { height: 440 });
    f.paint.document.createElement = (name) => {
      const node = create(name),
        nativeRect = node.getBoundingClientRect.bind(node);
      /** Independent external native geometry locates complete chip and rich-descendant pieces below the fixed calibration box. */
      node.getBoundingClientRect = () => {
        const value = nativeRect();
        let root: typeof node | null = node;
        while (root && root.dataset.reorderChip === undefined) root = root.parent;
        // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
        return root ? ({ ...value, top, bottom: top + value.height, y: top } as DOMRect) : value;
      };
      return node;
    };
    try {
      const source = readExactSource(f.state),
        selection = f.state.selection,
        prepared = preparePassiveMotion(f.view, 0);
      assert.ok(prepared);
      stage = new ReorderPassiveStage(f.view, prepared);
      const plane = stage.node,
        child = stage.mount(prepared);
      assert.ok(child);
      assert.equal(plane.style.width, `${prepared.context.width}px`);
      assert.equal(plane.style.height, '256px');
      assert.equal(plane.style.contain, 'layout size');
      assert.equal(plane.style.overflow, 'visible');
      assert.equal(plane.getBoundingClientRect().height, 256);
      f.paint.admitLayer(stage);
      const placement = child.read(selectChanged);
      assert.equal(placement.kind, 'placement');
      if (placement.kind !== 'placement') assert.fail('actual-calibration-required');
      assert.equal(placement.scaleY, 1);
      assert.equal(placement.scaleX, 200 / prepared.context.width);
      child.place(placement);
      const calibration = stage.measured(),
        read = child.read(selectChanged);
      assert.equal(read.kind, 'ready');
      if (read.kind !== 'ready') assert.fail('complete-below-calibration-groups-required');
      assert.deepEqual(
        read.scene.chips.map((chip) => chip.id),
        [1, 2],
      );
      assert.equal(new Set(read.scene.geometry.map((rect) => rect.index)).size, 2);
      assert.ok(read.scene.geometry.every((rect) => rect.top === top && rect.bottom === top + 20));
      assert.ok(
        read.scene.geometry.every((rect) => rect.top > plane.getBoundingClientRect().bottom),
      );
      assert.equal(read.scene.node.querySelectorAll('[data-reorder-chip]').length, 3);
      child.reveal();
      assert.equal(read.scene.node.querySelectorAll('[data-reorder-motion-chip]').length, 2);
      assert.equal(read.scene.whole, undefined);
      assert.equal(read.scene.node.style.clip, 'rect(-20px, 230px, 420px, -10px)');
      read.scene.paint({
        progress: 0.5,
        positions: [
          { id: 1, left: 25, top: top + 5 },
          { id: 2, left: 105, top: top + 5 },
        ],
      });
      assert.equal(read.scene.chips[0]!.roots[0]!.node.style.top, '5px');
      assert.equal(plane.style.height, '256px');
      child.destroy();
      const warm = stage.mount(prepared);
      assert.ok(warm);
      assert.equal(warm.read(selectChanged).kind, 'ready');
      assert.equal(stage.measured(), calibration);
      assert.equal(stage.node, plane);
      assert.equal(plane.getBoundingClientRect().height, 256);
      stage.destroy();
      assert.equal(plane.isConnected, false);
      assert.equal(child.node.isConnected, false);
      assert.equal(warm.node.isConnected, false);
      const allocations = f.paint.allocations;
      assert.equal(stage.mount(prepared), null);
      assert.equal(f.paint.allocations, allocations);
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
      assert.equal(redoDepth(f.state), 0);
    } finally {
      stage?.destroy();
      f.destroy();
    }
  });
