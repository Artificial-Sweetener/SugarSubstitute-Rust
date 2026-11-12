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
/** Actual presentation distinguishes pending pointer delivery from optional canonical preparation and typed native fallback. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { nativeReorderViewportFootprint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-paint.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { wholeFixture, publishWhole } from './reorder-pointer-stage-fixture.ts';

for (const outcome of ['ready', 'unavailable'] as const)
  for (const pointerPending of [false, true])
    test(`actual ${outcome} preparation retires only transient landing pending; pointer delivery pending=${pointerPending}`, () => {
      const f = motionPresentationFixture();
      try {
        Object.assign(f.input, { pointer: 1 });
        if (outcome === 'unavailable')
          // The actual held group fits while both peer groups lie outside this literal viewport.
          Object.assign(f.paint.scroller.box, { left: 50, width: 40 });
        const box = f.view.scrollDOM.getBoundingClientRect();
        const viewport = {
          left: box.left,
          right: box.right,
          top: box.top,
          bottom: box.bottom,
        };
        const rects = nativeReorderViewportFootprint(f.view, f.input.held);
        assert.ok(rects?.length);
        const selection = f.state.selection;
        f.presentation.mark(f.input, rects, 'rgb(1,2,3)', viewport, false);
        const landing = f.paint.document.body!.querySelector('.sp-reorder-landing');
        assert.ok(landing);
        assert.equal(landing.dataset.reorderLanding, 'settled');
        assert.equal(landing.style.opacity, '0.82');
        if (pointerPending) f.presentation.pending();
        f.prepare();
        f.jobs.shift()!();
        f.tasks.shift()!();
        assert.equal(landing.dataset.reorderLanding, 'pending');
        assert.equal(landing.style.opacity, '0.52');
        f.deliver();
        assert.equal(f.jobs.length, 0);
        assert.equal(f.state.field(reorderPreview).mask !== null, outcome === 'ready');
        assert.equal(landing.isConnected, true);
        assert.equal(landing.dataset.reorderLanding, pointerPending ? 'pending' : 'settled');
        assert.equal(landing.style.opacity, pointerPending ? '0.52' : '0.82');
        const contour = landing.querySelector('[data-reorder-landing-contour]');
        assert.ok(contour);
        f.presentation.mark(f.input, rects, 'rgb(1,2,3)', viewport, false);
        assert.equal(f.paint.document.body!.querySelector('.sp-reorder-landing'), landing);
        assert.equal(landing.querySelector('[data-reorder-landing-contour]'), contour);
        assert.equal(landing.dataset.reorderLanding, 'settled');
        assert.equal(landing.style.opacity, '0.82');
        assert.equal(readExactSource(f.state), 'a, b, c');
        assert.equal(f.state.selection, selection);
        assert.equal(undoDepth(f.state), 0);
        assert.equal(redoDepth(f.state), 0);
        f.presentation.cancel();
        assert.equal(landing.isConnected, false);
        assert.equal(f.tasks.length, outcome === 'ready' ? 1 : 0);
        if (outcome === 'ready') f.tasks.shift()!();
        assert.equal(f.state.field(reorderPreview).mask === null, true);
      } finally {
        f.destroy();
      }
    });

for (const preparing of [false, true])
  test(`fresh whole marker settles pointer delivery while preparation pending=${preparing}`, () => {
    const f = wholeFixture();
    try {
      const selection = f.state.selection;
      f.prepare();
      f.deliver();
      const display = f.presentation.displayPaint(f.input),
        landing = f.paint.document.body!.querySelector('.sp-reorder-landing');
      assert.ok(display && landing);
      const path = landing.querySelector('[data-reorder-landing-contour]');
      assert.ok(path);
      f.presentation.pending();
      if (preparing) {
        publishWhole(f, 0);
        f.prepare();
      }
      assert.equal(landing.dataset.reorderLanding, 'pending');
      assert.equal(landing.style.opacity, '0.52');
      f.presentation.mark(f.input, display.rects, display.color, display.viewport, false);
      assert.equal(f.paint.document.body!.querySelector('.sp-reorder-landing'), landing);
      assert.equal(landing.querySelector('[data-reorder-landing-contour]'), path);
      assert.equal(landing.dataset.reorderLanding, preparing ? 'pending' : 'settled');
      assert.equal(landing.style.opacity, preparing ? '0.52' : '0.82');
      if (preparing) {
        f.deliver();
        assert.equal(landing.dataset.reorderLanding, 'settled');
        assert.equal(landing.style.opacity, '0.82');
      }
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
      assert.equal(redoDepth(f.state), 0);
    } finally {
      f.destroy();
    }
  });
