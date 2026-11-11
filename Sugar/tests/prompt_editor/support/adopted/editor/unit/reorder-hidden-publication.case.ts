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
/** One current hidden publication supplies both semantic origin and destinations through actual native transactions. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection } from '@codemirror/state';
import { undoDepth, undo, redo } from '@codemirror/commands';
import { fixture, key } from './reorder-session-fixture.ts';
import { drain, geometryUpdate } from './reorder-marker-fixture.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { collisionGeometry } from './reorder-passive-geometry-fixture.ts';
/** Independent observed fragments follow actual full/hidden publication identity, with isolated geometry-only changes. */
function publication(wrapped = false, reviewed = false) {
  const f = fixture(),
    query = f.view.contentDOM.querySelectorAll;
  const box = (left: number, top: number) => ({
    left,
    right: left + 20,
    top,
    bottom: top + 20,
    width: 20,
    height: 20,
  });
  let shifted = false,
    origin = false;
  collisionGeometry(f, (id) => {
    const layout = shifted
      ? [box(10, 10), reviewed ? box(10, 0) : box(180, 10), box(10, 40)]
      : [box(10, 10), box(10, 40), box(10, 70)];
    return (
      wrapped && id === 1
        ? [box(10, 10), box(10, 70)]
        : wrapped && id === 2
          ? [box(10, 100)]
          : [layout[id]!]
    ) as DOMRect[];
  });
  f.view.contentDOM.querySelectorAll = ((selector: string) =>
    Array.from(query.call(f.view.contentDOM, selector), (node) => ({
      dataset: (node as HTMLElement).dataset,
      /** Preserve the original current node attribute protocol while overriding geometry alone. */
      getAttribute(name: string): string | null {
        return (node as HTMLElement).getAttribute(name);
      },
      getClientRects() {
        const full = f.state.field(reorderPreview).value!.order.order.includes(0),
          id = Number((node as HTMLElement).dataset.reorderChip);
        const layout = full
          ? [box(10, origin ? 90 : 10), box(10, 40), box(10, 70)]
          : shifted
            ? [box(10, 10), reviewed ? box(10, 0) : box(180, 10), box(10, 40)]
            : [box(10, 10), box(10, 40), box(10, 70)];
        return wrapped && !full && id === 1
          ? [box(10, 10), box(10, 70)]
          : wrapped && !full && id === 2
            ? [box(10, 100)]
            : [layout[id]!];
      },
    }))) as unknown as typeof f.view.contentDOM.querySelectorAll;
  return {
    f,
    shift() {
      shifted = true;
    },
    shiftAbsentHeld() {
      origin = true;
    },
  };
}
for (const deferred of [false, true])
  for (const absentHeld of [false, true])
    test(`current hidden slot survives ${absentHeld ? 'absent held CSS' : 'hidden target'} supersession before ${deferred ? 'deferred' : 'write'}`, () => {
      const { f, shift, shiftAbsentHeld } = publication();
      try {
        f.view.dispatch({ selection: EditorSelection.range(1, 0) });
        f.owner.keydown(key('Alt'));
        drain(f);
        f.owner.keydown(key('ArrowDown'));
        f.measure();
        f.task();
        const publication = f.state.field(reorderPreview).value,
          pending = f.jobs.shift()!,
          observed = pending.read(f.view);
        assert.ok(observed);
        assert.ok(publication);
        assert.equal(publication.order.order.includes(0), true);
        if (deferred) pending.write!(observed, f.view);
        if (absentHeld) shiftAbsentHeld();
        else shift();
        f.owner.update(geometryUpdate(f, true, false));
        f.owner.keyup(key('Alt'));
        if (!deferred) pending.write!(observed, f.view);
        assert.equal(f.state.field(reorderPreview).value, publication);
        assert.equal(readExactSource(f.state), 'a, b, c');
        assert.equal(undoDepth(f.state), 0);
        drain(f);
        assert.equal(readExactSource(f.state), absentHeld ? 'b, a, c' : 'b, c, a');
        assert.deepEqual(
          [f.state.selection.main.anchor, f.state.selection.main.head],
          absentHeld ? [4, 3] : [7, 6],
        );
        assert.equal(undoDepth(f.state), 1);
        assert.equal(f.state.field(reorderPreview).value, null);
        assert.equal(f.owner.blocksInput(), false);
        assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
        assert.equal(undo(f.view), true);
        assert.equal(readExactSource(f.state), 'a, b, c');
        assert.deepEqual([f.state.selection.main.anchor, f.state.selection.main.head], [1, 0]);
        assert.equal(redo(f.view), true);
        assert.equal(readExactSource(f.state), absentHeld ? 'b, a, c' : 'b, c, a');
      } finally {
        f.destroy();
      }
    });
for (const deferred of [false, true])
  test(`initial held seed supersession before ${deferred ? 'deferred' : 'write'} renews the full input read`, () => {
    const { f, shiftAbsentHeld } = publication(true);
    try {
      f.owner.keydown(key('Alt'));
      drain(f);
      f.owner.keydown(key('ArrowDown'));
      const pending = f.jobs.shift()!,
        seed = pending.read(f.view);
      assert.ok(seed);
      if (deferred) pending.write!(seed, f.view);
      shiftAbsentHeld();
      f.owner.update(geometryUpdate(f, true, false));
      f.owner.keyup(key('Alt'));
      if (!deferred) pending.write!(seed, f.view);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
      drain(f);
      assert.equal(readExactSource(f.state), 'b, a, c');
      assert.equal(undoDepth(f.state), 1);
      assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
    } finally {
      f.destroy();
    }
  });

test('unchanged initial wrapped seed stays in the current semantic slot without a source or history event', () => {
  const { f } = publication(true);
  try {
    f.owner.keydown(key('Alt'));
    drain(f);
    f.owner.keydown(key('ArrowDown'));
    f.owner.keyup(key('Alt'));
    drain(f);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
  } finally {
    f.destroy();
  }
});

for (const deferred of [false, true])
  test(`original reviewed hidden-only fragments renew before ${deferred ? 'deferred' : 'write'} under current semantic origin`, () => {
    const { f, shift } = publication(false, true);
    try {
      f.owner.keydown(key('Alt'));
      drain(f);
      f.owner.keydown(key('ArrowDown'));
      f.measure();
      f.task();
      const hidden = f.state.field(reorderPreview).value!,
        pending = f.jobs.shift()!,
        stale = pending.read(f.view);
      assert.ok(stale);
      if (deferred) pending.write!(stale, f.view);
      shift();
      f.owner.update(geometryUpdate(f, true, false));
      f.owner.keyup(key('Alt'));
      if (!deferred) pending.write!(stale, f.view);
      assert.equal(f.state.field(reorderPreview).value, hidden);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
      drain(f);
      assert.equal(readExactSource(f.state), 'b, a, c');
      assert.equal(undoDepth(f.state), 1);
      assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
      assert.equal(undo(f.view), true);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(redo(f.view), true);
      assert.equal(readExactSource(f.state), 'b, a, c');
    } finally {
      f.destroy();
    }
  });
