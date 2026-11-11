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
/** Real canonical and borrowed-raster owners expose the exact A/B settlement-to-overlap lifecycle. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import {
  ReadonlyWidgetMedia,
  type ReadonlyMediaPlan,
} from '../../../../../../frontend/sugarprompt/editor/src/readonly-widget-media.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { preparedLoraState } from './reorder-passive-reuse-fixture.ts';

test('settled A and B retain both exact canonical and raster identities through later overlap within the original cap', () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'document'),
    causes: unknown[] = [];
  const preflight = ReadonlyWidgetMedia.prototype.preflightAll,
    plans: ReadonlyMediaPlan[] = [];
  /** Observe each actual admitted plan without changing canonical media policy or hydration. */
  ReadonlyWidgetMedia.prototype.preflightAll = function (this: ReadonlyWidgetMedia) {
    const result = preflight.call(this);
    if (result.kind === 'ready') plans.push(result.plan);
    return result;
  };
  let paints = 0,
    f: ReturnType<typeof motionPresentationFixture> | null = null,
    stage: ReorderPassiveStage | null = null;
  try {
    f = motionPresentationFixture(
      preparedLoraState(() => {
        paints++;
      }),
      (document) => {
        Object.defineProperty(globalThis, 'document', {
          configurable: true,
          value: document,
        });
      },
    );
    const source = readExactSource(f.state),
      selection = f.state.selection,
      prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared);
    stage = new ReorderPassiveStage(f.view, prepared);
    const a = stage.mount(prepared),
      b = stage.mount(prepared);
    assert.ok(a && b);
    const aNode = a.node,
      bNode = b.node,
      aCanonical = aNode.querySelector('.sp-reorder-preview'),
      bCanonical = bNode.querySelector('.sp-reorder-preview'),
      aRich = aNode.querySelector('.sp-lora-chip'),
      bRich = bNode.querySelector('.sp-lora-chip'),
      aBuffer = aNode.querySelector<HTMLCanvasElement>('.sp-lora-media'),
      bBuffer = bNode.querySelector<HTMLCanvasElement>('.sp-lora-media');
    assert.ok(aCanonical && bCanonical && aRich && bRich && aBuffer && bBuffer);
    assert.equal(paints, 3);
    assert.equal(stage.mount(prepared) === null, true);
    b.destroy();
    a.destroy();
    assert.equal(plans.length, 2);
    assert.equal(
      plans.every((plan) => plan.hydrate() === false),
      true,
    );
    assert.equal(aNode.isConnected, true);
    assert.equal(bNode.isConnected, true);
    assert.deepEqual(
      [aBuffer.width, aBuffer.height, bBuffer.width, bBuffer.height],
      [16, 8, 16, 8],
    );
    const resumed = stage.mount(prepared),
      overlap = stage.mount(prepared);
    assert.ok(resumed && overlap);
    assert.equal(resumed.node === aNode, true);
    assert.equal(overlap.node === bNode, true);
    assert.equal(overlap.node === aNode, false);
    assert.equal(paints, 3);
    assert.equal(stage.mount(prepared) === null, true);
    assert.equal(stage.node.querySelectorAll('.sp-reorder-motion-paint').length, 2);
    assert.equal(resumed.node.querySelector('.sp-reorder-preview') === aCanonical, true);
    assert.equal(resumed.node.querySelector('.sp-lora-chip') === aRich, true);
    assert.equal(resumed.node.querySelector('.sp-lora-media') === aBuffer, true);
    assert.equal(overlap.node.querySelector('.sp-reorder-preview') === bCanonical, true);
    assert.equal(overlap.node.querySelector('.sp-lora-chip') === bRich, true);
    assert.equal(overlap.node.querySelector('.sp-lora-media') === bBuffer, true);
    a.destroy();
    b.destroy();
    assert.equal(resumed.current(), true);
    assert.equal(overlap.current(), true);
    const overlapBuffer = overlap.node.querySelector<HTMLCanvasElement>('.sp-lora-media');
    assert.ok(overlapBuffer);
    stage.destroy();
    assert.deepEqual(
      [
        aBuffer.width,
        aBuffer.height,
        bBuffer.width,
        bBuffer.height,
        overlapBuffer.width,
        overlapBuffer.height,
      ],
      [0, 0, 0, 0, 0, 0],
    );
    assert.equal(aNode.isConnected || bNode.isConnected || overlap.node.isConnected, false);
    assert.equal(readExactSource(f.state) === source, true);
    assert.equal(f.state.selection === selection, true);
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
  } catch (cause) {
    causes.push(cause);
  }
  for (const release of [
    () => stage?.destroy(),
    () => {
      ReadonlyWidgetMedia.prototype.preflightAll = preflight;
    },
    () => f?.destroy(),
    () => {
      if (previous) Object.defineProperty(globalThis, 'document', previous);
      else Reflect.deleteProperty(globalThis, 'document');
    },
  ]) {
    try {
      release();
    } catch (cause) {
      causes.push(cause);
    }
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'idle-pair-proof-retirement');
});
for (const change of ['wrapper-key', 'context', 'source'] as const)
  test(`two idle canonical media owners reject ${change} changes and both retire with their Stage`, () => {
    const previous = Object.getOwnPropertyDescriptor(globalThis, 'document'),
      causes: unknown[] = [];
    let f: ReturnType<typeof motionPresentationFixture> | null = null,
      stage: ReorderPassiveStage | null = null;
    try {
      f = motionPresentationFixture(
        preparedLoraState(() => {}),
        (document) => {
          Object.defineProperty(globalThis, 'document', {
            configurable: true,
            value: document,
          });
        },
      );
      const prepared = preparePassiveMotion(f.view, 0);
      assert.ok(prepared);
      stage = new ReorderPassiveStage(f.view, prepared);
      const a = stage.mount(prepared),
        b = stage.mount(prepared);
      assert.ok(a && b);
      const nodes = [a.node, b.node],
        buffers = nodes.map((node) => node.querySelector<HTMLCanvasElement>('.sp-lora-media'));
      assert.equal(
        buffers.every((buffer) => buffer !== null),
        true,
      );
      b.destroy();
      a.destroy();
      assert.equal(
        nodes.every((node) => node.isConnected),
        true,
      );
      if (change === 'wrapper-key') {
        const changed = Object.freeze({
          ...prepared,
          contentCss: prepared.contentCss + 'color: blue;',
        });
        const next = stage.mount(changed);
        assert.ok(next);
        assert.equal(
          nodes.some((node) => node === next.node),
          false,
        );
        assert.equal(
          nodes.some((node) => node.isConnected),
          false,
        );
        assert.equal(
          buffers.every((buffer) => buffer?.width === 0 && buffer.height === 0),
          true,
        );
        assert.equal(stage.current(), true);
      } else {
        if (change === 'context') f.view.dom.className += ' changed-context';
        else f.dispatch({ changes: { from: 0, insert: 'x' } });
        assert.equal(stage.mount(prepared) === null, true);
        assert.equal(stage.current(), false);
        assert.equal(
          buffers.every((buffer) => buffer?.width === 16 && buffer.height === 8),
          true,
        );
      }
      stage.destroy();
      assert.equal(
        nodes.some((node) => node.isConnected),
        false,
      );
      assert.equal(
        buffers.every((buffer) => buffer?.width === 0 && buffer.height === 0),
        true,
      );
    } catch (cause) {
      causes.push(cause);
    }
    for (const release of [
      () => stage?.destroy(),
      () => f?.destroy(),
      () => {
        if (previous) Object.defineProperty(globalThis, 'document', previous);
        else Reflect.deleteProperty(globalThis, 'document');
      },
    ]) {
      try {
        release();
      } catch (cause) {
        causes.push(cause);
      }
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'idle-pair-rejection-retirement');
  });
