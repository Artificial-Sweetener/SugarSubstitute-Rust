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
/** Passive lease retirement clears its own motion output before canonical wrappers can be borrowed again. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { reorderMotionTargets } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-targets.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

/** Real Stage, Pending and Scene owners perform calibration/reveal; only native geometry is externally transported. */
function preparedScene() {
  const f = motionPresentationFixture();
  const prepared = preparePassiveMotion(f.view, 0);
  assert.ok(prepared);
  const stage = new ReorderPassiveStage(f.view, prepared);
  const pending = stage.mount(prepared);
  assert.ok(pending);
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
  const placement = pending.read(select);
  assert.equal(placement.kind, 'placement');
  if (placement.kind !== 'placement') assert.fail('native-placement-required');
  pending.place(placement);
  const read = pending.read(select);
  assert.equal(read.kind, 'ready');
  if (read.kind !== 'ready') assert.fail('native-scene-required');
  pending.reveal();
  read.scene.paint({
    progress: 0.5,
    positions: read.scene.chips.map((chip) => ({
      id: chip.id,
      left: chip.left + 5,
      top: chip.top + 3,
    })),
  });
  return { f, prepared, stage, pending, scene: read.scene, select };
}

test('retired passive wrappers clear actual scene offsets before exact-node reuse and fresh geometry', () => {
  const f = preparedScene();
  try {
    const roots = f.scene.chips.flatMap((chip) => chip.roots.map((root) => root.node));
    assert.ok(roots.length > 0);
    assert.ok(roots.every((root) => root.style.left !== '' && root.style.visibility === 'visible'));
    const node = f.pending.node;
    let removals = 0,
      additions = 0;
    const remove = node.remove.bind(node),
      append = f.stage.node.append.bind(f.stage.node);
    node.remove = () => {
      removals++;
      remove();
    };
    f.stage.node.append = (...nodes) => {
      additions++;
      append(...nodes);
    };
    f.pending.destroy();
    assert.ok(node.parentElement === f.stage.node);
    assert.equal(node.isConnected, true);
    assert.equal(node.dataset.reorderMotionProgress, undefined);
    assert.equal(node.style.clip, 'rect(0px, 0px, 0px, 0px)');
    for (const root of roots) {
      assert.equal(root.style.visibility, '');
      assert.equal(root.style.position, '');
      assert.equal(root.style.left, '');
      assert.equal(root.style.top, '');
      assert.equal(root.dataset.reorderMotionChip, undefined);
    }
    const next = f.stage.mount(f.prepared);
    assert.ok(next);
    assert.equal(next.node, node);
    assert.equal(removals, 0);
    assert.equal(additions, 0);
    for (const root of roots) assert.ok(next.node.contains(root));
    const read = next.read(f.select);
    assert.equal(read.kind, 'ready');
    if (read.kind !== 'ready') assert.fail('fresh-native-scene-required');
    assert.deepEqual(read.scene.geometry, f.scene.geometry);
    assert.equal(node.style.clip, 'rect(0px, 0px, 0px, 0px)');
    next.reveal();
    assert.notEqual(node.style.clip, 'rect(0px, 0px, 0px, 0px)');
    f.scene.paint({
      progress: 1,
      positions: roots.map((_root, id) => ({ id, left: 999, top: 999 })),
    });
    assert.ok(roots.every((root) => root.style.left === ''));
  } finally {
    try {
      f.stage.destroy();
    } finally {
      f.f.destroy();
    }
  }
});

test('a retiring reset cannot mutate a newer lease reentered into the same retained slot', () => {
  const f = preparedScene();
  try {
    const root = f.scene.chips[0]!.roots[0]!.node;
    let value = root.style.visibility;
    let next: ReturnType<ReorderPassiveStage['mount']> = null;
    Object.defineProperty(root.style, 'visibility', {
      configurable: true,
      get: () => value,
      set: (requested) => {
        value = requested;
        Object.defineProperty(root.style, 'visibility', {
          configurable: true,
          writable: true,
          value,
        });
        f.stage.release(f.pending);
        next = f.stage.mount(f.prepared);
        assert.ok(next);
        root.style.visibility = 'collapse';
        root.style.position = 'absolute';
        root.style.left = '777px';
        root.style.top = '888px';
        root.dataset.reorderMotionChip = 'new-lease';
      },
    });
    f.pending.destroy();
    assert.ok(next);
    assert.equal(root.style.visibility, 'collapse');
    assert.equal(root.style.position, 'absolute');
    assert.equal(root.style.left, '777px');
    assert.equal(root.style.top, '888px');
    assert.equal(root.dataset.reorderMotionChip, 'new-lease');
    f.pending.destroy();
    assert.equal(root.style.left, '777px');
  } finally {
    try {
      f.stage.destroy();
    } finally {
      f.f.destroy();
    }
  }
});

for (const raw of [null, undefined])
  test(`failed passive reset retires the whole captured slot and preserves raw ${String(raw)}`, () => {
    const f = preparedScene();
    try {
      const node = f.pending.node;
      const root = f.scene.chips[0]!.roots[0]!.node;
      Object.defineProperty(root.style, 'visibility', {
        configurable: true,
        get: () => 'visible',
        set: () => {
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw raw;
        },
      });
      let caught = false;
      try {
        f.pending.destroy();
      } catch (cause) {
        caught = true;
        assert.equal(cause, raw);
      }
      assert.equal(caught, true);
      assert.equal(node.isConnected, false);
      assert.equal(root.style.left, '');
      assert.equal(root.style.top, '');
      f.pending.destroy();
    } finally {
      try {
        f.stage.destroy();
      } finally {
        f.f.destroy();
      }
    }
  });

test('reset and terminal cleanup failures keep the retired reservation counted beside its accepted sibling', () => {
  const f = preparedScene();
  try {
    const sibling = f.stage.mount(f.prepared);
    assert.ok(sibling);
    const node = f.pending.node,
      root = f.scene.chips[0]!.roots[0]!.node;
    const cleanup = new Error('controlled-physical-retirement');
    const remove = node.remove.bind(node);
    let attempts = 0;
    node.remove = () => {
      attempts++;
      throw cleanup;
    };
    Object.defineProperty(root.style, 'visibility', {
      configurable: true,
      get: () => 'visible',
      set: () => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw null;
      },
    });
    assert.throws(
      () => f.pending.destroy(),
      (cause) =>
        cause instanceof AggregateError && cause.errors[0] === null && cause.errors[1] === cleanup,
    );
    assert.equal(attempts, 1);
    assert.equal(f.stage.mount(f.prepared) === null, true);
    assert.equal(sibling.current(), true);
    f.pending.destroy();
    assert.equal(attempts, 1);
    node.remove = remove;
  } finally {
    try {
      f.stage.destroy();
    } finally {
      f.f.destroy();
    }
  }
});

for (const raw of [null, undefined])
  test(`failed whole-idle clip retires its exact slot and preserves raw ${String(raw)}`, () => {
    const f = preparedScene();
    try {
      const sibling = f.stage.mount(f.prepared);
      assert.ok(sibling);
      const node = f.pending.node,
        root = f.scene.chips[0]!.roots[0]!.node;
      const set = node.style.setProperty.bind(node.style);
      node.style.setProperty = (name, value, priority) => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        if (name === 'clip') throw raw;
        set(name, value, priority);
      };
      let caught = false;
      try {
        f.pending.destroy();
      } catch (cause) {
        caught = true;
        assert.equal(cause, raw);
      }
      assert.equal(caught, true);
      assert.equal(node.isConnected, false);
      assert.equal(root.style.left, '');
      assert.equal(root.style.top, '');
      assert.equal(sibling.current(), true);
      f.pending.destroy();
    } finally {
      try {
        f.stage.destroy();
      } finally {
        f.f.destroy();
      }
    }
  });

test('an externally detached idle tree is refused before warm canonical mutation', () => {
  const f = preparedScene();
  try {
    const sibling = f.stage.mount(f.prepared);
    assert.ok(sibling);
    const node = f.pending.node;
    f.pending.destroy();
    node.remove();
    const widget = node.querySelector<HTMLElement>('.sp-reorder-preview');
    assert.ok(widget);
    widget.insertBefore = () => {
      assert.fail('detached-idle-must-not-mutate');
    };
    assert.equal(f.stage.mount(f.prepared) === null, true);
    assert.equal(node.isConnected, false);
    assert.equal(sibling.current(), true);
  } finally {
    try {
      f.stage.destroy();
    } finally {
      f.f.destroy();
    }
  }
});
