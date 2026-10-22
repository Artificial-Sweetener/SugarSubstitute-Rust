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
/** Whole pointer cancellation retains the original physical write reservation through late native delegates. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { redoDepth, undoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { reorderMotionTargets } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-targets.ts';
import { wholeFixture } from './reorder-pointer-stage-fixture.ts';

/** An independent translated-origin oracle selects actual native peer geometry through the existing motion owner. */
const select = (geometry: Parameters<typeof reorderMotionTargets>[1]) =>
  reorderMotionTargets(
    geometry.map((rect) => ({
      ...rect,
      left: rect.left + 10,
      right: rect.right + 10,
    })),
    geometry,
    null,
  );

for (const frontier of ['clip', 'frame'] as const)
  test(`whole before-delegate ${frontier} cancellation cannot lend its physical shell before late writes close`, () => {
    const f = wholeFixture(),
      failures: unknown[] = [];
    let stage: ReorderPassiveStage | null = null;
    try {
      const source = readExactSource(f.state),
        selection = f.state.selection,
        prepared = preparePassiveMotion(f.view, 0);
      assert.ok(prepared?.whole);
      stage = new ReorderPassiveStage(f.view, prepared);
      const ownedStage = stage,
        pending = ownedStage.mount(prepared);
      assert.ok(pending);
      f.paint.admitLayer(ownedStage);
      const placement = pending.read(select);
      assert.equal(placement.kind, 'placement');
      if (placement.kind !== 'placement') assert.fail('WHOLE_REENTRY_PLACEMENT_REQUIRED');
      pending.place(placement);
      const read = pending.read(select);
      assert.equal(read.kind, 'ready');
      if (read.kind !== 'ready') assert.fail('WHOLE_REENTRY_SCENE_REQUIRED');
      const node = pending.node,
        replacements: { newer: ReturnType<ReorderPassiveStage['mount']> } = {
          newer: null,
        };
      let entered = false;
      /** Nested cancellation revokes the original lease while its outer native delegate remains reserved. */
      const replace = (): void => {
        if (entered) return;
        entered = true;
        pending.destroy();
        replacements.newer = ownedStage.mount(prepared);
      };
      if (frontier === 'clip') {
        const setProperty = node.style.setProperty.bind(node.style);
        node.style.setProperty = (name, value, priority) => {
          if (name === 'clip' && value === 'auto') replace();
          setProperty(name, value, priority);
        };
        pending.reveal();
      } else {
        pending.reveal();
        const root = read.scene.chips[0]!.roots[0]!.node;
        let left = root.style.left;
        Object.defineProperty(root.style, 'left', {
          configurable: true,
          get: () => left,
          /** Resume the captured old native write after cancellation has admitted another physical shell. */
          set(value: string) {
            if (value !== '') replace();
            left = value;
          },
        });
        read.scene.paint({
          progress: 0.5,
          positions: [{ id: read.scene.chips[0]!.id, left: 77, top: 31 }],
        });
      }
      const newer = replacements.newer;
      assert.equal(entered, true);
      assert.ok(newer);
      assert.notEqual(newer.node, node);
      assert.equal(newer.current(), true);
      assert.equal(node.isConnected, false);
      assert.equal(node.parentElement, null);
      assert.equal(node.style.clip, 'rect(0px, 0px, 0px, 0px)');
      assert.equal(node.dataset.reorderMotionProgress, undefined);
      assert.equal(node.dataset.reorderMotionEndpoint, undefined);
      assert.equal(newer.node.style.clip, 'rect(0px, 0px, 0px, 0px)');
      assert.equal(newer.node.dataset.reorderMotionProgress, undefined);
      assert.equal(newer.node.dataset.reorderMotionEndpoint, undefined);
      assert.equal(newer.node.querySelectorAll('[data-reorder-motion-chip]').length, 0);
      assert.ok(
        Array.from(newer.node.querySelectorAll<HTMLElement>('[data-reorder-chip]')).every(
          (root) => root.style.left === '' && root.style.top === '',
        ),
      );
      pending.destroy();
      assert.equal(newer.current(), true);
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
    } catch (cause) {
      failures.push(cause);
    }
    for (const cleanup of [() => stage?.destroy(), () => f.destroy()])
      try {
        cleanup();
      } catch (cause) {
        failures.push(cause);
      }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'WHOLE_REENTRY_RETIREMENT_FAILED');
  });
