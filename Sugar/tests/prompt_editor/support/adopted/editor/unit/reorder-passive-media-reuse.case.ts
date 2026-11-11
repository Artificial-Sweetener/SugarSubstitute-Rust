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
/** Actual whole-mount prepared media and source-neutral Core orders exercise finite canonical paint reuse. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { ReorderWidget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { installReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { preparedLoraState } from './reorder-passive-reuse-fixture.ts';

test('45 whole-fit borrowed rasters retain their actual buffers and rich nodes across genuine Core order changes', () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'document'),
    painted: HTMLCanvasElement[] = [];
  const causes: unknown[] = [];
  let f: ReturnType<typeof motionPresentationFixture> | null = null,
    stage: ReorderPassiveStage | null = null;
  try {
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
      prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared);
    stage = new ReorderPassiveStage(f.view, prepared);
    const first = stage.mount(prepared);
    assert.ok(first);
    const shell = first.node,
      buffers = Array.from(shell.querySelectorAll<HTMLCanvasElement>('.sp-lora-media')),
      rich = Array.from(shell.querySelectorAll('.sp-lora-chip'));
    assert.equal(buffers.length, 45);
    assert.equal(new Set(buffers).size, 45);
    assert.ok(buffers.every((canvas) => canvas.width === 256 && canvas.height === 256));
    assert.equal(
      buffers.reduce((sum, canvas) => sum + canvas.width * canvas.height, 0),
      2949120,
    );
    assert.deepEqual(painted.slice(45), buffers);
    const parallel = stage.mount(prepared);
    assert.ok(parallel);
    const parallelBuffers = Array.from(
      parallel.node.querySelectorAll<HTMLCanvasElement>('.sp-lora-media'),
    );
    assert.equal(parallelBuffers.length, 45);
    assert.equal(new Set([...buffers, ...parallelBuffers]).size, 90);
    assert.equal(
      [...buffers, ...parallelBuffers].reduce(
        (sum, canvas) => sum + canvas.width * canvas.height,
        0,
      ),
      5898240,
    );
    assert.equal(stage.mount(prepared), null);
    assert.equal(painted.length, 135);
    parallel.destroy();
    first.destroy();
    assert.ok(parallelBuffers.every((canvas) => canvas.width === 256 && canvas.height === 256));
    const order = moveReorderChip(f.input.capture.prepared, f.input.order, 0, {
      kind: 'row',
      row: 0,
      insertion: 5,
    });
    assert.ok(order);
    const preview = prepareReorderPreview(
      { token: f.input.token, capture: f.input.capture, activeChip: 0, order },
      'complete',
    );
    assert.ok(preview);
    f.dispatch({ effects: installReorderPreview.of(preview) });
    assert.equal(new ReorderWidget(preview, null).updateDOM(f.root, f.view), true);
    const next = preparePassiveMotion(f.view, 0);
    assert.ok(next);
    const second = stage.mount(next);
    assert.ok(second);
    assert.ok(second.node === shell);
    assert.equal(painted.length, 135);
    assert.deepEqual(new Set(shell.querySelectorAll('.sp-lora-chip')), new Set(rich));
    assert.deepEqual(new Set(shell.querySelectorAll('.sp-lora-media')), new Set(buffers));
    const other = stage.mount(next);
    assert.ok(other);
    assert.equal(other.node === parallel.node, true);
    assert.equal(stage.mount(next) === null, true);
    assert.equal(painted.length, 135);
    assert.equal(new Set(other.node.querySelectorAll('.sp-lora-media')).size, 45);
    assert.equal(
      parallelBuffers.every((canvas) => other.node.contains(canvas)),
      true,
    );
    assert.equal(
      parallelBuffers.every((canvas) => canvas.width === 256 && canvas.height === 256),
      true,
    );
    assert.equal(first.current(), false);
    assert.equal(first.retained(), false);
    first.destroy();
    assert.equal(second.current(), true);
    assert.equal(second.node.querySelectorAll('[data-reorder-motion-chip]').length, 0);
    stage.destroy();
    assert.ok(buffers.every((canvas) => canvas.width === 0 && canvas.height === 0));
    assert.equal(
      parallelBuffers.every((canvas) => canvas.width === 0 && canvas.height === 0),
      true,
    );
    assert.equal(readExactSource(f.state), source);
    assert.equal(f.state.selection, selection);
    assert.equal(undoDepth(f.state), 0);
    assert.equal(redoDepth(f.state), 0);
  } catch (cause) {
    causes.push(cause);
  } finally {
    for (const release of [
      () => stage?.destroy(),
      () => f?.destroy(),
      () => {
        if (previous) Object.defineProperty(globalThis, 'document', previous);
        else Reflect.deleteProperty(globalThis, 'document');
      },
    ])
      try {
        release();
      } catch (cause) {
        causes.push(cause);
      }
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'reuse-proof-retirement');
});

test('overbudget viewport-only media never recycles a partial or unhydrated registration owner', () => {
  const causes: unknown[] = [];
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'document');
  let paints = 0,
    f: ReturnType<typeof motionPresentationFixture> | null = null,
    stage: ReorderPassiveStage | null = null;
  try {
    f = motionPresentationFixture(
      preparedLoraState(
        () => {
          paints++;
        },
        20,
        512,
        512,
      ),
      (document) =>
        Object.defineProperty(globalThis, 'document', {
          configurable: true,
          value: document,
        }),
    );
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared);
    stage = new ReorderPassiveStage(f.view, prepared);
    const first = stage.mount(prepared);
    assert.ok(first);
    const old = first.node;
    assert.equal(old.querySelectorAll('.sp-lora-media').length, 0);
    first.destroy();
    const second = stage.mount(prepared);
    assert.ok(second);
    assert.ok(second.node !== old);
    assert.equal(second.node.querySelectorAll('.sp-lora-media').length, 0);
    assert.equal(paints, 20);
  } catch (cause) {
    causes.push(cause);
  } finally {
    for (const release of [
      () => stage?.destroy(),
      () => f?.destroy(),
      () => {
        if (previous) Object.defineProperty(globalThis, 'document', previous);
        else Reflect.deleteProperty(globalThis, 'document');
      },
    ])
      try {
        release();
      } catch (cause) {
        causes.push(cause);
      }
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'partial-media-proof-retirement');
});

for (const frontier of ['shell', 'canonical-backing'] as const)
  test(`ready canonical construction retirement at ${frontier} returns null without repaint or invariant failure`, () => {
    const previous = Object.getOwnPropertyDescriptor(globalThis, 'document');
    let paints = 0,
      f: ReturnType<typeof motionPresentationFixture> | null = null,
      stage: ReorderPassiveStage | null = null;
    const causes: unknown[] = [];
    try {
      f = motionPresentationFixture(
        preparedLoraState(
          () => {
            paints++;
          },
          45,
          256,
          256,
        ),
        (document) =>
          Object.defineProperty(globalThis, 'document', {
            configurable: true,
            value: document,
          }),
      );
      const prepared = preparePassiveMotion(f.view, 0);
      assert.ok(prepared);
      stage = new ReorderPassiveStage(f.view, prepared);
      const owner = stage,
        create = f.paint.document.createElement.bind(f.paint.document);
      let retired = false;
      const created: HTMLElement[] = [];
      f.paint.document.createElement = (name) => {
        const node = create(name);
        created.push(node as unknown as HTMLElement);
        if (frontier === 'shell' && !retired) {
          retired = true;
          owner.destroy();
        }
        if (frontier === 'canonical-backing') {
          let value = node.className;
          Object.defineProperty(node, 'className', {
            configurable: true,
            get: () => value,
            set(next: string) {
              value = next;
              if (next === 'sp-lora-backing' && !retired) {
                retired = true;
                owner.destroy();
              }
            },
          });
        }
        return node;
      };
      assert.equal(owner.mount(prepared), null);
      assert.equal(retired, true);
      assert.equal(paints, 45);
      assert.ok(created.every((node) => !node.isConnected));
      assert.equal(owner.mount(prepared), null);
    } catch (cause) {
      causes.push(cause);
    } finally {
      for (const release of [
        () => stage?.destroy(),
        () => f?.destroy(),
        () => {
          if (previous) Object.defineProperty(globalThis, 'document', previous);
          else Reflect.deleteProperty(globalThis, 'document');
        },
      ])
        try {
          release();
        } catch (cause) {
          causes.push(cause);
        }
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'construction-proof-retirement');
  });
