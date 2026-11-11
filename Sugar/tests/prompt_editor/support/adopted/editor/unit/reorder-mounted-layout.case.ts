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
/** Actual mounted layout memoization consumes finite observed native operands, never source-derived size estimates. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { EditorView } from '@codemirror/view';
import { ReorderMountedLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-layout.ts';
import type { MountedReorderTarget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-target.ts';
import { ReorderLineLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-line-layout.ts';
import { SourceLineOrigins } from '../../../../../../frontend/sugarprompt/editor/src/source-line-origins.ts';
import { serializeValueReorder } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { captureReorder, reorderState } from '../transaction/reorder-fixture.ts';

/** Only browser style/rectangle transport is controlled; the real layout owner issues every retained result. */
function specimen() {
  let width = 240.375,
    font = '14px Segoe UI',
    connected = true;
  let rects = [
    {
      left: 10.125,
      right: 35.375,
      top: 20.25,
      bottom: 40.625,
      width: 25.25,
      height: 20.375,
    },
  ];
  const line = { getBoundingClientRect: () => ({ left: 0, top: 0, width }) };
  const node = {
    get isConnected() {
      return connected;
    },
    closest: () => line,
    getBoundingClientRect: () => {
      const left = Math.min(...rects.map((rect) => rect.left)),
        right = Math.max(...rects.map((rect) => rect.right)),
        top = Math.min(...rects.map((rect) => rect.top)),
        bottom = Math.max(...rects.map((rect) => rect.bottom));
      return {
        left,
        right,
        top,
        bottom,
        width: right - left,
        height: bottom - top,
      };
    },
    getClientRects: () => rects,
    remove() {},
  } as unknown as HTMLElement;
  const view = {
    themeClasses: 'native-theme',
    contentDOM: {},
    dom: {
      ownerDocument: {
        defaultView: {
          getComputedStyle: () => ({
            font,
            width: `${width}px`,
            height: '20.375px',
            direction: 'ltr',
            whiteSpace: 'pre-wrap',
            lineHeight: '20.375px',
          }),
        },
      },
    },
  } as unknown as EditorView;
  const target: MountedReorderTarget = {
    node,
    kind: 'chip',
    index: 7,
    leases: [],
    origins: [1],
  };
  return {
    owner: new ReorderMountedLayout(view),
    target,
    resize() {
      width = 241.375;
    },
    restoreWidth() {
      width = 240.375;
    },
    setWidth(next: number) {
      width = next;
    },
    changeFont() {
      font = '15px Segoe UI';
    },
    detach() {
      connected = false;
    },
    rewrap() {
      rects = [
        rects[0]!,
        {
          ...rects[0]!,
          left: 0,
          right: 14.125,
          width: 14.125,
          top: 40.625,
          bottom: 61,
        },
      ];
    },
    multipleFragments() {
      rects = [
        rects[0]!,
        {
          ...rects[0]!,
          left: 12,
          right: 30,
          width: 18,
          top: 40.625,
          bottom: 61,
        },
        { ...rects[0]!, left: 10.125, right: 35.375, top: 61, bottom: 81.375 },
      ];
    },
    changeMiddleFragment() {
      rects = [rects[0]!, { ...rects[1]!, left: 14, right: 32 }, rects[2]!];
    },
  };
}

test('the same native context and a genuine revisit retain the exact immutable measured output', () => {
  const f = specimen(),
    first = f.owner.measure([f.target])!;
  assert.deepEqual(first, [
    {
      kind: 'chip',
      index: 7,
      left: 10.125,
      right: 35.375,
      top: 20.25,
      bottom: 40.625,
    },
  ]);
  assert.equal(f.owner.measure([f.target]), first);
  f.resize();
  const resized = f.owner.measure([f.target])!;
  assert.notEqual(resized, first);
  f.restoreWidth();
  assert.equal(f.owner.measure([f.target]), first);
  f.changeFont();
  assert.notEqual(f.owner.measure([f.target]), first);
  f.owner.destroy();
  assert.equal(f.owner.measure([f.target]), null);
});

test('changed canonical hard-line context and wrapping admit all newly measured physical fragments', () => {
  const f = specimen(),
    first = f.owner.measure([f.target])!;
  f.rewrap();
  const next = f.owner.measure([{ ...f.target, origins: [2] }])!;
  assert.notEqual(next, first);
  assert.deepEqual(next, [
    first[0]!,
    { kind: 'chip', index: 7, left: 0, right: 14.125, top: 40.625, bottom: 61 },
  ]);
  f.detach();
  assert.equal(f.owner.measure([f.target]), null);
  f.owner.destroy();
});

test('a changed middle physical fragment cannot reuse unchanged union and first-fragment geometry', () => {
  const f = specimen();
  f.multipleFragments();
  const first = f.owner.measure([f.target])!;
  assert.equal(f.owner.measure([f.target]), first);
  const bounds = f.target.node.getBoundingClientRect();
  f.changeMiddleFragment();
  assert.deepEqual(f.target.node.getBoundingClientRect(), bounds);
  const next = f.owner.measure([f.target])!;
  assert.notEqual(next, first);
  assert.deepEqual(next, [
    first[0]!,
    { kind: 'chip', index: 7, left: 14, right: 32, top: 40.625, bottom: 61 },
    first[2]!,
  ]);
  assert.equal(f.owner.measure([f.target]), next);
  f.owner.destroy();
});

test('unexpected native observation remains observable rather than becoming successful empty geometry', () => {
  const f = specimen(),
    cause = new Error('controlled-native-layout-observation');
  f.target.node.getClientRects = () => {
    throw cause;
  };
  assert.throws(
    () => f.owner.measure([f.target]),
    (error) => error === cause,
  );
  f.owner.destroy();
});

test('mounted context retention is finite and an evicted native layout receives a fresh observable output', () => {
  const f = specimen(),
    first = f.owner.measure([f.target])!;
  for (let context = 0; context < 64; context++) {
    f.setWidth(300.375 + context);
    f.owner.measure([f.target]);
  }
  f.restoreWidth();
  const revisit = f.owner.measure([f.target])!;
  assert.notEqual(revisit, first);
  assert.deepEqual(revisit, first);
  f.owner.destroy();
});

test('large same-hard-line measurements consume compact canonical identities and retain actual output', () => {
  const capture = captureReorder(
    reorderState(Array.from({ length: 512 }, (_value, at) => `chip${at}`).join(', ')).state,
  );
  const output = serializeValueReorder(capture.prepared)!;
  const recipe = new SourceLineOrigins(capture.prepared.scope, capture.projection).lines(
    output,
  )[0]!;
  assert.ok(recipe.pieces.length >= 512);
  const f = specimen(),
    paint = new ReorderLineLayout(),
    lengths: number[] = [];
  const targets = Array.from({ length: 512 }, (_value, index) => {
    const lease = paint.borrow(`chip:${index}`, () => ({
      node: f.target.node,
      destroy() {},
    }));
    return {
      ...f.target,
      index,
      origins: [recipe.identity],
      leases: [
        {
          ...lease,
          geometry(context: string) {
            lengths.push(context.length);
            return lease.geometry(context);
          },
        },
      ],
    };
  });
  const first = f.owner.measure(targets)!;
  assert.equal(first.length, 512);
  assert.deepEqual(
    first.map((rect) => rect.index),
    Array.from({ length: 512 }, (_value, index) => index),
  );
  assert.ok(lengths.every((length) => length < 1024));
  assert.equal(f.owner.measure(targets), first);
  f.owner.destroy();
  paint.destroy();
});

test('marker geometry selects exact native witnesses before any unrelated observation among 180 targets', () => {
  const f = specimen(),
    second = specimen();
  const fallback: MountedReorderTarget = { ...second.target, index: 8 };
  const unrelated = Array.from(
    { length: 178 },
    (_, index): MountedReorderTarget => ({
      ...f.target,
      index: 100 + index,
      node: new Proxy({} as HTMLElement, {
        get() {
          throw new Error('unselected-native-observation');
        },
      }),
    }),
  );
  const targets = [...unrelated.slice(0, 90), f.target, fallback, ...unrelated.slice(90)];
  const witnesses = {
    held: 7,
    fallback: { kind: 'chip' as const, index: 8 },
    trailing: false,
  };
  const first = f.owner.marker(targets, witnesses)!;
  assert.deepEqual(
    first.map((rect) => rect.index),
    [7, 8],
  );
  assert.equal(f.owner.marker(targets, witnesses), first);
  assert.deepEqual(first, [
    {
      kind: 'chip',
      index: 7,
      left: 10.125,
      right: 35.375,
      top: 20.25,
      bottom: 40.625,
    },
    {
      kind: 'chip',
      index: 8,
      left: 10.125,
      right: 35.375,
      top: 20.25,
      bottom: 40.625,
    },
  ]);
  f.rewrap();
  const wrapped = f.owner.marker(targets, witnesses)!;
  assert.notEqual(wrapped, first);
  assert.deepEqual(
    wrapped.map((rect) => rect.index),
    [7, 7, 8],
  );
  f.changeFont();
  assert.notEqual(f.owner.marker(targets, witnesses), wrapped);
  f.detach();
  assert.equal(f.owner.marker(targets, witnesses), null);
  f.owner.destroy();
  second.owner.destroy();
});
