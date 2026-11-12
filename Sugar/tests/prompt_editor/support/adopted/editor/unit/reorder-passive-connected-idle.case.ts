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
/** Whole idle output retains physically closed connected canonical resources under its original finite Slot. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { createReorderApplicationHold } from '../../../../../../frontend/sugarprompt/editor/src/reorder-session-application.ts';
import {
  publishReorderPreview,
  reorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { ReorderWidget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { wholeFixture } from './reorder-pointer-stage-fixture.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { preparedLoraState } from './reorder-passive-reuse-fixture.ts';

/** Preserve an existing body cause before independently attempting every cleanup, including raw null/undefined. */
function retire(actions: readonly (() => void)[], causes: unknown[] = []): void {
  for (const action of actions)
    try {
      action();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'connected-slot-proof-retirement-failed');
}

/** One proof owner preserves the body failure first and attempts every independent terminal action. */
function complete(body: () => void, cleanup: readonly (() => void)[]): void {
  const causes: unknown[] = [];
  try {
    body();
  } catch (cause) {
    causes.push(cause);
  }
  retire(cleanup, causes);
}

/** Actual pointer publication supplies whole policy; this helper does not synthesize a Prepared context or lease. */
function wholeStage() {
  const f = wholeFixture();
  let stage: ReorderPassiveStage | null = null;
  try {
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared?.whole);
    stage = new ReorderPassiveStage(f.view, prepared);
    const pending = stage.mount(prepared);
    assert.ok(pending);
    return { f, prepared, stage, pending };
  } catch (primary) {
    try {
      retire([() => stage?.destroy(), () => f.destroy()]);
    } catch (cleanup) {
      throw new AggregateError([primary, cleanup], 'CONNECTED_IDLE_ACQUISITION_FAILED');
    }
    throw primary;
  }
}
test('whole idle release and warm reuse keep the exact two shells connected without native reparenting', () => {
  const { f, prepared, stage, pending } = wholeStage();
  const node = pending.node,
    append = stage.node.append.bind(stage.node),
    remove = node.remove.bind(node);
  let additions = 0,
    removals = 0,
    other: HTMLElement | null = null;
  complete(() => {
    const second = stage.mount(prepared);
    assert.ok(second);
    other = second.node;
    stage.node.append = (...nodes) => {
      ++additions;
      append(...nodes);
    };
    node.remove = () => {
      ++removals;
      remove();
    };
    pending.destroy();
    assert.equal(node.isConnected, true);
    assert.equal(node.parentElement, stage.node);
    assert.equal(node.style.getPropertyPriority('clip'), 'important');
    assert.equal(node.style.clip, 'rect(0px, 0px, 0px, 0px)');
    assert.equal(node.dataset.reorderMotionProgress, undefined);
    assert.equal(node.dataset.reorderMotionEndpoint, undefined);
    assert.equal(node.querySelectorAll('[data-reorder-motion-chip]').length, 0);
    assert.equal(second.current(), true);
    const reused = stage.mount(prepared);
    assert.ok(reused);
    assert.equal(reused.node, node);
    assert.equal(reused.current(), true);
    assert.equal(second.node, other);
    assert.equal(stage.mount(prepared), null);
    pending.destroy();
    assert.equal(reused.current(), true);
    assert.deepEqual([additions, removals], [0, 0]);
  }, [
    () => {
      stage.node.append = append;
    },
    () => {
      node.remove = remove;
    },
    () => stage.destroy(),
    () => f.destroy(),
    () => assert.equal(node.isConnected, false),
    () => {
      if (other) assert.equal(other.isConnected, false);
    },
  ]);
});

for (const frontier of ['before', 'after'] as const)
  test(`whole ${frontier}-delegate close holds its physical reservation through native setter reentry`, () => {
    const { f, prepared, stage, pending } = wholeStage();
    const node = pending.node,
      setProperty = node.style.setProperty.bind(node.style);
    const admitted: { newer: ReturnType<ReorderPassiveStage['mount']> } = {
      newer: null,
    };
    let entered = false;
    complete(() => {
      node.style.setProperty = (name, value, priority) => {
        if (name !== 'clip' || entered) {
          setProperty(name, value, priority);
          return;
        }
        entered = true;
        if (frontier === 'after') setProperty(name, value, priority);
        admitted.newer = stage.mount(prepared);
        assert.ok(admitted.newer);
        assert.notEqual(admitted.newer.node, node);
        assert.equal(stage.mount(prepared), null);
        if (frontier === 'before') setProperty(name, value, priority);
      };
      pending.destroy();
      assert.equal(entered, true);
      assert.ok(admitted.newer);
      assert.equal(admitted.newer.current(), true);
      assert.equal(node.isConnected, true);
      assert.equal(node.parentElement, stage.node);
      assert.equal(node.style.clip, 'rect(0px, 0px, 0px, 0px)');
      node.style.setProperty = setProperty;
      const reused = stage.mount(prepared);
      assert.ok(reused);
      assert.equal(reused.node, node);
      assert.equal(reused.current(), true);
      assert.equal(stage.mount(prepared), null);
    }, [
      () => {
        node.style.setProperty = setProperty;
      },
      () => stage.destroy(),
      () => f.destroy(),
    ]);
  });

for (const failure of [null, undefined])
  test(`whole raw ${String(failure)} close failure retires its captured shell and preserves the cause`, () => {
    const { f, stage, pending } = wholeStage();
    const node = pending.node,
      setProperty = node.style.setProperty.bind(node.style);
    let attempted = false;
    complete(() => {
      node.style.setProperty = (name, value, priority) => {
        setProperty(name, value, priority);
        if (name === 'clip') {
          attempted = true;
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw failure;
        }
      };
      let caught = false;
      try {
        pending.destroy();
      } catch (cause) {
        caught = true;
        assert.equal(cause, failure);
      }
      assert.equal(caught, true);
      assert.equal(attempted, true);
      assert.equal(node.isConnected, false);
      assert.equal(node.parentElement, null);
    }, [
      () => {
        node.style.setProperty = setProperty;
      },
      () => stage.destroy(),
      () => f.destroy(),
    ]);
  });

test('externally detached whole idle output retires before canonical reconciliation', () => {
  const { f, prepared, stage, pending } = wholeStage();
  const node = pending.node;
  complete(() => {
    pending.destroy();
    assert.equal(node.isConnected, true);
    node.remove();
    const rich = node.querySelector<HTMLElement>('[data-reorder-chip]');
    assert.ok(rich);
    const parent = rich.parentElement!;
    parent.insertBefore = () => {
      throw new Error('DETACHED_CANONICAL_MUTATION');
    };
    assert.equal(stage.mount(prepared), null);
    assert.equal(node.isConnected, false);
    assert.equal(node.parentElement, null);
  }, [() => stage.destroy(), () => f.destroy()]);
});
test('whole pointer idle reuse retains 45 actual canvases and rich nodes without repainting borrowed rasters', () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'document'),
    painted: HTMLCanvasElement[] = [];
  let f: ReturnType<typeof motionPresentationFixture> | null = null,
    stage: ReorderPassiveStage | null = null;
  complete(() => {
    f = motionPresentationFixture(
      preparedLoraState((canvas) => painted.push(canvas), 45, 256, 256),
      (document) =>
        Object.defineProperty(globalThis, 'document', {
          configurable: true,
          value: document,
        }),
    );
    const source = readExactSource(f.state),
      selection = f.state.selection,
      pickup = f.state.field(reorderPreview).value!,
      hold = createReorderApplicationHold(pickup.order);
    /** Each proposal consumes the actual Core result and supported pointer publication owner. */
    const publish = (insertion: number): void => {
      const current = f!.state.field(reorderPreview).value!,
        order = moveReorderChip(current.capture.prepared, current.order, 0, {
          kind: 'row',
          row: 0,
          insertion,
        });
      assert.ok(order);
      assert.equal(
        publishReorderPreview(f!.view, { ...current, order }, 'pointer', hold, () => true),
        'published',
      );
      assert.equal(
        new ReorderWidget(f!.state.field(reorderPreview).paint!, null).updateDOM(f!.root, f!.view),
        true,
      );
      Object.assign(f!.input, { pointer: 1, order });
    };
    publish(2);
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared?.whole);
    stage = new ReorderPassiveStage(f.view, prepared);
    const a = stage.mount(prepared),
      b = stage.mount(prepared);
    assert.ok(a && b);
    const shell = a.node,
      rich = Array.from(shell.querySelectorAll('.sp-lora-chip')),
      buffers = Array.from(shell.querySelectorAll<HTMLCanvasElement>('.sp-lora-media')),
      other = Array.from(b.node.querySelectorAll<HTMLCanvasElement>('.sp-lora-media'));
    assert.equal(buffers.length, 45);
    assert.equal(other.length, 45);
    assert.equal(new Set([...buffers, ...other]).size, 90);
    assert.equal(painted.length, 135);
    assert.equal(
      [...buffers, ...other].reduce((sum, canvas) => sum + canvas.width * canvas.height, 0),
      5898240,
    );
    assert.equal(stage.mount(prepared), null);
    const geometry = [shell, ...shell.querySelectorAll<HTMLElement>('*')].map((node) => ({
      node,
      bounds: node.getBoundingClientRect,
      fragments: node.getClientRects,
    }));
    for (const item of geometry) {
      item.node.getBoundingClientRect = () => {
        throw new Error('connected-mutation-layout-read');
      };
      item.node.getClientRects = () => {
        throw new Error('connected-mutation-fragment-read');
      };
    }
    a.destroy();
    b.destroy();
    assert.equal(shell.isConnected, true);
    assert.equal(shell.parentElement, stage.node);
    assert.ok(buffers.every((canvas) => canvas.width === 256 && canvas.height === 256));
    publish(5);
    const next = preparePassiveMotion(f.view, 0);
    assert.ok(next?.whole);
    const returned = stage.mount(next),
      parallel = stage.mount(next);
    assert.ok(returned && parallel);
    assert.equal(returned.node, shell);
    assert.equal(returned.current(), true);
    assert.equal(parallel.node, b.node);
    assert.deepEqual(new Set(shell.querySelectorAll('.sp-lora-chip')), new Set(rich));
    assert.deepEqual(new Set(shell.querySelectorAll('.sp-lora-media')), new Set(buffers));
    assert.equal(painted.length, 135);
    for (const item of geometry) {
      item.node.getBoundingClientRect = item.bounds;
      item.node.getClientRects = item.fragments;
    }
    assert.equal(stage.mount(next), null);
    assert.equal(readExactSource(f.state), source);
    assert.equal(f.state.selection, selection);
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
    stage.destroy();
    assert.ok([...buffers, ...other].every((canvas) => canvas.width === 0 && canvas.height === 0));
  }, [
    () => stage?.destroy(),
    () => f?.destroy(),
    () => {
      if (previous) Object.defineProperty(globalThis, 'document', previous);
      else Reflect.deleteProperty(globalThis, 'document');
    },
  ]);
});
