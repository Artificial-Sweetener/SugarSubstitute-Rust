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
/** The real Session/Measurement owners retain native wheel viewport priority without retiring placement or source authority. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { fixture, pointer, dragPointer, key } from './reorder-session-fixture.ts';
import { drain, paintPointerFrame, pointerTransferFixture } from './reorder-marker-fixture.ts';

/** Admit a genuine current primary hold at the top edge through actual session and measured paint. */
function held() {
  const f = fixture(),
    before = f.state;
  f.setChipGeometry(0, 15, 1);
  f.owner.pointerdown(pointer(1, 1));
  drain(f);
  dragPointer(f.owner, pointer(1, 1));
  drain(f);
  paintPointerFrame(f);
  assert.equal(f.frames.size, 1);
  assert.equal(f.captures.size, 1);
  return { f, before };
}
/** Source, complete native selection and history remain owned by CM after viewport-only interaction. */
function unchanged(
  f: ReturnType<typeof fixture>,
  before: ReturnType<typeof fixture>['state'],
): void {
  assert.equal(f.state.doc, before.doc);
  assert.equal(f.state.selection, before.selection);
  assert.equal(readExactSource(f.state), 'a, b, c');
  assert.equal(undoDepth(f.state), 0);
}

test('wheel invalidates a genuine pending edge read before its write and leaves native viewport untouched', () => {
  const { f, before } = held();
  try {
    f.frame(30);
    const requests = f.jobs.splice(0),
      values = requests.map((request) => request.read(f.view));
    assert.ok(
      values.some(
        (value) =>
          value && typeof value === 'object' && 'direction' in value && value.direction === -1,
      ),
    );
    assert.equal(f.owner.wheel(), false);
    f.view.scrollDOM.scrollTop = 120;
    requests.forEach((request, index) => request.write?.(values[index], f.view));
    f.owner.refreshGeometry();
    drain(f);
    assert.equal(f.view.scrollDOM.scrollTop, 120);
    assert.equal(f.frames.size, 0);
    assert.equal(f.captures.size, 1);
    unchanged(f, before);
  } finally {
    f.destroy();
  }
});

test('stationary wheel pause survives geometry refresh and identical-coordinate pointer events while placement stays active', () => {
  const { f, before } = held();
  try {
    assert.equal(f.owner.wheel(), false);
    f.view.scrollDOM.scrollTop = 120;
    assert.equal(f.frames.size, 0);
    f.owner.refreshGeometry();
    assert.ok(f.jobs.length > 0);
    drain(f);
    assert.equal(f.frames.size, 0);
    assert.equal(f.owner.pointermove(pointer(1, 1)), true);
    drain(f);
    f.owner.refreshGeometry();
    drain(f);
    assert.equal(f.frames.size, 0);
    assert.equal(f.view.scrollDOM.scrollTop, 120);
    assert.equal(f.owner.blocksInput(), true);
    assert.ok(f.state.field(reorderPreview).value);
    unchanged(f, before);
  } finally {
    f.destroy();
  }
});

for (const [x, y] of [
  [15.25, 1],
  [15, 1.25],
] as const)
  test(`actual pointer motion x=${x}/y=${y} resumes original edge24/30 behavior`, () => {
    const { f, before } = held();
    try {
      assert.equal(f.owner.wheel(), false);
      f.view.scrollDOM.scrollTop = 120;
      f.owner.pointermove(pointer(1, y, x));
      drain(f);
      assert.equal(f.frames.size, 2);
      f.frame(30);
      drain(f);
      assert.equal(f.view.scrollDOM.scrollTop, 96);
      unchanged(f, before);
    } finally {
      f.destroy();
    }
  });

test('dormant wheel is a no-op and cancelling a paused hold does not pause a new hold at identical coordinates', () => {
  const f = fixture(),
    before = f.state;
  try {
    assert.equal(f.owner.wheel(), false);
    assert.equal(f.frames.size, 0);
    assert.equal(f.jobs.length, 0);
    assert.equal(f.owner.blocksInput(), false);
    f.setChipGeometry(0, 15, 1);
    f.owner.pointerdown(pointer(1, 1));
    drain(f);
    dragPointer(f.owner, pointer(1, 1));
    drain(f);
    paintPointerFrame(f);
    assert.equal(f.owner.wheel(), false);
    assert.equal(f.frames.size, 0);
    f.owner.keydown(key('Escape'));
    drain(f);
    assert.equal(f.captures.size, 0);
    f.owner.pointerdown(pointer(2, 1));
    drain(f);
    dragPointer(f.owner, pointer(2, 1));
    drain(f);
    paintPointerFrame(f);
    assert.equal(f.frames.size, 1);
    assert.equal(f.captures.size, 1);
    unchanged(f, before);
  } finally {
    f.destroy();
  }
});

test('retargeting the same finite session releases wheel pause for a new hold even at identical coordinates', () => {
  const f = pointerTransferFixture(),
    before = f.state;
  try {
    f.setChipGeometry(0, 15, 1);
    f.owner.pointerdown(pointer(1, 1));
    drain(f);
    dragPointer(f.owner, pointer(1, 1));
    drain(f);
    paintPointerFrame(f);
    assert.equal(f.owner.wheel(), false);
    assert.equal(f.frames.size, 0);
    assert.equal(f.owner.pointerdown(f.down(0, 2, 1, 15)), true);
    drain(f);
    dragPointer(f.owner, pointer(2, 1));
    drain(f);
    paintPointerFrame(f);
    assert.equal(f.frames.size, 1);
    assert.equal(f.captures.size, 1);
    unchanged(f, before);
  } finally {
    f.destroy();
  }
});

test('foreign selection retires a wheel-paused hold and every owned resource without restoring the caret', () => {
  const { f } = held();
  try {
    assert.equal(f.owner.wheel(), false);
    f.view.dispatch({ selection: { anchor: 2 } });
    drain(f);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.frames.size, 0);
    assert.equal(f.captures.size, 0);
    assert.equal(f.state.selection.main.head, 2);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('wheel frame-cancellation failure remains observable and stale callback cannot mutate browser viewport', () => {
  const { f, before } = held(),
    window = f.view.dom.ownerDocument.defaultView!,
    cancel = window.cancelAnimationFrame,
    cause = new Error('wheel-frame-cancel-failed');
  try {
    window.cancelAnimationFrame = () => {
      throw cause;
    };
    assert.throws(
      () => f.owner.wheel(),
      (error) => error === cause,
    );
    window.cancelAnimationFrame = cancel;
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.captures.size, 0);
    f.view.scrollDOM.scrollTop = 120;
    f.frame(30);
    drain(f);
    assert.equal(f.view.scrollDOM.scrollTop, 120);
    assert.equal(f.frames.size, 0);
    unchanged(f, before);
  } finally {
    window.cancelAnimationFrame = cancel;
    f.destroy();
  }
});

test('disposing a wheel-paused hold releases capture and cannot reopen edge work through wheel', () => {
  const { f, before } = held();
  try {
    assert.equal(f.owner.wheel(), false);
    f.owner.destroy();
    assert.equal(f.owner.wheel(), false);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.captures.size, 1);
    f.task();
    drain(f);
    assert.equal(f.frames.size, 0);
    assert.equal(f.captures.size, 0);
    assert.equal(f.owner.blocksInput(), false);
    unchanged(f, before);
  } finally {
    f.destroy();
  }
});
