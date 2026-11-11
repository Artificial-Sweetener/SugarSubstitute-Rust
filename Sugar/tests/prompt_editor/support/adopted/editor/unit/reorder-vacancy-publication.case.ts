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
/** Actual session/CM transactions prove vacancy navigation history, settlement and currentness. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, Transaction } from '@codemirror/state';
import { undoDepth, undo, redo } from '@codemirror/commands';
import { publicationFixture } from './reorder-publication-fixture.ts';
import { key } from './reorder-session-fixture.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';

for (const early of [false, true])
  test(`repeated Up/Down/Up uses the current vacancy and commits once, early=${early}`, () => {
    const source = 'aaaaaaaaaa\nb\nc',
      transport = publicationFixture(source, 11),
      { f } = transport;
    try {
      f.owner.keydown(key('Alt'));
      transport.settle();
      const selection = f.state.selection;
      for (const [index, arrow] of ['ArrowUp', 'ArrowDown', 'ArrowUp'].entries()) {
        f.owner.keydown(key(arrow));
        if (early && index === 2) f.owner.keyup(key('Alt'));
        transport.settle();
      }
      if (!early) {
        assert.equal(readExactSource(f.state), source);
        assert.equal(undoDepth(f.state), 0);
        assert.equal(f.state.selection, selection);
        f.owner.keyup(key('Alt'));
        transport.settle();
      }
      assert.equal(readExactSource(f.state), 'b, aaaaaaaaaa\n\nc');
      assert.equal(undoDepth(f.state), 1);
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(undo(f.view), true);
      assert.equal(readExactSource(f.state), source);
      assert.equal(redo(f.view), true);
      assert.equal(readExactSource(f.state), 'b, aaaaaaaaaa\n\nc');
      assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
    } finally {
      f.destroy();
    }
  });

test('nullable initial preferredX at a trailing blank boundary restores source and history', () => {
  const source = 'a\nb',
    transport = publicationFixture(source, 2),
    { f } = transport;
  try {
    const selection = f.state.selection;
    f.owner.keydown(key('Alt'));
    transport.settle();
    for (let arrow = 0; arrow < 3; arrow++) {
      f.owner.keydown(key('ArrowDown'));
      transport.settle();
    }
    f.owner.keyup(key('Alt'));
    transport.settle();
    assert.equal(readExactSource(f.state), source);
    assert.equal(f.state.selection, selection);
    assert.equal(undoDepth(f.state), 0);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
  } finally {
    f.destroy();
  }
});

for (const retire of ['source', 'selection', 'dispose'] as const)
  test(`vacancy read cannot publish after ${retire} retirement`, () => {
    const source = 'aaaaaaaaaa\nb\nc',
      transport = publicationFixture(source, 11),
      { f } = transport;
    try {
      f.owner.keydown(key('Alt'));
      transport.settle();
      f.owner.keydown(key('ArrowUp'));
      transport.measure();
      f.task();
      const pending = f.jobs.shift()!;
      assert.ok(pending);
      const result = pending.read(f.view);
      if (retire === 'source')
        f.view.dispatch({
          changes: { from: 0, to: 1, insert: 'z' },
          annotations: Transaction.addToHistory.of(false),
        });
      else if (retire === 'selection') f.view.dispatch({ selection: EditorSelection.cursor(0) });
      else f.owner.destroy();
      const after = readExactSource(f.state);
      pending.write?.(result, f.view);
      transport.settle();
      assert.equal(readExactSource(f.state), after);
      assert.equal(undoDepth(f.state), 0);
      assert.equal(f.owner.blocksInput(), false);
      assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
    } finally {
      f.destroy();
    }
  });
