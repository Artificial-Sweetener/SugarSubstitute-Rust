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
/** The canonical shell owns protected rectangular clipping through acquisition, admitted reveal, exact reuse and retirement. */
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
    try {
      stage?.destroy();
    } finally {
      f.destroy();
    }
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
    assert.equal(node.style.clip, 'rect(0px, 0px, 0px, 0px)');
    f.paint.admitLayer(stage);
    const placement = pending.read(select);
    assert.equal(placement.kind, 'placement');
    if (placement.kind !== 'placement') assert.fail('publication-placement-required');
    pending.place(placement);
    const read = pending.read(select);
    assert.equal(read.kind, 'ready');
    if (read.kind !== 'ready') assert.fail('publication-scene-required');
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
      throw new AggregateError([primary, cleanup], 'publication-fixture-acquisition-failed');
    }
    throw primary;
  }
}

test('pending and reused canonical shells remain clipped until complete reveal and clear before retirement', () => {
  const f = publicationFixture();
  try {
    assert.equal(f.node.style.clip, 'rect(0px, 0px, 0px, 0px)');
    f.pending.reveal();
    assert.equal(f.node.style.clip, 'rect(-20px, 230px, 80px, -10px)');
    assert.equal(
      f.scene.chips.every((chip) =>
        chip.roots.every((root) => root.node.style.visibility === 'visible'),
      ),
      true,
    );
    f.pending.destroy();
    assert.equal(f.node.style.clip, 'rect(0px, 0px, 0px, 0px)');
    assert.equal(f.node.isConnected, true);
    const next = f.stage.mount(f.prepared);
    assert.ok(next);
    assert.equal(next.node === f.node, true);
    assert.equal(next.node.style.clip, 'rect(0px, 0px, 0px, 0px)');
    assert.equal(next.read(select).kind, 'ready');
    assert.equal(next.node.style.clip, 'rect(0px, 0px, 0px, 0px)');
    next.reveal();
    assert.equal(next.node.style.clip, 'rect(-20px, 230px, 80px, -10px)');
    f.pending.destroy();
    assert.equal(next.node.style.clip, 'rect(-20px, 230px, 80px, -10px)');
    assert.equal(readExactSource(f.f.state) === f.source, true);
    assert.equal(f.f.state.selection === f.selection, true);
    assert.equal(undoDepth(f.f.state), 0);
    assert.equal(redoDepth(f.f.state), 0);
  } finally {
    f.destroy();
  }
});

for (const frontier of ['clip', 'root'] as const)
  test(`reentrant ${frontier} reveal cannot expose a newer pending lease borrowed from the same shell`, () => {
    const f = publicationFixture();
    try {
      let next: ReturnType<ReorderPassiveStage['mount']> = null,
        reentered = false;
      const cancel = (): void => {
        if (reentered) return;
        reentered = true;
        f.pending.destroy();
        next = f.stage.mount(f.prepared);
      };
      if (frontier === 'clip') {
        const set = f.node.style.setProperty.bind(f.node.style);
        f.node.style.setProperty = (name, value, priority) => {
          if (name === 'clip') cancel();
          set(name, value, priority);
        };
      } else {
        const style = f.scene.chips[0]!.roots[0]!.node.style;
        let value = style.visibility;
        Object.defineProperty(style, 'visibility', {
          configurable: true,
          get: () => value,
          set(requested: string) {
            cancel();
            value = requested;
          },
        });
      }
      f.pending.reveal();
      assert.equal(reentered, true);
      /** The delegated setter assigns synchronously; the getter retains TypeScript's exact nullable boundary. */
      const replacement = (): ReturnType<ReorderPassiveStage['mount']> => next;
      const child = replacement();
      assert.ok(child);
      assert.equal(child.node === f.node, false);
      assert.equal(f.node.isConnected, false);
      assert.equal(child.node.style.clip, 'rect(0px, 0px, 0px, 0px)');
      assert.equal(child.node.querySelectorAll('[data-reorder-motion-chip]').length, 0);
      assert.equal(child.read(select).kind, 'ready');
      child.reveal();
      assert.equal(child.node.style.clip, 'rect(-20px, 230px, 80px, -10px)');
      f.pending.destroy();
      assert.equal(child.node.style.clip, 'rect(-20px, 230px, 80px, -10px)');
      assert.equal(readExactSource(f.f.state) === f.source, true);
      assert.equal(f.f.state.selection === f.selection, true);
      assert.equal(undoDepth(f.f.state), 0);
      assert.equal(redoDepth(f.f.state), 0);
    } finally {
      f.destroy();
    }
  });

for (const primary of [null, undefined])
  test(`failed protected clip clear preserves raw ${String(primary)} and retires the exact shell after remaining resets`, () => {
    const f = publicationFixture();
    try {
      f.pending.reveal();
      const set = f.node.style.setProperty.bind(f.node.style);
      let attempts = 0;
      f.node.style.setProperty = (name, value, priority) => {
        set(name, value, priority);
        if (name === 'clip') {
          attempts++;
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw primary;
        }
      };
      let failed = false,
        actual: unknown;
      try {
        f.pending.destroy();
      } catch (cause) {
        failed = true;
        actual = cause;
      }
      assert.equal(failed, true);
      assert.equal(actual === primary, true);
      assert.equal(attempts, 1);
      assert.equal(f.node.style.getPropertyPriority('clip'), 'important');
      assert.equal(f.node.isConnected, false);
      assert.equal(f.node.style.clip, 'rect(0px, 0px, 0px, 0px)');
      assert.equal(f.node.querySelectorAll('[data-reorder-motion-chip]').length, 0);
      assert.equal(readExactSource(f.f.state) === f.source, true);
      assert.equal(undoDepth(f.f.state), 0);
      assert.equal(redoDepth(f.f.state), 0);
    } finally {
      f.destroy();
    }
  });

test('publication supersession during initial clipping acquisition retires unpublished canonical paint', () => {
  const f = motionPresentationFixture();
  let stage: ReorderPassiveStage | null = null;
  try {
    const source = readExactSource(f.state),
      prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared);
    stage = new ReorderPassiveStage(f.view, prepared);
    const create = f.document.createElement.bind(f.document);
    let superseded = false,
      removals = 0;
    f.document.createElement = (name) => {
      const node = create(name),
        remove = node.remove.bind(node);
      const set = node.style.setProperty.bind(node.style);
      node.style.setProperty = (property, value, priority) => {
        set(property, value, priority);
        if (property !== 'clip' || node.className !== 'sp-reorder-motion-paint' || superseded)
          return;
        assert.equal(value, 'rect(0px, 0px, 0px, 0px)');
        assert.equal(priority, 'important');
        superseded = true;
        f.publish();
      };
      node.remove = () => {
        remove();
        if (node.className === 'sp-reorder-motion-paint') removals++;
      };
      return node;
    };
    assert.equal(stage.mount(prepared) === null, true);
    assert.equal(superseded, true);
    assert.equal(removals, 1);
    assert.equal(f.view.dom.querySelectorAll('.sp-reorder-motion-paint').length, 0);
    assert.equal(readExactSource(f.state) === source, true);
    assert.equal(undoDepth(f.state), 0);
    assert.equal(redoDepth(f.state), 0);
  } finally {
    try {
      stage?.destroy();
    } finally {
      f.destroy();
    }
  }
});
