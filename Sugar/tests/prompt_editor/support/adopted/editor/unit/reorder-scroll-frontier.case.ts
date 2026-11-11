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
/** Already-read native edge-scroll writes retain the same pointer, geometry and finite gesture authority. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { fixture, pointer, dragPointer } from './reorder-session-fixture.ts';
import { drain, geometryUpdate } from './reorder-marker-fixture.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

for (const mode of ['motion', 'geometry', 'source', 'cancel', 'unchanged'] as const)
  test(`already-read edge write preserves ${mode} frontier and complete source/history/capture retirement`, () => {
    const f = fixture();
    try {
      f.setChipGeometry(0, 15, 119);
      f.owner.pointerdown(pointer(1, 119));
      drain(f);
      dragPointer(f.owner, pointer(1, 119));
      drain(f);
      f.frame(30);
      const pending = f.jobs.shift()!,
        measured = pending.read(f.view);
      assert.ok(measured && typeof measured === 'object');
      assert.equal(Reflect.get(measured, 'direction'), 1);
      assert.equal(Reflect.get(measured, 'scale'), 1);
      if (mode === 'motion') f.owner.pointermove(pointer(1, 60));
      if (mode === 'geometry') f.owner.update(geometryUpdate(f, true, false));
      if (mode === 'source') f.view.dispatch({ changes: { from: 0, insert: 'x' } });
      if (mode === 'cancel') f.owner.close();
      pending.write!(measured, f.view);
      assert.equal(f.view.scrollDOM.scrollTop, mode === 'unchanged' ? 24 : 0);
      assert.equal(readExactSource(f.state), mode === 'source' ? 'xa, b, c' : 'a, b, c');
      assert.equal(undoDepth(f.state), mode === 'source' ? 1 : 0);
      f.owner.close();
      drain(f);
      assert.equal(f.captures.size + f.frames.size + f.jobs.length + f.tasks.length, 0);
    } finally {
      f.destroy();
    }
  });
