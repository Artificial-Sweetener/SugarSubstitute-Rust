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
import { drain } from './reorder-marker-fixture.ts';
/** Scheduling failures must retire the genuine mounted session and all finite native resources with exact causes. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { dragPointer, fixture, key, pointer } from './reorder-session-fixture.ts';

for (const frontier of ['entry', 'arrow', 'motion', 'release', 'scroll'] as const)
  test(`native ${frontier} scheduling failure releases the same session and preserves its original cause`, () => {
    const f = fixture(),
      cause = new Error('controlled-native-scheduling-failure'),
      schedule = f.view.requestMeasure;
    const pointerActive = ['motion', 'release', 'scroll'].includes(frontier);
    try {
      if (frontier !== 'entry') {
        if (pointerActive) f.owner.pointerdown(pointer());
        else f.owner.keydown(key('Alt'));
        f.measure();
        f.task();
        if (pointerActive) {
          drain(f);
          dragPointer(f.owner);
          f.measure();
          f.task();
          f.owner.pointermove(pointer(1, 20, 16));
        }
      }
      f.view.requestMeasure = () => {
        throw cause;
      };
      assert.throws(
        () => {
          if (frontier === 'entry') f.owner.keydown(key('Alt'));
          else if (frontier === 'arrow') f.owner.keydown(key('ArrowRight'));
          else if (frontier === 'motion') f.owner.pointermove(pointer(1, 119));
          else if (frontier === 'release') f.owner.pointerup(pointer());
          else f.frame(30);
        },
        (error) => error === cause,
      );
      while (f.tasks.length) f.task();
      assert.equal(f.owner.blocksInput(), false);
      assert.equal(f.captures.size, 0);
      assert.equal(f.frames.size, 0);
      assert.equal(f.removed, pointerActive ? 1 : 0);
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(readExactSource(f.state), 'a, b, c');
      f.view.requestMeasure = schedule;
      assert.ok(f.jobs.length <= 1);
      while (f.jobs.length || f.tasks.length) {
        if (f.jobs.length) f.measure();
        if (f.tasks.length) f.task();
      }
      f.owner.keydown(key('Alt'));
      f.measure();
      f.task();
      assert.notEqual(f.state.field(reorderPreview).value, null);
    } finally {
      f.view.requestMeasure = schedule;
      f.destroy();
    }
  });

test('an observation failure retains ordered cleanup causes while releasing later resources', () => {
  const f = fixture(),
    cause = new Error('controlled-native-scheduling-failure'),
    cleanup = new Error('controlled-frame-release-failure');
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    dragPointer(f.owner, pointer());
    f.measure();
    f.task();
    f.owner.pointermove(pointer(1, 20, 16));
    const window = f.view.dom.ownerDocument.defaultView!;
    const cancel = window.cancelAnimationFrame.bind(window);
    window.cancelAnimationFrame = (id) => {
      cancel(id);
      throw cleanup;
    };
    f.view.requestMeasure = () => {
      throw cause;
    };
    assert.throws(
      () => f.owner.pointermove(pointer()),
      (error) =>
        error instanceof AggregateError &&
        error.errors[0] === cause &&
        error.errors[1] instanceof AggregateError &&
        error.errors[1].errors.length === 2 &&
        error.errors[1].errors[0] === cleanup &&
        error.errors[1].errors[1] === cleanup,
    );
    while (f.tasks.length) f.task();
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.captures.size, 0);
    assert.equal(f.frames.size, 0);
    assert.equal(f.removed, 1);
    assert.equal(f.state.field(reorderPreview).value, null);
  } finally {
    f.destroy();
  }
});
