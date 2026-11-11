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
/** Actual session and prepared proxy paint prove pointer following independently of delayed target geometry. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { fixture, pointer, dragPointer } from './reorder-session-fixture.ts';
import { drain } from './reorder-marker-fixture.ts';

for (const latest of [55, 63]) {
  test(`latest pointer x=${latest} paints the prepared proxy before any queued target read`, () => {
    const f = fixture();
    try {
      f.owner.pointerdown(pointer());
      drain(f);
      dragPointer(f.owner, pointer());
      drain(f);
      for (const [handle, callback] of [...f.frames]) {
        f.frames.delete(handle);
        callback(16);
      }
      const source = readExactSource(f.state),
        selection = f.state.selection,
        publication = f.state.field(reorderPreview).value;
      const surface = Array.from(f.view.dom.ownerDocument.body.childNodes).find((node) =>
        (node as HTMLElement).className.includes('sp-reorder-drag-surface'),
      ) as HTMLElement | undefined;
      assert.ok(surface);
      assert.equal(surface.style.transform, 'translate(10px, 10px)');
      f.owner.pointermove(pointer(1, 21, 55));
      if (latest !== 55) f.owner.pointermove(pointer(1, 23, latest));
      assert.ok(f.jobs.length);
      // Deliver actual owned animation callbacks while every target request remains deliberately unread.
      for (const [handle, callback] of [...f.frames]) {
        f.frames.delete(handle);
        callback(16);
      }
      assert.equal(
        surface.style.transform,
        `translate(${latest - 5}px, ${latest === 55 ? 11 : 13}px)`,
      );
      assert.equal(surface.style.top, '0px');
      assert.equal(f.state.field(reorderPreview).value, publication);
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
      f.owner.keydown({ key: 'Escape' } as KeyboardEvent);
      drain(f);
      assert.equal(f.frames.size + f.jobs.length + f.tasks.length + f.captures.size, 0);
    } finally {
      f.destroy();
    }
  });
}
