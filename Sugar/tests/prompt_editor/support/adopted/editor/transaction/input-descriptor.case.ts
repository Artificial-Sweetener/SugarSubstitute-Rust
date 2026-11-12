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
/** Actual native transactions prove the accepted-input protocol without a mounted input lifetime. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, Transaction } from '@codemirror/state';
import { authorshipExtension } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import { exactSourceExtension } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import { inputEdit } from '../../../../../../frontend/sugarprompt/editor/src/input-descriptor.ts';

/** Construct actual source and provenance owners without a mounted input lifetime. */
function initial(source: string): EditorState {
  return EditorState.create({
    doc: source,
    extensions: [exactSourceExtension(source), authorshipExtension],
  });
}

test('accepted typing observes the actual native document, directional selection, context and transaction time', () => {
  const state = initial('cat');
  const transaction = state.update({
    changes: { from: 1, insert: '🙂' },
    selection: { anchor: 3, head: 1 },
    userEvent: 'input.type.compose',
    annotations: Transaction.time.of(40),
  });
  const edit = inputEdit(transaction)!;
  assert.deepEqual(
    [edit.from, edit.to, edit.deletion, edit.manual, edit.protected, edit.time, edit.userEvent],
    [1, 3, false, false, false, 40, 'input.type.compose'],
  );
  assert.equal(edit.doc, transaction.newDoc);
  assert.equal(edit.selection, transaction.newSelection);
  assert.equal(edit.profile, state.facet(featureProfile));
});

test('manual parenthesis escape insertion and removal retain their actual changed interval', () => {
  for (const [state, changes, event, expected] of [
    [initial('(cat)'), { from: 0, insert: '\\' }, 'input.type', [0, 1, false, true]],
    [initial('\\(cat)'), { from: 0, to: 1 }, 'delete.backward', [0, 0, true, true]],
  ] as const) {
    const edit = inputEdit(state.update({ changes, userEvent: event }))!;
    assert.deepEqual([edit.from, edit.to, edit.deletion, edit.manual], expected);
  }
});

test('non-input, selection-only and discontiguous native transactions cannot invent a pending authoring interval', () => {
  const state = initial('cat');
  for (const transaction of [
    state.update({ selection: { anchor: 1 } }),
    state.update({
      changes: { from: 1, insert: 'x' },
      userEvent: 'input.weight',
    }),
    state.update({
      changes: [
        { from: 0, insert: 'x' },
        { from: 3, insert: 'y' },
      ],
      userEvent: 'input.type',
    }),
  ])
    assert.equal(inputEdit(transaction), null);
});
