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
/** Coordinate identity alone cannot retire geometry for the actual unchanged accepted publication. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { paintFixture, drain } from './reorder-marker-fixture.ts';
import { pointer, dragPointer } from './reorder-session-fixture.ts';
import { installReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { markerCurrentnessFixture } from './reorder-marker-currentness-fixture.ts';

for (const phase of ['read', 'write'] as const) {
  test(`coordinate-only identity replacement retains accepted marker ${phase} geometry without source/history changes`, () => {
    const c = markerCurrentnessFixture();
    try {
      const source = readExactSource(c.f.state),
        selection = c.f.state.selection,
        request = c.issue();
      if (phase === 'read') c.input.coordinates = { x: 16, y: 21 };
      const measured = request.read(c.f.view);
      assert.ok(measured && typeof measured === 'object' && 'rect' in measured);
      if (phase === 'write') c.input.coordinates = { x: 16, y: 21 };
      request.write!(measured, c.f.view);
      assert.equal(
        c.nodes.some((node) => node.className === 'sp-reorder-insertion-marker' && !node.removed),
        true,
      );
      assert.equal(c.landing.length, 1);
      assert.equal(c.landing[0]?.pending, true);
      assert.equal(
        c.nodes.find((node) => node.className === 'sp-reorder-insertion-marker' && !node.removed)
          ?.style.opacity,
        '0.52',
      );
      assert.equal(readExactSource(c.f.state), source);
      assert.equal(c.f.state.selection, selection);
      assert.equal(undoDepth(c.f.state), 0);
    } finally {
      c.destroy();
    }
  });
}

for (const retirement of ['order', 'geometry', 'source', 'destroy'] as const) {
  test(`genuine ${retirement} retirement rejects already measured accepted marker geometry`, () => {
    const c = markerCurrentnessFixture();
    try {
      const request = c.issue(),
        measured = request.read(c.f.view);
      assert.ok(measured && typeof measured === 'object' && 'rect' in measured);
      if (retirement === 'order') c.replaceOrder();
      else if (retirement === 'geometry') c.placement.refresh(c.input);
      else if (retirement === 'source') c.f.view.dispatch({ changes: { from: 0, insert: 'x' } });
      else c.placement.destroy();
      request.write!(measured, c.f.view);
      assert.equal(
        c.nodes.some((node) => node.className === 'sp-reorder-insertion-marker' && !node.removed),
        false,
      );
      assert.equal(c.landing.length, 0);
    } finally {
      c.destroy();
    }
  });
}

/** A synchronous actual publication observer may retire the old gesture and fully prepare its successor before apply returns. */
test("reentrant preview publication cannot clear or dim a newer hold's accepted marker", () => {
  const { f, nodes } = paintFixture();
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    dragPointer(f.owner, pointer());
    drain(f);
    const source = readExactSource(f.state),
      selection = f.state.selection;
    let replacement: (typeof nodes)[number] | undefined;
    let entered = false;
    f.observe((transaction) => {
      if (!transaction.effects.some((effect) => effect.is(installReorderPreview))) return;
      f.observe(null);
      entered = true;
      f.owner.close();
      f.owner.pointerdown(pointer(2));
      drain(f);
      dragPointer(f.owner, pointer(2));
      drain(f);
      replacement = nodes.findLast(
        (node) => node.className === 'sp-reorder-insertion-marker' && !node.removed,
      );
      assert.ok(replacement);
      assert.equal(replacement.style.opacity, '1');
    });
    f.owner.pointermove(pointer(1, 20, 55));
    drain(f);
    assert.equal(entered, true);
    assert.ok(replacement);
    assert.equal(replacement.removed, false);
    assert.equal(replacement.style.opacity, '1');
    assert.deepEqual([...f.captures], [2]);
    assert.equal(readExactSource(f.state), source);
    assert.equal(f.state.selection, selection);
    assert.equal(undoDepth(f.state), 0);
    assert.equal(f.owner.pointerup(pointer(1)), false);
    assert.equal(f.owner.pointerup(pointer(2)), true);
    drain(f);
    assert.equal(readExactSource(f.state), source);
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});
