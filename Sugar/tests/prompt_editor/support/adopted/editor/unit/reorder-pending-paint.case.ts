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
/** Repeated observations of one actual native issue retain prepared output; genuine currentness changes still retire it. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

test('same-preview reads before each native phase retain one canonical child through real mask/frame publication', () => {
  const f = motionPresentationFixture();
  try {
    f.prepare();
    f.prepare();
    f.prepare();
    assert.equal(f.jobs.length, 1);
    f.jobs.shift()!();
    f.prepare();
    assert.equal(f.jobs.length, 0);
    f.tasks.shift()!();
    const child = f.paint.editor.querySelector('.sp-reorder-motion-paint');
    assert.ok(child);
    const literal = child.textContent;
    for (const phase of ['layer', 'placement', 'ready'] as const) {
      f.prepare();
      assert.equal(child.isConnected, true);
      assert.equal(f.jobs.length, 1);
      f.jobs.shift()!();
      assert.equal(f.lastRead, phase);
      f.tasks.shift()!();
      assert.equal(f.paint.editor.querySelector('.sp-reorder-motion-paint'), child);
    }
    assert.ok(f.state.field(reorderPreview).mask);
    assert.equal(child.textContent, literal);
    assert.equal(f.jobs.length, 0);
    f.paint.step(90);
    assert.equal(child.dataset.reorderMotionProgress, '0.5');
    assert.equal(f.paint.editor.querySelector('.sp-reorder-motion-paint'), child);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});
for (const change of ['appearance', 'viewport', 'scroll', 'live-paint'] as const)
  test(`same-preview ${change} change invalidates the captured READ instead of coalescing stale output`, () => {
    const f = motionPresentationFixture();
    try {
      f.prepare();
      f.jobs.shift()!();
      const write = f.tasks.shift()!;
      if (change === 'appearance') f.paint.editor.style.color = 'rgb(1,2,3)';
      else if (change === 'viewport')
        Object.assign(f.paint.document.defaultView, { innerWidth: 1024 });
      else if (change === 'scroll') Object.assign(f.paint.scroller, { scrollTop: 7 });
      else f.widget.destroy(f.root);
      // The native session routes observed scroll/viewport updates through measurement.refresh -> presentation.cancel.
      if (change === 'viewport' || change === 'scroll') f.presentation.cancel();
      f.prepare();
      const allocations = f.paint.allocations;
      write();
      assert.equal(f.paint.allocations, allocations);
      assert.equal(f.state.field(reorderPreview).mask, null);
      assert.equal(f.paint.editor.querySelectorAll('.sp-reorder-motion-paint').length, 0);
      assert.equal(f.jobs.length, 1);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
    } finally {
      f.destroy();
    }
  });
for (const change of ['publication', 'selection', 'source', 'cancel'] as const)
  test(`pending ${change} retirement never preserves obsolete paint publication`, () => {
    const f = motionPresentationFixture();
    try {
      f.prepare();
      f.jobs.shift()!();
      f.tasks.shift()!();
      const child = f.paint.editor.querySelector('.sp-reorder-motion-paint');
      assert.ok(child);
      if (change === 'publication') {
        f.publish();
        f.prepare();
      } else if (change === 'selection') {
        f.dispatch({ selection: EditorSelection.range(4, 1) });
        f.prepare();
      } else if (change === 'source') {
        f.dispatch({ changes: { from: 0, to: 1, insert: 'x' } });
        f.prepare();
      } else f.presentation.cancel();
      if (change === 'publication') {
        assert.equal(child.dataset.reorderMotionProgress, undefined);
        assert.equal(child.style.clip, 'rect(0px, 0px, 0px, 0px)');
        assert.ok(child.parentElement === f.paint.editor.querySelector('.sp-reorder-motion-stage'));
        assert.equal(child.parentElement?.parentElement, f.paint.layerMount);
        assert.equal(f.view.dom.querySelectorAll('.sp-reorder-motion-stage').length, 1);
        assert.equal(child.isConnected, true);
        assert.equal(child.querySelectorAll('[data-reorder-motion-chip]').length, 0);
      } else if (change === 'cancel') assert.equal(child.isConnected, false);
      else {
        f.jobs.shift()!();
        f.tasks.shift()!();
        assert.equal(child.isConnected, false);
      }
      assert.equal(f.state.field(reorderPreview).mask, null);
      assert.equal(f.frames.size, 0);
    } finally {
      f.destroy();
    }
  });
