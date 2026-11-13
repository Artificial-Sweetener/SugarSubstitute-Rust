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
/** Real wheel/command owners consume authoritative reorder state without competing geometry, feedback or source work. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { wheelFeedback } from '../../../../../../frontend/sugarprompt/editor/src/wheel-feedback.ts';
import { currentNativeReorder } from '../../../../../../frontend/sugarprompt/editor/src/reorder-command.ts';
import { richProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { wheelReorderFixture } from './wheel-reorder-fixture.ts';
import type { EditorState } from '@codemirror/state';
import { EmphasisWidget } from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';

/** The real policy fixture's two literal canonical boundary widgets replace the retired body mark operand. */
function assertReadyPaint(state: EditorState): void {
  const roles: (string | number)[][] = [];
  const cursor = state.field(richProjection).all.iter();
  for (; cursor.value; cursor.next()) {
    const widget = cursor.value.spec.widget;
    if (widget instanceof EmphasisWidget)
      roles.push([cursor.from, cursor.to, widget.part, widget.feedback]);
  }
  assert.deepEqual(roles, [
    [0, 1, 'prefix', 1],
    [5, 11, 'suffix', 1],
  ]);
}

/** Literal external mouse coordinates are independent of the source and owner policies. */
const point = { clientX: 20, clientY: 20 } as MouseEvent;
/** A genuine wheel callback's default-prevention outcome remains independently observable. */
function wheel() {
  let prevented = false;
  return {
    get prevented() {
      return prevented;
    },
    event: {
      clientX: 20,
      clientY: 20,
      deltaX: 0,
      deltaY: -1,
      ctrlKey: false,
      preventDefault() {
        prevented = true;
      },
    } as WheelEvent,
  };
}

for (const kind of ['preview', 'grab'] as const)
  test(`${kind} ownership suppresses sustained wheel geometry and keeps source/history/default wheel unchanged`, async () => {
    const f = await wheelReorderFixture();
    try {
      const source = readExactSource(f.state),
        selection = f.state.selection;
      f.publish(kind);
      f.forbidGeometry();
      for (let index = 0; index < 144; index++)
        // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
        f.owner.move({ ...point, clientX: 20 + index } as MouseEvent);
      f.owner.geometryChanged();
      while (f.jobs.length) f.measure();
      f.drain();
      f.dwell();
      f.drain();
      const event = wheel();
      assert.equal(f.owner.wheel(event.event), false);
      assert.equal(event.prevented, false);
      assert.equal(f.jobs.length + f.timers.size, 0);
      assert.equal(f.positionReads + f.rangeReads, 0);
      assert.equal(f.state.field(wheelFeedback).target, null);
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
    } finally {
      f.destroy();
    }
  });

for (const boundary of ['write', 'dwell'] as const)
  test(`queued ${boundary} observes accepted reorder before deferred publication even before its update notification`, async () => {
    const f = await wheelReorderFixture();
    try {
      f.owner.move(point);
      if (boundary === 'write') {
        const job = f.jobs.shift()!;
        const measured = job.read(f.view);
        assert.equal(f.positionReads, 1);
        f.publish('preview', false);
        job.write?.(measured, f.view);
      } else {
        f.measure();
        f.drain();
        assert.equal(f.timers.size, 1);
        f.publish('preview', false);
        f.dwell();
      }
      f.forbidGeometry();
      f.drain();
      assert.equal(f.state.field(wheelFeedback).target, null);
      assert.equal(f.timers.size, 0);
      const capture = f.state.field(reorderPreview).value!.capture;
      assert.equal(currentNativeReorder(f.state, capture), true);
      assert.equal(readExactSource(f.state), '(soft:2.00), next');
      assert.equal(undoDepth(f.state), 0);
    } finally {
      f.destroy();
    }
  });

test('existing actual readiness retires on takeover and a new ordinary move prepares visible feedback again', async () => {
  const f = await wheelReorderFixture();
  try {
    f.owner.move(point);
    f.measure();
    f.drain();
    f.dwell();
    const ready = f.state.field(wheelFeedback);
    assert.deepEqual(ready.target?.range, { from: 1, to: 5 });
    assert.equal(ready.target?.kind, 'emphasis');
    assertReadyPaint(f.state);
    f.publish('preview');
    f.forbidGeometry();
    f.drain();
    assert.equal(f.state.field(wheelFeedback).target, null);
    f.dwell();
    f.drain();
    assert.equal(f.state.field(wheelFeedback).target, null);
    f.publish('ordinary');
    f.drain();
    f.allowGeometry();
    f.owner.move(point);
    f.measure();
    f.drain();
    f.dwell();
    const returned = f.state.field(wheelFeedback);
    assert.deepEqual(returned.target?.range, { from: 1, to: 5 });
    assert.equal(returned.target?.kind, 'emphasis');
    assertReadyPaint(f.state);
    assert.notEqual(returned, ready);
    assert.equal(readExactSource(f.state), '(soft:2.00), next');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('modifier keys without authoritative reorder state retain ordinary wheel readiness', async () => {
  const f = await wheelReorderFixture();
  try {
    f.owner.move({
      // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
      ...point,
      altKey: true,
      ctrlKey: true,
      getModifierState: () => true,
    } as MouseEvent);
    f.measure();
    f.drain();
    f.dwell();
    assert.deepEqual(f.state.field(wheelFeedback).target?.range, {
      from: 1,
      to: 5,
    });
    assert.equal(f.state.field(wheelFeedback).target?.kind, 'emphasis');
    assertReadyPaint(f.state);
  } finally {
    f.destroy();
  }
});

test('an already queued actual wheel source step cannot publish after accepted reorder takes over', async () => {
  const f = await wheelReorderFixture();
  try {
    f.owner.move(point);
    f.measure();
    f.drain();
    f.dwell();
    const event = wheel();
    assert.equal(f.owner.wheel(event.event), true);
    assert.equal(event.prevented, true);
    f.dwell();
    const job = f.jobs.shift()!;
    const measured = job.read(f.view);
    job.write?.(measured, f.view);
    const queuedSource = f.tasks[0];
    assert.ok(queuedSource);
    f.publish('preview');
    assert.equal(f.tasks[0], queuedSource);
    assert.equal(f.state.field(wheelFeedback).target, null);
    assert.equal(currentNativeReorder(f.state, f.state.field(reorderPreview).value!.capture), true);
    f.drain();
    while (f.jobs.length) {
      f.measure();
      f.drain();
    }
    f.dwell();
    f.drain();
    assert.equal(readExactSource(f.state), '(soft:2.00), next');
    assert.equal(undoDepth(f.state), 0);
    const capture = f.state.field(reorderPreview).value!.capture;
    assert.equal(currentNativeReorder(f.state, capture), true);
  } finally {
    f.destroy();
  }
});

test('ordinary queued wheel repeats keep their token authority while actual source sessions advance', async () => {
  const f = await wheelReorderFixture();
  try {
    f.owner.move(point);
    f.measure();
    f.drain();
    f.dwell();
    assert.equal(f.owner.wheel(wheel().event), true);
    assert.equal(f.owner.wheel(wheel().event), true);
    f.dwell();
    while (f.jobs.length) {
      f.measure();
      f.drain();
    }
    assert.equal(readExactSource(f.state), '(soft:2.10), next');
    assert.equal(undoDepth(f.state), 2);
  } finally {
    f.destroy();
  }
});

for (const kind of ['lora', 'wildcard'] as const)
  test(`existing ready ${kind} projection retires before genuine reorder entry captures its current paint`, async () => {
    const f = await wheelReorderFixture(kind);
    try {
      const source = readExactSource(f.state),
        selection = f.state.selection,
        ordinary = f.state.field(richProjection).all;
      f.owner.move(point);
      f.measure();
      f.drain();
      f.dwell();
      assert.equal(f.state.field(wheelFeedback).target?.kind, kind);
      assert.notEqual(f.state.field(richProjection).all, ordinary);
      f.forbidGeometry();
      const capture = f.takeover();
      assert.equal(f.state.field(wheelFeedback).target, null);
      assert.equal(currentNativeReorder(f.state, capture), true);
      f.dwell();
      f.drain();
      assert.equal(f.state.field(wheelFeedback).target, null);
      assert.equal(currentNativeReorder(f.state, capture), true);
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
    } finally {
      f.destroy();
    }
  });

test('accepted wheel action survives a later hover without transferring its queued result', async () => {
  const f = await wheelReorderFixture('two-emphasis');
  try {
    f.owner.move(point);
    f.measure();
    f.drain();
    f.dwell();
    assert.deepEqual(f.state.field(wheelFeedback).target?.range, {
      from: 1,
      to: 4,
    });
    assert.equal(f.owner.wheel(wheel().event), true);
    // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
    const dog = { ...point, clientX: 150 } as MouseEvent;
    f.owner.move(dog);
    f.measure();
    f.drain();
    f.advance(64);
    while (f.jobs.length) {
      f.measure();
      f.drain();
    }
    assert.equal(readExactSource(f.state), '(cat:1.25), (dog:1.30)');
    const second = wheel();
    // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
    assert.equal(f.owner.wheel({ ...second.event, clientX: 150 } as WheelEvent), false);
    assert.equal(second.prevented, false);
    f.advance(64);
    while (f.jobs.length) {
      f.measure();
      f.drain();
    }
    assert.equal(readExactSource(f.state), '(cat:1.25), (dog:1.30)');
    assert.equal(undoDepth(f.state), 1);
    assert.equal(f.state.field(wheelFeedback).target, null);
  } finally {
    f.destroy();
  }
});
