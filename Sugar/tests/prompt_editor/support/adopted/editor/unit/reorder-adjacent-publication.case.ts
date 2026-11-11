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
/** Actual current publications and CodeMirror history prove adjacent vacancy movement with exact CRLF and scalar offsets. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, Transaction } from '@codemirror/state';
import { undoDepth, undo, redo } from '@codemirror/commands';
import { publicationFixture } from './reorder-publication-fixture.ts';
import { key } from './reorder-session-fixture.ts';
import {
  readExactSource,
  exactReplacement,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import {
  prepareNativeStructure,
  installSemanticStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';

for (const endings of [
  ['\n', '\n'],
  ['\r\n', '\r\n'],
  ['\r\n', '\n'],
] as const)
  for (const early of [false, true])
    test(`FIFO adjacent Down retains scalar caret and exact history, ending lengths=${endings.map((e) => e.length).join('/')}, early=${early}`, () => {
      const source = `🦊${endings[0]}b${endings[1]}c`,
        expected = `🦊${endings[0]}${endings[1]}c, b`,
        transport = publicationFixture('🦊\nb\nc', 3),
        { f } = transport;
      try {
        f.view.dispatch({
          ...exactReplacement(f.state, 0, readExactSource(f.state).length, source),
          selection: EditorSelection.cursor(3),
          annotations: Transaction.addToHistory.of(false),
        });
        f.view.dispatch({
          effects: installSemanticStructure.of({
            purpose: 'native',
            prepared: prepareNativeStructure(f.state),
          }),
          annotations: Transaction.addToHistory.of(false),
        });
        assert.equal(readExactSource(f.state), source);
        f.owner.keydown(key('Alt'));
        transport.settle();
        const selection = f.state.selection;
        // Both keys are admitted before the first hidden publication settles.
        f.owner.keydown(key('ArrowDown'));
        f.owner.keydown(key('ArrowDown'));
        if (early) f.owner.keyup(key('Alt'));
        transport.settle();
        if (!early) {
          assert.equal(readExactSource(f.state), source);
          assert.equal(f.state.selection, selection);
          assert.equal(undoDepth(f.state), 0);
          f.owner.keyup(key('Alt'));
          transport.settle();
        }
        assert.equal(readExactSource(f.state), expected);
        assert.equal(f.state.selection.main.head, 7);
        assert.equal(undoDepth(f.state), 1);
        assert.equal(f.state.field(reorderPreview).value, null);
        assert.equal(f.owner.blocksInput(), false);
        assert.equal(undo(f.view), true);
        assert.equal(readExactSource(f.state), source);
        assert.equal(f.state.selection.main.head, 3);
        assert.equal(redo(f.view), true);
        assert.equal(readExactSource(f.state), expected);
        // Native redo places the caret after the whole replacement.
        assert.equal(f.state.selection.main.head, 8);
        assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
      } finally {
        f.destroy();
      }
    });
