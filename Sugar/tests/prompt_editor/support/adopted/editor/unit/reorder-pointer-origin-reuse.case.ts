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
/** Actual Session/measurement proof observes avoided origin reads without substituting Core or Presentation. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { fixture, pointer, dragPointer } from './reorder-session-fixture.ts';
import { drain } from './reorder-marker-fixture.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { currentReorderPaintTargets } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { markerCurrentnessFixture } from './reorder-marker-currentness-fixture.ts';

test('unchanged canonical pointer placement retains accepted unavailable paint without full origin traversal', () => {
  const f = fixture();
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    dragPointer(f.owner);
    drain(f);
    f.owner.pointermove(pointer(1, 20, 55));
    drain(f);
    assert.equal(f.state.field(reorderPreview).native, false);
    assert.deepEqual(f.state.field(reorderPreview).value?.order.order, [1, 2, 0]);
    f.owner.pointermove(pointer(1, 20, 5));
    drain(f);
    assert.equal(f.state.field(reorderPreview).native, false);
    assert.deepEqual(f.state.field(reorderPreview).value?.order.order, [0, 1, 2]);
    const source = readExactSource(f.state),
      selection = f.state.selection,
      root = f.paintedRoot,
      preview = f.state.field(reorderPreview).value;
    assert.ok(preview);
    assert.ok(root);
    const targets = currentReorderPaintTargets(f.view);
    assert.ok(targets);
    const nodes = targets.filter((target) => target.kind === 'chip').map((target) => target.node);
    assert.equal(nodes.length, 3);
    const unread = targets.find((target) => target.kind === 'chip' && target.index === 2);
    assert.ok(unread);
    let reads = 0;
    unread.node.getClientRects = () => {
      reads++;
      throw new Error('unused-full-origin-read');
    };
    f.owner.pointermove(pointer(1, 21, 5));
    drain(f);
    assert.equal(reads, 0);
    assert.equal(f.paintedRoot, root);
    assert.equal(f.state.field(reorderPreview).value, preview);
    assert.equal(readExactSource(f.state), source);
    assert.equal(f.state.selection, selection);
    assert.equal(undoDepth(f.state), 0);
    f.owner.pointerup(pointer(1, 21, 5));
    drain(f);
    f.owner.pointermove(pointer(1, 21, 5));
    drain(f);
    assert.equal(reads, 0);
  } finally {
    f.destroy();
  }
});

test('native refresh replaces held descendant footprint with unchanged outer coordinates', () => {
  const c = markerCurrentnessFixture();
  try {
    c.replaceOrder();
    assert.equal(c.f.state.field(reorderPreview).native, false);
    const first = c.issue(),
      before = first.read(c.f.view);
    assert.ok(before && typeof before === 'object' && 'landingRects' in before);
    first.write!(before, c.f.view);
    const target = currentReorderPaintTargets(c.f.view)?.find(
      (target) => target.kind === 'chip' && target.index === c.input.held,
    );
    assert.ok(target);
    const rect = target.node.getBoundingClientRect(),
      original = target.node.getClientRects.bind(target.node);
    target.node.getBoundingClientRect = () => rect;
    target.node.getClientRects = () =>
      Array.from(original(), (box) => ({
        // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
        ...box,
        left: box.left + 2,
        right: box.right + 2,
      })) as unknown as DOMRectList;
    const next = c.issue(),
      after = next.read(c.f.view);
    assert.ok(after && typeof after === 'object' && 'landingRects' in after);
    assert.notDeepEqual(after.landingRects, before.landingRects);
    assert.deepEqual(target.node.getBoundingClientRect(), rect);
  } finally {
    c.destroy();
  }
});
