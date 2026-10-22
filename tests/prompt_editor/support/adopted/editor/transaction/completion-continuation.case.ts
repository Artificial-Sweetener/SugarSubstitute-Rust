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
/** Native state proves prepared complete replacements, conservative retirement and exact metadata reuse without browser costs. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, StateEffect, type Transaction } from '@codemirror/state';
import { undo, redo } from '@codemirror/commands';
import {
  readExactSource,
  lineEndings,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { authorship } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { completionSession } from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';
import {
  completionAcceptanceTransaction,
  completionOptionTransaction,
} from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-acceptance.ts';
import { preparedTagCompletion } from './tag-completion-fixture.ts';

test('deleted shared prefix and repeated refinement reuse actual prepared output, exact endings and native undo order', () => {
  const original = preparedTagCompletion();
  let state = original.state;
  const option = original.result.options[0]!,
    splice = option.acceptance.keyboard.splice!;
  assert.equal(splice.from, 3);
  assert.ok(splice.to >= 6);
  const endings = state.field(lineEndings);
  state = state.update({
    changes: { from: 5, to: 6 },
    selection: { anchor: 5 },
    userEvent: 'delete.backward',
  }).state;
  state = state.update({
    changes: { from: 5, insert: 'i' },
    selection: { anchor: 6 },
    userEvent: 'input.type',
  }).state;
  assert.equal(state.field(semanticStructures).native, null);
  assert.equal(state.field(completionSession)!.result, original.result);
  assert.equal(
    state.field(completionSession)!.result.options[0]!.acceptance.keyboard.splice,
    splice,
  );
  assert.equal(completionAcceptanceTransaction(state, option, 'keyboard'), null);
  const accepted = completionOptionTransaction(state, original.result, 0, 'keyboard')!;
  assert.ok(accepted);
  assert.equal(accepted.effects[0], splice.effects[0]);
  state = accepted.state;
  assert.equal(readExactSource(state), '🙂\r\nhands, \rtail');
  assert.equal(state.selection.main.head, 10);
  assert.equal(state.field(lineEndings), endings);
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
  assert.equal(readExactSource(state), '🙂\r\nhai\rtail');
  assert.equal(state.selection.main.head, 6);
  assert.equal(redo(target), true);
  assert.equal(readExactSource(state), '🙂\r\nhands, \rtail');
});

test('literal completion keeps prepared authoring output while native prefix changes inside emphasis', () => {
  const { state: initial, result } = preparedTagCompletion('((re))\r\ntail', 3, true);
  assert.equal(initial.field(authorship).generated.size, 1);
  const prepared = result.options[0]!.acceptance.keyboard.edit.transactions[0]!.state;
  const state = initial.update({
    changes: { from: 3, insert: 'a' },
    selection: { anchor: 4 },
    userEvent: 'input.type',
  }).state;
  const accepted = completionOptionTransaction(state, result, 0, 'keyboard')!;
  assert.ok(accepted);
  assert.equal(readExactSource(accepted.state), '(real \\(version\\), :1.21)\r\ntail');
  assert.equal(prepared.field(authorship).generated.size, 1);
  assert.equal(accepted.state.field(authorship), prepared.field(authorship));
  assert.deepEqual(accepted.newSelection, prepared.selection);
});

test('syntax changes, insufficient prefixes, unrelated edits, composition and context retirement cannot use the prepared splice', () => {
  const { state, result } = preparedTagCompletion('han', 3);
  for (const character of ['(', ':', '\\', '\n', "'", '"']) {
    const next = state.update({
      changes: { from: 3, insert: character },
      selection: { anchor: 3 + character.length },
      userEvent: 'input.type',
    }).state;
    assert.equal(completionOptionTransaction(next, result, 0, 'keyboard'), null);
  }
  const shortened = state.update({
    changes: { from: 1, to: 3 },
    selection: { anchor: 1 },
    userEvent: 'delete.backward',
  }).state;
  assert.equal(completionOptionTransaction(shortened, result, 0, 'keyboard'), null);
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
  assert.ok(preparedTagCompletion('soft ght', 5).result.options[0]!.acceptance.keyboard.splice);
});

test('right text chooses the actual prepared normalized branch after insertion or deletion without re-preparing source', () => {
  for (const [deleting, authored] of [
    [false, false],
    [true, false],
    [false, true],
    [true, true],
  ]) {
    const body = deleting ? 'soft lightting' : 'soft liting';
    const { state: initial, result } = preparedTagCompletion(
      (authored ? `((${body}))` : body) + '\r\ntail',
      (deleting ? 10 : 7) + (authored ? 1 : 0),
      authored,
    );
    const index = result.options.findIndex((option) => option.label === 'soft lighting');
    assert.ok(index >= 0);
    const option = result.options[index]!,
      chosen = deleting
        ? option.acceptance.keyboard.right!.splice
        : option.acceptance.keyboard.splice!;
    assert.ok(option.acceptance.keyboard.right && option.acceptance.keyboard.splice);
    assert.ok(chosen.effects.length > 0);
    const target = initial.update({
      changes: { from: chosen.from, to: chosen.to, insert: chosen.insert },
      effects: chosen.effects,
      selection: chosen.selection,
    }).state;
    const caret = initial.selection.main.head;
    const refined = initial.update({
      changes: deleting ? { from: caret - 1, to: caret } : { from: caret, insert: 'ght' },
      selection: { anchor: caret + (deleting ? -1 : 3) },
      userEvent: deleting ? 'delete.backward' : 'input.type',
    }).state;
    assert.equal(refined.field(semanticStructures).native, null);
    assert.equal(refined.field(completionSession)!.result, result);
    const accepted = completionOptionTransaction(refined, result, index, 'keyboard')!;
    assert.ok(accepted);
    assert.equal(
      readExactSource(accepted.state),
      (authored ? '(' : '') +
        (deleting ? 'soft lighting, ' : 'soft lighting, ting') +
        (authored ? ':1.21)' : '') +
        '\r\ntail',
    );
    assert.equal(accepted.state.selection.main.head, 15 + (authored ? 1 : 0));
    assert.equal(accepted.effects[0], chosen.effects[0]);
    assert.equal(accepted.state.field(lineEndings), initial.field(lineEndings));
    assert.equal(accepted.state.field(authorship), target.field(authorship));
    assert.equal(target.field(authorship).generated.size, authored ? 1 : 0);
  }
});

test('literal escapes retain prepared output after content refinement but an insertion inside the stored escape rejects', () => {
  const { state: initial, result } = preparedTagCompletion('real \\(ve\r\ntail', 9);
  assert.ok(result.options[0]!.acceptance.keyboard.splice);
  const refined = initial.update({
    changes: { from: 9, insert: 'r' },
    selection: { anchor: 10 },
    userEvent: 'input.type',
  }).state;
  const accepted = completionOptionTransaction(refined, result, 0, 'keyboard')!;
  assert.ok(accepted);
  assert.equal(readExactSource(accepted.state), 'real \\(version\\), \r\ntail');
  const split = initial.update({
    changes: { from: 6, insert: 'x' },
    selection: { anchor: 10 },
    userEvent: 'input.type',
  }).state;
  assert.equal(completionOptionTransaction(split, result, 0, 'keyboard'), null);
});

test('fixed literal apostrophes reuse actual prepared output across ASCII and astral name refinement', () => {
  for (const [source, inserted, expected] of [
    ["o'cloc", 'k', "o'clock time, "],
    ["𐐀'βe", 't', "𐐀'βeta time, "],
  ] as const) {
    const { state: initial, result } = preparedTagCompletion(source, source.length);
    assert.ok(result.options[0]!.acceptance.keyboard.splice);
    const refined = initial.update({
      changes: { from: source.length, insert: inserted },
      selection: { anchor: source.length + inserted.length },
      userEvent: 'input.type',
    }).state;
    assert.equal(refined.field(completionSession)!.result, result);
    const accepted = completionOptionTransaction(refined, result, 0, 'keyboard')!;
    assert.ok(accepted);
    assert.equal(readExactSource(accepted.state) === expected, true);
    assert.equal(accepted.effects[0], result.options[0]!.acceptance.keyboard.splice!.effects[0]);
  }
});

test("removing an apostrophe's adjacent letter retires reuse when the quote becomes structural", () => {
  const { state: initial, result } = preparedTagCompletion("o'cloc", 6);
  const changed = initial.update({
    changes: { from: 0, to: 1 },
    selection: { anchor: 5 },
    userEvent: 'delete.backward',
  }).state;
  assert.equal(changed.field(completionSession)!.result, result);
  assert.equal(changed.field(completionSession)!.consumedShift, null);
  assert.equal(completionOptionTransaction(changed, result, 0, 'keyboard'), null);
});
