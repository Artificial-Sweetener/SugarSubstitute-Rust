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
/** Actual canonical passive paint must survive finite same-context proposal leases. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { reorderMotionTargets } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-targets.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

test('same-context proposal reuses the original complete canonical paint instead of reconstructing it', () => {
  const f = motionPresentationFixture();
  let stage: ReorderPassiveStage | null = null;
  try {
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared);
    stage = new ReorderPassiveStage(f.view, prepared);
    const first = stage.mount(prepared);
    assert.ok(first);
    const original = first.node;
    const lines = Array.from(
      original.querySelectorAll('[data-reorder-chip]'),
      (node) => node.childNodes[0],
    );
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
    f.paint.admitLayer(stage);
    const placement = first.read(select);
    assert.equal(placement.kind, 'placement');
    if (placement.kind !== 'placement') assert.fail('calibration-required');
    first.place(placement);
    const read = first.read(select);
    assert.equal(read.kind, 'ready');
    if (read.kind !== 'ready') assert.fail('actual-scene-required');
    first.reveal();
    read.scene.paint({
      progress: 0.5,
      positions: [{ id: 1, left: 25, top: 20 }],
    });
    first.destroy();
    const second = stage.mount(prepared);
    assert.ok(second);
    assert.ok(second.node === original);
    assert.deepEqual(
      Array.from(second.node.querySelectorAll('[data-reorder-chip]'), (node) => node.childNodes[0]),
      lines,
    );
    read.scene.paint({
      progress: 0.9,
      positions: [{ id: 1, left: 999, top: 999 }],
    });
    read.scene.destroy();
    assert.equal(second.node.dataset.reorderMotionProgress, undefined);
    assert.equal(second.node.style.clip, 'rect(0px, 0px, 0px, 0px)');
    assert.equal(second.node.querySelectorAll('[data-reorder-motion-chip]').length, 0);
    assert.equal(second.current(), true);
    f.view.scrollDOM.scrollTop = 17;
    assert.equal(second.read(select).kind, 'unavailable');
    assert.equal(second.node.querySelectorAll('[data-reorder-motion-chip]').length, 0);
  } finally {
    try {
      stage?.destroy();
    } finally {
      f.destroy();
    }
  }
});
