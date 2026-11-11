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
/** Actual measured scroll-owner direction/write frontiers preserve stationary short-view interiors and finite resource lifetime. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { hideReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { ReorderMeasurement } from '../../../../../../frontend/sugarprompt/editor/src/reorder-measurement.ts';
import type { ReorderMeasureInput } from '../../../../../../frontend/sugarprompt/editor/src/reorder-measurement-contracts.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { captureReorder } from '../transaction/reorder-fixture.ts';
import { fixture } from './reorder-session-fixture.ts';

/** The native viewport operand is measured in viewport CSS pixels. */
type Viewport = Pick<DOMRect, 'top' | 'bottom' | 'height'>;
/** Bind genuine Core capture to the real measurement owner; native browser scroll/paint causality remains browser-owned. */
function scrolling(viewport: Viewport, y: number, clientHeight = viewport.height) {
  const f = fixture(),
    before = f.state,
    capture = captureReorder(before);
  let current = true,
    coordinates = { x: 15, y };
  const input: ReorderMeasureInput = {
    token: {},
    capture,
    order: capture.prepared.initial,
    held: 0,
    pointer: 1,
    hidden: hideReorderChip(capture.prepared, capture.prepared.initial, 0)!,
    get coordinates() {
      return coordinates;
    },
  };
  const measurement = new ReorderMeasurement(f.view, {
    current: () => current,
    collision: () => null,
    prepare() {
      throw new Error('scroll-policy-published-Core-target');
    },
    restore() {
      throw new Error('scroll-policy-restored-Core-order');
    },
    commit() {
      throw new Error('scroll-policy-committed-source');
    },
    failed() {
      throw new Error('scroll-policy-native-boundary-failed');
    },
  });
  f.view.scrollDOM.getBoundingClientRect = () => ({
    ...viewport,
    left: 0,
    right: 200,
    width: 200,
    x: 0,
    y: viewport.top,
    toJSON: () => ({}),
  });
  Object.defineProperty(f.view.scrollDOM, 'clientHeight', {
    value: clientHeight,
    configurable: true,
  });
  f.view.scrollDOM.scrollTop = 100;
  return {
    f,
    measurement,
    input,
    /** Advance the actual finite RAF and read its queued native measurement without applying the write yet. */
    read(time = 30) {
      measurement.scroll(input);
      f.frame(time);
      const request = f.jobs.shift();
      assert.ok(request);
      const result = request.read(f.view);
      assert.ok(result && typeof result === 'object' && 'direction' in result);
      return {
        direction: result.direction,
        write: () => request.write?.(result, f.view),
      };
    },
    /** Replace genuine current coordinate identity between read/write. */
    replaceCoordinates() {
      coordinates = { ...coordinates };
    },
    /** Foreign session currentness invalidates the same queued writer. */
    invalidate() {
      current = false;
    },
    /** The owner retains no source/selection/history authority and releases its frame at teardown. */
    destroy() {
      measurement.destroy();
      assert.equal(f.state.doc, before.doc);
      assert.equal(f.state.selection, before.selection);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
      assert.equal(f.frames.size, 0);
      assert.equal(f.captures.size, 0);
      f.destroy();
    },
  };
}
const short = { top: -1.0625, bottom: 67.9375, height: 69 };
for (const [y, direction] of [
  [25.0625, 0],
  [42.0625, 0],
  [15, -1],
  [16.1875, 0],
  [50.6875, 0],
  [51, 1],
] as const)
  test(`69px measured viewport pointerY=${y} has literal direction ${direction}`, () => {
    const owner = scrolling(short, y);
    try {
      const read = owner.read();
      assert.equal(read.direction, direction);
      read.write();
      assert.equal(owner.f.view.scrollDOM.scrollTop, 100 + direction * 24);
      if (direction === 0) assert.equal(owner.f.frames.size, 0);
    } finally {
      owner.destroy();
    }
  });
for (const [y, direction] of [
  [35.999, -1],
  [36, 0],
  [108, 0],
  [108.001, 1],
] as const)
  test(`144px measured viewport preserves original36px boundary y=${y}`, () => {
    const owner = scrolling({ top: 0, bottom: 144, height: 144 }, y);
    try {
      const read = owner.read();
      assert.equal(read.direction, direction);
      read.write();
      assert.equal(owner.f.view.scrollDOM.scrollTop, 100 + direction * 24);
    } finally {
      owner.destroy();
    }
  });
for (const viewport of [
  { top: 0, bottom: 0, height: 0 },
  { top: 0, bottom: -1, height: -1 },
  { top: NaN, bottom: 69, height: 69 },
  { top: 0, bottom: Infinity, height: 69 },
  { top: 0, bottom: 69, height: NaN },
  { top: 69, bottom: 0, height: 69 },
])
  test(`invalid viewport refuses scrolling operand ${
    Object.keys(viewport)
      .filter((key) => !Number.isFinite(viewport[key as keyof Viewport]))
      .join(',') || viewport.height
  }`, () => {
    const owner = scrolling(viewport, 0);
    try {
      const read = owner.read();
      assert.equal(read.direction, 0);
      read.write();
      assert.equal(owner.f.view.scrollDOM.scrollTop, 100);
      assert.equal(owner.f.frames.size, 0);
    } finally {
      owner.destroy();
    }
  });
for (const y of [NaN, Infinity])
  test(`nonfinite pointerY=${y} refuses edge scrolling`, () => {
    const owner = scrolling(short, y);
    try {
      const read = owner.read();
      assert.equal(read.direction, 0);
      read.write();
      assert.equal(owner.f.view.scrollDOM.scrollTop, 100);
    } finally {
      owner.destroy();
    }
  });
for (const boundary of ['coordinate', 'currentness', 'epoch', 'disposal'] as const)
  test(`queued edge write rejects replaced ${boundary} without source or history`, () => {
    const owner = scrolling(short, 51);
    try {
      const read = owner.read();
      assert.equal(read.direction, 1);
      if (boundary === 'coordinate') owner.replaceCoordinates();
      else if (boundary === 'currentness') owner.invalidate();
      else if (boundary === 'epoch') owner.measurement.cancel();
      else owner.measurement.destroy();
      read.write();
      assert.equal(owner.f.view.scrollDOM.scrollTop, 100);
    } finally {
      owner.destroy();
    }
  });
test('scaled edge scrolling retains24 physical pixels and original30ms gate', () => {
  const owner = scrolling(short, 51, 34.5);
  try {
    const early = owner.read(29);
    assert.equal(early.direction, 1);
    early.write();
    assert.equal(owner.f.view.scrollDOM.scrollTop, 100);
    const accepted = owner.read(30);
    assert.equal(accepted.direction, 1);
    accepted.write();
    assert.equal(owner.f.view.scrollDOM.scrollTop, 112);
  } finally {
    owner.destroy();
  }
});
