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
/** Actual native request failures retire authority before deferred dispatch-capable physical cleanup. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { fixture, pointer, dragPointer, key } from './reorder-session-fixture.ts';
import { drain } from './reorder-marker-fixture.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { reorderGrabCursor } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-cursor.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

for (const phase of ['read', 'write'] as const)
  test(`native edge ${phase} failure retains its raw cause and queues physical retirement`, () => {
    const f = fixture(),
      cause = new Error('controlled-native-measure-failure'),
      window = f.view.dom.ownerDocument.defaultView!,
      request = window.requestAnimationFrame,
      box = f.view.scrollDOM.getBoundingClientRect;
    try {
      f.setChipGeometry(0, 15, 119);
      f.owner.pointerdown(pointer(1, 119));
      drain(f);
      dragPointer(f.owner, pointer(1, 119));
      drain(f);
      f.frame(30);
      const pending = f.jobs.shift()!;
      assert.ok(pending);
      const state = f.state,
        source = readExactSource(state);
      assert.equal(f.captures.size, 1);
      if (phase === 'read') {
        f.view.scrollDOM.getBoundingClientRect = () => {
          throw cause;
        };
        assert.throws(
          () => pending.read(f.view),
          (actual) => actual === cause,
        );
      } else {
        const measured = pending.read(f.view);
        assert.ok(measured);
        window.requestAnimationFrame = () => {
          throw cause;
        };
        assert.throws(
          () => pending.write!(measured, f.view),
          (actual) => actual === cause,
        );
      }
      assert.equal(f.owner.blocksInput(), false);
      assert.equal(f.owner.keydown(key('Alt')), false);
      assert.equal(f.state, state);
      assert.equal(f.captures.size, 1);
      window.requestAnimationFrame = request;
      f.view.scrollDOM.getBoundingClientRect = box;
      drain(f);
      assert.equal(f.state.doc, state.doc);
      assert.equal(f.state.selection, state.selection);
      assert.equal(readExactSource(f.state), source);
      assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(f.state.field(reorderGrabCursor), null);
      assert.equal(f.captures.size + f.frames.size + f.jobs.length + f.tasks.length, 0);
      assert.equal(f.owner.keydown(key('Alt')), true);
      drain(f);
      assert.equal(f.owner.blocksInput(), true);
      f.owner.keydown(key('Escape'));
      assert.equal(f.captures.size + f.frames.size, 0);
    } finally {
      window.requestAnimationFrame = request;
      f.view.scrollDOM.getBoundingClientRect = box;
      f.destroy();
    }
  });
