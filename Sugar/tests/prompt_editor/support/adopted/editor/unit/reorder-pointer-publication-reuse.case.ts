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
/** Repeated physical target observations retain canonical publication while genuine destinations keep exact source/history authority. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import {
  installReorderPreview,
  reorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { drain, paintPointerFrame } from './reorder-marker-fixture.ts';
import { fixture, pointer, dragPointer } from './reorder-session-fixture.ts';

for (const x of [15, 55]) {
  test(`equivalent pointer destination x=${x} retains complete publication and mounted paint while latest feedback and release remain authoritative`, () => {
    const f = fixture();
    try {
      f.owner.pointerdown(pointer());
      drain(f);
      dragPointer(f.owner, pointer(1, 20, x));
      drain(f);
      const before = f.state.field(reorderPreview),
        source = readExactSource(f.state),
        selection = f.state.selection,
        painted = f.paintedRoot,
        nativeTargets = before.native
          ? Array.from(f.view.contentDOM.querySelectorAll('[data-reorder-chip]'))
          : null;
      assert.ok(before.value);
      if (before.native) {
        assert.equal(painted === null, true);
        assert.equal(nativeTargets?.length, 3);
      } else assert.ok(painted);
      assert.deepEqual(before.value.order.order, x === 15 ? [0, 1, 2] : [1, 2, 0]);
      let publications = 0;
      f.observe((transaction) => {
        if (transaction.effects.some((effect) => effect.is(installReorderPreview))) publications++;
      });
      f.owner.pointermove(pointer(1, 21, x));
      drain(f);
      paintPointerFrame(f);
      const after = f.state.field(reorderPreview);
      assert.equal(after.value, before.value);
      assert.equal(after.value?.order, before.value.order);
      assert.equal(after.value?.output, before.value.output);
      assert.equal(after.decorations, before.decorations);
      assert.equal(f.paintedRoot === painted, true);
      assert.equal(after.native, before.native);
      if (nativeTargets) {
        const retained = Array.from(f.view.contentDOM.querySelectorAll('[data-reorder-chip]'));
        assert.equal(retained.length, nativeTargets.length);
        assert.equal(
          retained.every((node, at) => node === nativeTargets[at]),
          true,
        );
      }
      assert.equal(publications, 0);
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
      const surface = Array.from(f.view.dom.ownerDocument.body.childNodes).find((node) =>
        (node as HTMLElement).className.includes('sp-reorder-drag-surface'),
      ) as HTMLElement | undefined;
      assert.ok(surface);
      assert.equal(surface.style.transform, `translate(${x - 5}px, 11px)`);
      f.owner.pointerup(pointer(1, 21, x));
      drain(f);
      assert.equal(readExactSource(f.state), x === 15 ? 'a, b, c' : 'b, c, a');
      assert.equal(undoDepth(f.state), x === 15 ? 0 : 1);
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
    } finally {
      f.destroy();
    }
  });
}
