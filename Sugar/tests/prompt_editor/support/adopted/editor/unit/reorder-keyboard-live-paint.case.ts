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
/** Actual canonical live DOM remains complete through finite external keyboard collision measurement and no-op settlement. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { publicationFixture } from './reorder-publication-fixture.ts';
import { key } from './reorder-session-fixture.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';

/** Observe genuinely mounted production chip nodes rather than reconstructing expected canonical paint. */
function chips(root: Node): readonly HTMLElement[] {
  const result: HTMLElement[] = [];
  const visit = (node: Node): void => {
    const element = node as HTMLElement;
    if (element.dataset?.reorderChip !== undefined) result.push(element);
    for (const child of Array.from(node.childNodes)) visit(child);
  };
  visit(root);
  return result;
}

test('over-budget edge no-op keeps actual complete mounted glyph nodes, source, selection and history throughout pending phases', () => {
  const source = `${'plainword '.repeat(2000)}, beta, tail`,
    transport = publicationFixture(source, 0),
    { f } = transport;
  try {
    f.owner.keydown(key('Alt'));
    transport.settle();
    for (const expected of [
      [1, 0, 2],
      [1, 2, 0],
    ]) {
      f.owner.keydown(key('ArrowRight'));
      transport.settle();
      assert.deepEqual(f.state.field(reorderPreview).value!.order.order, expected);
    }
    f.draw();
    const root = f.paintedRoot!;
    assert.ok(root);
    const retained = chips(root),
      selection = f.state.selection;
    assert.deepEqual(
      retained.map((node) => Number(node.dataset.reorderChip)),
      [1, 2, 0],
    );
    f.owner.keydown(key('ArrowRight'));
    let observed = 0;
    for (let turn = 0; turn < 32 && (f.jobs.length || f.tasks.length); turn++) {
      if (f.jobs.length) transport.measure();
      if (f.tasks.length) f.task();
      f.draw();
      assert.equal(f.paintedRoot, root);
      assert.deepEqual(chips(root), retained);
      assert.deepEqual(f.state.field(reorderPreview).value!.order.order, [1, 2, 0]);
      assert.equal(f.state.field(reorderPreview).value!.presentation, 'complete');
      assert.equal(readExactSource(f.state) === source, true);
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
      if (transport.collisionTargets().length) observed++;
    }
    assert.ok(observed > 0);
    assert.equal(f.jobs.length + f.tasks.length, 0);
    f.owner.keydown(key('Escape'));
    transport.settle();
    assert.equal(readExactSource(f.state) === source, true);
    assert.equal(undoDepth(f.state), 0);
    assert.equal(transport.collisionTargets().length, 0);
    assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
  } finally {
    f.destroy();
  }
});
