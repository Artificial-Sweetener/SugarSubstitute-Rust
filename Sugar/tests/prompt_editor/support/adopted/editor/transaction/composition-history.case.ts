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
/** Supported native history initialization reuses the actual opaque value and releases its temporary provider. */
import test from 'node:test';
import assert from 'node:assert/strict';
import { Compartment, EditorSelection, EditorState } from '@codemirror/state';
import { history, historyField, undo, undoDepth, redoDepth } from '@codemirror/commands';
import { compositionHistoryRestoration } from '../../../../../../frontend/sugarprompt/editor/src/composition-history.ts';

for (const undone of [false, true])
  test(`native checkpoint retains the actual ${undone ? 'redo' : 'undo'} value, current selection and finite initializer`, () => {
    const initialization = new Compartment();
    let state = EditorState.create({
      doc: 'base',
      extensions: [history(), initialization.of([])],
    });
    state = state.update({
      changes: { from: 0, insert: 'x' },
      userEvent: 'input.type',
    }).state;
    if (undone)
      assert.equal(
        undo({
          state,
          dispatch: (transaction) => {
            state = transaction.state;
          },
        }),
        true,
      );
    const checkpoint = state.field(historyField);
    const source = state.doc;
    state = state.update({
      changes: { from: 1, insert: 'candidate' },
      userEvent: 'input.type.compose.start',
    }).state;
    state = state.update({
      changes: { from: 1, to: 10 },
      selection: EditorSelection.cursor(1),
      userEvent: 'input.type.compose',
    }).state;
    assert.equal(state.doc.eq(source), true);
    const [restore, release] = compositionHistoryRestoration(state, initialization, checkpoint);
    assert.equal(restore.startState, state);
    assert.equal(release.startState, restore.state);
    assert.equal(release.state.field(historyField), checkpoint);
    assert.deepEqual(initialization.get(release.state), []);
    assert.equal(release.state.doc, state.doc);
    assert.equal(release.state.selection, state.selection);
    assert.equal(undoDepth(release.state), undone ? 0 : 1);
    assert.equal(redoDepth(release.state), undone ? 1 : 0);
  });
