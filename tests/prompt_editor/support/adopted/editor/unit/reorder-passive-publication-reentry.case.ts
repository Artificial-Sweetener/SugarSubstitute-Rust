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
/** Existing Stage/Scene APIs expose a cancelled native delegate that resumes after nested retirement on both old and new runtime boundaries. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { redoDepth, undoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { reorderMotionTargets } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-targets.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

/** Actual motion selection admits translated peers from independently transported native geometry. */
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

/** Construction and terminal cleanup stay finite even if a setup assertion fails. */
function publicationFixture() {
  const f = motionPresentationFixture();
  let stage: ReorderPassiveStage | null = null;
  /** Retire the actual Stage before its native fixture regardless of either cleanup result. */
  const destroy = (): void => {
    const failures: unknown[] = [];
    for (const retire of [() => stage?.destroy(), () => f.destroy()])
      try {
        retire();
      } catch (cause) {
        failures.push(cause);
      }
    if (failures.length === 1) throw failures[0];
    if (failures.length)
      throw new AggregateError(failures, 'publication-fixture-retirement-failed');
  };
  try {
    const source = readExactSource(f.state),
      selection = f.state.selection,
      prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared);
    stage = new ReorderPassiveStage(f.view, prepared);
    const pending = stage.mount(prepared);
    assert.ok(pending);
    const node = pending.node;
    assert.equal(
      node.style.clip || node.style.opacity,
      node.style.clip ? 'rect(0px, 0px, 0px, 0px)' : '0',
    );
    f.paint.admitLayer(stage);
    const placement = pending.read(select);
    assert.equal(placement.kind, 'placement');
    if (placement.kind !== 'placement') assert.fail('opacity-placement-required');
    pending.place(placement);
    const read = pending.read(select);
    assert.equal(read.kind, 'ready');
    if (read.kind !== 'ready') assert.fail('opacity-scene-required');
    return {
      f,
      stage,
      prepared,
      pending,
      scene: read.scene,
      node,
      source,
      selection,
      destroy,
    };
  } catch (primary) {
    try {
      destroy();
    } catch (cleanup) {
      throw new AggregateError([primary, cleanup], 'opacity-fixture-acquisition-failed');
    }
    throw primary;
  }
}

for (const frontier of ['publication', 'frame'] as const)
  test(`a before-delegate ${frontier} cancellation never lends its in-flight physical shell`, () => {
    const f = publicationFixture(),
      failures: unknown[] = [];
    try {
      let next: ReturnType<ReorderPassiveStage['mount']> = null,
        entered = false;
      const replace = (): void => {
        if (entered) return;
        entered = true;
        f.pending.destroy();
        next = f.stage.mount(f.prepared);
      };
      if (frontier === 'publication') {
        if (f.node.style.clip) {
          const set = f.node.style.setProperty.bind(f.node.style);
          f.node.style.setProperty = (name, value, priority) => {
            if (name === 'clip' && value !== 'rect(0px, 0px, 0px, 0px)') replace();
            set(name, value, priority);
          };
        } else {
          let opacity = f.node.style.opacity;
          Object.defineProperty(f.node.style, 'opacity', {
            configurable: true,
            get: () => opacity,
            set(value: string) {
              if (value === '1') replace();
              opacity = value;
            },
          });
        }
        f.pending.reveal();
      } else {
        f.pending.reveal();
        const style = f.scene.chips[0]!.roots[0]!.node.style;
        let left = style.left;
        Object.defineProperty(style, 'left', {
          configurable: true,
          get: () => left,
          set(value: string) {
            if (value !== '') replace();
            left = value;
          },
        });
        f.scene.paint({
          progress: 0.5,
          positions: [{ id: f.scene.chips[0]!.id, left: 77, top: 31 }],
        });
      }
      assert.equal(entered, true);
      /** Only the native callback assigns this reference; no runtime-private owner is inspected. */
      const replacement = (): ReturnType<ReorderPassiveStage['mount']> => next;
      const child = replacement();
      assert.ok(child);
      assert.equal(
        child.node.style.clip || child.node.style.opacity,
        child.node.style.clip ? 'rect(0px, 0px, 0px, 0px)' : '0',
      );
      assert.ok(
        [...child.node.querySelectorAll<HTMLElement>('[data-reorder-chip]')].every(
          (root) => root.style.left === '' && root.style.top === '',
        ),
      );
      assert.notEqual(child.node, f.node);
      assert.equal(f.node.isConnected, false);
      assert.equal(child.node.querySelectorAll('[data-reorder-motion-chip]').length, 0);
      assert.equal(child.node.dataset.reorderMotionProgress, undefined);
      assert.equal(child.node.dataset.reorderMotionEndpoint, undefined);
      assert.equal(readExactSource(f.f.state), f.source);
      assert.equal(f.f.state.selection, f.selection);
      assert.deepEqual([undoDepth(f.f.state), redoDepth(f.f.state)], [0, 0]);
    } catch (cause) {
      failures.push(cause);
    }
    try {
      f.destroy();
    } catch (cause) {
      failures.push(cause);
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length)
      throw new AggregateError(failures, 'publication-control-retirement-failed');
  });
