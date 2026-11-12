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
/** Whole paint uses its admitted native viewport without measuring an unused partial clip rectangle. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { redoDepth, undoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { reorderMotionTargets } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-targets.ts';
import { wholeFixture } from './reorder-pointer-stage-fixture.ts';

test('calibrated whole paint reads and reveals without consulting a partial clip origin', () => {
  const fixture = wholeFixture(),
    failures: unknown[] = [];
  let stage: ReorderPassiveStage | null = null,
    restore: (() => void) | null = null;
  try {
    const source = readExactSource(fixture.state),
      selection = fixture.state.selection,
      prepared = preparePassiveMotion(fixture.view, 0);
    assert.ok(prepared?.whole);
    stage = new ReorderPassiveStage(fixture.view, prepared);
    const pending = stage.mount(prepared);
    assert.ok(pending);
    fixture.paint.admitLayer(stage);
    /** Shift the original targets independently so this exercises actual admitted whole paint. */
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
    const placement = pending.read(select);
    assert.equal(placement.kind, 'placement');
    if (placement.kind !== 'placement') assert.fail('WHOLE_NATIVE_CALIBRATION_REQUIRED');
    pending.place(placement);
    const node = stage.node,
      original = node.getBoundingClientRect;
    restore = () => {
      node.getBoundingClientRect = original;
    };
    /** The Stage's genuine calibration already completed; whole reveal never consumes a local clip. */
    node.getBoundingClientRect = () => {
      throw new Error('UNUSED_WHOLE_CLIP_ORIGIN_READ');
    };
    const read = pending.read(select);
    assert.equal(read.kind, 'ready');
    if (read.kind !== 'ready') assert.fail('WHOLE_NATIVE_PAINT_REQUIRED');
    assert.equal(read.scene.whole, true);
    assert.ok(read.scene.chips.length > 0);
    assert.equal(pending.node.style.clip, 'rect(0px, 0px, 0px, 0px)');
    pending.reveal();
    assert.equal(pending.node.style.clip, 'auto');
    read.scene.publishEndpoint!('whole-clip-measurement');
    assert.equal(pending.node.dataset.reorderMotionEndpoint, 'whole-clip-measurement');
    assert.equal(readExactSource(fixture.state) === source, true);
    assert.equal(fixture.state.selection === selection, true);
    assert.deepEqual([undoDepth(fixture.state), redoDepth(fixture.state)], [0, 0]);
  } catch (cause) {
    failures.push(cause);
  }
  for (const release of [() => restore?.(), () => stage?.destroy(), () => fixture.destroy()])
    try {
      release();
    } catch (cause) {
      failures.push(cause);
    }
  if (failures.length === 1) throw failures[0];
  if (failures.length) throw new AggregateError(failures, 'WHOLE_CLIP_PROOF_RETIREMENT_FAILED');
});
