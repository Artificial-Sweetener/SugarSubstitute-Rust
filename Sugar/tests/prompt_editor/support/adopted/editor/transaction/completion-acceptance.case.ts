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
/** Accepted suggestions are built for the state at acceptance; rows ranked for an older prefix never apply. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, StateEffect, type Transaction } from '@codemirror/state';
import { undo, redo } from '@codemirror/commands';
import {
  readExactSource,
  lineEndings,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  semanticStructures,
  installSemanticStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { completionSession } from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';
import { prepareCompletionOptions } from '../../../../../../frontend/sugarprompt/editor/src/completion-options.ts';
import {
  completionAcceptanceTransaction,
  completionOptionTransaction,
} from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-acceptance.ts';
import { preparedTagCompletion, refreshedTagCompletion } from './tag-completion-fixture.ts';

/** Accept the fresh row with this label at the current state. */
function acceptFresh(current: EditorState, label: string): Transaction {
  const { state, result } = refreshedTagCompletion(current);
  const index = result.options.findIndex((option) => option.label === label);
  assert.ok(index >= 0, `missing fresh ${label}`);
  const accepted = completionOptionTransaction(state, result, index, 'keyboard');
  assert.ok(accepted);
  assert.equal(accepted.startState, state);
  return accepted;
}

test('typing inside the query makes visible rows stale; fresh rows accept with exact endings and undo order', () => {
  const original = preparedTagCompletion();
  const endings = original.state.field(lineEndings);
  const typed = original.state.update({
    changes: { from: 6, insert: 'd' },
    selection: { anchor: 7 },
    userEvent: 'input.type',
  }).state;
  assert.equal(typed.field(semanticStructures).native, null);
  assert.equal(typed.field(completionSession)!.result, original.result);
  // A row ranked for the earlier prefix is never applied to the newer text.
  assert.equal(completionOptionTransaction(typed, original.result, 0, 'keyboard'), null);
  assert.equal(
    completionAcceptanceTransaction(typed, original.result.options[0]!, 'keyboard'),
    null,
  );
  let state = acceptFresh(typed, 'hands').state;
  assert.equal(readExactSource(state), '🙂\r\nhands, \rtail');
  assert.equal(state.selection.main.head, 10);
  assert.deepEqual(state.field(lineEndings), endings);
  const target = {
    /** Native commands always observe the current real state. */
    get state() {
      return state;
    },
    /** Delegate changes only to the native command's transaction. */
    dispatch(transaction: Transaction) {
      state = transaction.state;
    },
  };
  state = state.update({
    changes: { from: 10, insert: 'X' },
    selection: { anchor: 11 },
    userEvent: 'input.type',
  }).state;
  assert.equal(undo(target), true);
  assert.equal(readExactSource(state), '🙂\r\nhands, \rtail');
  assert.equal(undo(target), true);
  assert.equal(readExactSource(state), '🙂\r\nhand\rtail');
  assert.equal(redo(target), true);
  assert.equal(readExactSource(state), '🙂\r\nhands, \rtail');
});

test('a candidate is not accepted for a prefix it no longer matches', () => {
  const original = preparedTagCompletion();
  const changed = original.state
    .update({ changes: { from: 5, to: 6 }, selection: { anchor: 5 }, userEvent: 'delete.backward' })
    .state.update({
      changes: { from: 5, insert: 'i' },
      selection: { anchor: 6 },
      userEvent: 'input.type',
    }).state;
  assert.equal(completionOptionTransaction(changed, original.result, 0, 'keyboard'), null);
  const fresh = prepareCompletionOptions(
    changed.update({
      effects: installSemanticStructure.of({
        purpose: 'native',
        prepared: prepareNativeStructure(changed),
      }),
    }).state,
  );
  assert.equal(fresh?.options.some((option) => option.label === 'hands') ?? false, false);
});

test('literal completion inside emphasis accepts the refined query with its authored weight', () => {
  const { state } = preparedTagCompletion('((re))\r\ntail', 3, true);
  const refined = state.update({
    changes: { from: 3, insert: 'a' },
    selection: { anchor: 4 },
    userEvent: 'input.type',
  }).state;
  assert.equal(
    readExactSource(acceptFresh(refined, 'real (version)').state),
    '(real \\(version\\), :1.21)\r\ntail',
  );
});

test('literal escapes and fixed apostrophes are accepted exactly after name refinement', () => {
  for (const [source, inserted, label, expected] of [
    ['real \\(ve\r\ntail', 'r', 'real (version)', 'real \\(version\\), \r\ntail'],
    ["o'cloc", 'k', "o'clock time", "o'clock time, "],
    ["𐐀'βe", 't', "𐐀'βeta time", "𐐀'βeta time, "],
  ] as const) {
    const caret = !source.includes('\r') ? source.length : source.indexOf('\r');
    const { state } = preparedTagCompletion(source, caret);
    const refined = state.update({
      changes: { from: caret, insert: inserted },
      selection: { anchor: caret + inserted.length },
      userEvent: 'input.type',
    }).state;
    assert.equal(readExactSource(acceptFresh(refined, label).state), expected);
  }
});

test('stale results, invalid rows, selection, composition, context and permission refuse acceptance', () => {
  const { state, result } = preparedTagCompletion('han', 3);
  assert.ok(completionOptionTransaction(state, result, 0, 'keyboard'));
  for (const character of ['(', ':', '\\', '\n', "'", '"']) {
    const next = state.update({
      changes: { from: 3, insert: character },
      selection: { anchor: 3 + character.length },
      userEvent: 'input.type',
    }).state;
    assert.equal(completionOptionTransaction(next, result, 0, 'keyboard'), null);
  }
  for (const edit of [
    { changes: { from: 3, insert: 'x' }, userEvent: 'input.type.compose' },
    { changes: { from: 0, to: 3, insert: 'other' } },
    { selection: { anchor: 2 } },
    { selection: { anchor: 3, head: 0 } },
    { effects: invalidateCommandContext.of(null) },
    { effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)) },
  ])
    assert.equal(
      completionOptionTransaction(state.update(edit).state, result, 0, 'keyboard'),
      null,
    );
  assert.equal(completionOptionTransaction(state, { ...result }, 0, 'keyboard'), null);
  for (const index of [-1, NaN, 0.5, 10])
    assert.equal(completionOptionTransaction(state, result, index, 'keyboard'), null);
});
