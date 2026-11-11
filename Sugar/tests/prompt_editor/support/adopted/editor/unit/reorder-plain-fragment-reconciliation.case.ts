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
/** Actual canonical output retains chip resources while genuine Core movement changes separator multiplicities. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  ReorderWidget,
  type PassiveReorderPaint,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { installReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { reorderState } from '../transaction/reorder-fixture.ts';
import { preparedLoraState } from './reorder-passive-reuse-fixture.ts';

test('actual 180-chip separator multiplicity change retains every chip and original canonical line', () => {
  const source = Array.from(
      { length: 180 },
      (_, index) => `value${String(index).padStart(3, '0')}`,
    ).join(', '),
    f = motionPresentationFixture(reorderState(source).state),
    failures: unknown[] = [];
  let paint: PassiveReorderPaint | null = null;
  try {
    const capture = f.input.capture,
      token = f.input.token;
    const firstOrder = moveReorderChip(capture.prepared, capture.prepared.initial, 80, {
      kind: 'row',
      row: 0,
      insertion: 81,
    });
    assert.ok(firstOrder);
    const first = prepareReorderPreview(
      { capture, token, activeChip: 80, order: firstOrder },
      'complete',
    );
    assert.ok(first);
    paint = new ReorderWidget(first, null).mountPassive(f.view);
    f.line.append(paint.node as unknown as (typeof f.chips)[number]);
    const original = paint
      .targets()
      .filter((target) => target.kind === 'chip')
      .map((target) => ({
        id: target.index,
        node: target.node,
        line: target.node.childNodes[0],
      }));
    assert.equal(original.length, 180);
    let order = firstOrder;
    for (const insertion of [82, 83]) {
      const moved = moveReorderChip(capture.prepared, order, 80, {
        kind: 'row',
        row: 0,
        insertion,
      });
      assert.ok(moved);
      order = moved;
    }
    const next = prepareReorderPreview({ capture, token, activeChip: 80, order }, 'complete');
    assert.ok(next);
    assert.equal(paint.reconcile(next), 'ready');
    assert.equal(paint.current(), true);
    assert.equal(paint.node.textContent === next.output.text, true);
    const targets = paint.targets().filter((target) => target.kind === 'chip');
    assert.deepEqual(
      targets.map((target) => target.index),
      order.order,
    );
    for (const old of original) {
      const current = targets.find((target) => target.index === old.id);
      assert.ok(current);
      assert.equal(current.node === old.node, true);
      assert.equal(current.node.childNodes[0] === old.line, true);
    }
    assert.equal(paint.node.querySelectorAll('.sp-reorder-gap').length, 181);
    assert.equal(readExactSource(f.state) === source, true);
    assert.equal(undoDepth(f.state), 0);
    assert.equal(redoDepth(f.state), 0);
  } catch (cause) {
    failures.push(cause);
  }
  for (const release of [() => paint?.destroy(), () => f.destroy()])
    try {
      release();
    } catch (cause) {
      failures.push(cause);
    }
  if (failures.length === 1) throw failures[0];
  if (failures.length) throw new AggregateError(failures, 'plain-fragment-proof-retirement');
});

test('new plain separator occurrences retain the actual whole-fit media registration and original canvases', () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'document'),
    painted: HTMLCanvasElement[] = [],
    failures: unknown[] = [];
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
      capture = f.input.capture,
      token = f.input.token;
    let order = moveReorderChip(capture.prepared, capture.prepared.initial, 20, {
      kind: 'row',
      row: 0,
      insertion: 21,
    });
    assert.ok(order);
    const first = prepareReorderPreview({ capture, token, activeChip: 20, order }, 'complete');
    assert.ok(first);
    f.dispatch({ effects: installReorderPreview.of(first) });
    assert.equal(new ReorderWidget(first, null).updateDOM(f.root, f.view), true);
    const prepared = preparePassiveMotion(f.view, 20);
    assert.ok(prepared);
    stage = new ReorderPassiveStage(f.view, prepared);
    const child = stage.mount(prepared);
    assert.ok(child);
    const shell = child.node,
      canvases = Array.from(shell.querySelectorAll<HTMLCanvasElement>('.sp-lora-media')),
      rich = Array.from(shell.querySelectorAll('.sp-lora-chip')),
      paintCount = painted.length;
    assert.equal(canvases.length, 45);
    f.paint.admitLayer(stage);
    const placement = child.read(() => []);
    assert.equal(placement.kind, 'placement');
    if (placement.kind !== 'placement') assert.fail('actual-calibration-required');
    child.place(placement);
    const initialRead = child.read(() => []);
    assert.equal(initialRead.kind, 'ready');
    if (initialRead.kind !== 'ready') assert.fail('actual-first-read-required');
    child.destroy();
    for (const insertion of [22, 23]) {
      const moved = moveReorderChip(capture.prepared, order, 20, {
        kind: 'row',
        row: 0,
        insertion,
      });
      assert.ok(moved);
      order = moved;
    }
    const next = prepareReorderPreview({ capture, token, activeChip: 20, order }, 'complete');
    assert.ok(next);
    f.dispatch({ effects: installReorderPreview.of(next) });
    assert.equal(new ReorderWidget(next, null).updateDOM(f.root, f.view), true);
    const updated = preparePassiveMotion(f.view, 20);
    assert.ok(updated);
    const reused = stage.mount(updated);
    assert.ok(reused);
    assert.equal(reused.node === shell, true);
    assert.equal(reused.current(), true);
    assert.equal(child.current(), false);
    const freshRead = reused.read(() => []);
    assert.equal(freshRead.kind, 'ready');
    if (freshRead.kind !== 'ready') assert.fail('actual-fresh-read-required');
    assert.equal(freshRead.scene.geometry === initialRead.scene.geometry, false);
    const currentCanvases = Array.from(shell.querySelectorAll('.sp-lora-media')),
      currentRich = Array.from(shell.querySelectorAll('.sp-lora-chip'));
    assert.equal(currentCanvases.length, canvases.length);
    assert.equal(new Set(currentCanvases).size, 45);
    assert.ok(canvases.every((canvas) => currentCanvases.includes(canvas)));
    assert.equal(currentRich.length, rich.length);
    assert.ok(rich.every((node) => currentRich.includes(node)));
    assert.equal(painted.length, paintCount);
    assert.ok(canvases.every((canvas) => canvas.width === 256 && canvas.height === 256));
    assert.equal(readExactSource(f.state) === source, true);
    assert.equal(f.state.selection, selection);
    assert.equal(undoDepth(f.state), 0);
    assert.equal(redoDepth(f.state), 0);
    stage.destroy();
    assert.ok(canvases.every((canvas) => canvas.width === 0 && canvas.height === 0));
  } catch (cause) {
    failures.push(cause);
  }
  for (const release of [
    () => stage?.destroy(),
    () => f?.destroy(),
    () => {
      if (descriptor) Object.defineProperty(globalThis, 'document', descriptor);
      else Reflect.deleteProperty(globalThis, 'document');
    },
  ])
    try {
      release();
    } catch (cause) {
      failures.push(cause);
    }
  if (failures.length === 1) throw failures[0];
  if (failures.length) throw new AggregateError(failures, 'plain-fragment-media-proof-retirement');
});

test('classification reentry preserves a newer same-preview canonical publication and retires the older update', () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'document'),
    failures: unknown[] = [];
  let f: ReturnType<typeof motionPresentationFixture> | null = null,
    paint: PassiveReorderPaint | null = null;
  let restore = (): void => {};
  try {
    f = motionPresentationFixture(
      preparedLoraState(() => {}, 45, 16, 8),
      (document) =>
        Object.defineProperty(globalThis, 'document', {
          configurable: true,
          value: document,
        }),
    );
    const capture = f.input.capture,
      token = f.input.token;
    let order = moveReorderChip(capture.prepared, capture.prepared.initial, 20, {
      kind: 'row',
      row: 0,
      insertion: 21,
    });
    assert.ok(order);
    const first = prepareReorderPreview({ capture, token, activeChip: 20, order }, 'complete');
    assert.ok(first);
    paint = new ReorderWidget(first, null).mountPassive(f.view);
    f.line.append(paint.node as unknown as (typeof f.chips)[number]);
    const originalNodes = paint.targets().map((target) => target.node),
      originalText = paint.node.textContent;
    for (const insertion of [22, 23]) {
      const moved = moveReorderChip(capture.prepared, order, 20, {
        kind: 'row',
        row: 0,
        insertion,
      });
      assert.ok(moved);
      order = moved;
    }
    const next = prepareReorderPreview({ capture, token, activeChip: 20, order }, 'complete');
    assert.ok(next);
    const ordinary = capture.projection!.ordinary,
      own = Object.getOwnPropertyDescriptor(ordinary, 'between'),
      original = ordinary.between,
      root = paint.node,
      view = f.view;
    let entered = false,
      accepted = false;
    Object.defineProperty(ordinary, 'between', {
      configurable: true,
      /** Enter through the actual ordinary projection read while preserving the newer canonical publication. */
      value(...args: Parameters<typeof ordinary.between>) {
        if (!entered) {
          entered = true;
          accepted = new ReorderWidget(first, null).updateDOM(root, view);
        }
        return original.apply(ordinary, args);
      },
    });
    restore = () => {
      if (own) Object.defineProperty(ordinary, 'between', own);
      else Reflect.deleteProperty(ordinary, 'between');
    };
    assert.equal(paint.reconcile(next), 'retired');
    assert.equal(entered, true);
    assert.equal(accepted, true);
    assert.equal(paint.current(), true);
    assert.equal(paint.node.textContent === originalText, true);
    const current = paint.targets().map((target) => target.node);
    assert.equal(current.length, originalNodes.length);
    assert.ok(current.every((node, index) => node === originalNodes[index]));
  } catch (cause) {
    failures.push(cause);
  }
  for (const release of [
    restore,
    () => paint?.destroy(),
    () => f?.destroy(),
    () => {
      if (descriptor) Object.defineProperty(globalThis, 'document', descriptor);
      else Reflect.deleteProperty(globalThis, 'document');
    },
  ])
    try {
      release();
    } catch (cause) {
      failures.push(cause);
    }
  if (failures.length === 1) throw failures[0];
  if (failures.length)
    throw new AggregateError(failures, 'plain-fragment-reentry-proof-retirement');
});
