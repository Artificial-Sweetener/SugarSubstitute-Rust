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
/** Actual native refinement retains the visible automatic row without granting metadata ranking explicit intent. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  completionSession,
  installCompletion,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';
import {
  completionState,
  refreshCompletionState,
  sessionResult,
} from './completion-session-fixture.ts';
import { completionAcceptanceTransaction } from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-acceptance.ts';

test('Backspace refinement retains the sole visible automatic identity when a higher ranked row becomes available', () => {
  let state = completionState('han');
  const original = sessionResult(state, [{ name: 'hands', backend: 'hands.pt' }]);
  state = state.update({ effects: installCompletion.of(original) }).state;
  const visible = state.field(completionSession)!;
  state = state.update({
    changes: { from: 2, to: 3 },
    selection: { anchor: 2 },
    userEvent: 'delete.backward',
  }).state;
  assert.equal(state.field(completionSession)!.phase, 'refreshing');
  assert.equal(state.field(completionSession)!.id, visible.id);
  assert.equal(completionAcceptanceTransaction(state, original.options[0]!, 'keyboard'), null);
  state = refreshCompletionState(state);
  const refined = sessionResult(state);
  state = state.update({ effects: installCompletion.of(refined) }).state;
  const published = state.field(completionSession)!;
  assert.equal(published.selected, 1);
  assert.equal(published.result.options[published.selected]!.label, 'hands');
  assert.equal(published.id, visible.id);
  assert.equal(published.phase, 'ready');
  assert.equal(state.doc.toString(), 'ha');
  assert.equal(state.selection.main.head, 2);
});

test('forward native refinement preserves a present automatic identity independently of its new rank', () => {
  let state = completionState('ha');
  const original = sessionResult(state);
  state = state.update({ effects: installCompletion.of(original) }).state;
  const visible = state.field(completionSession)!;
  state = state.update({
    changes: { from: 2, insert: 'i' },
    selection: { anchor: 3 },
    userEvent: 'input.type',
  }).state;
  assert.equal(state.field(completionSession)!.phase, 'refreshing');
  state = refreshCompletionState(state);
  const refined = sessionResult(state, [
    { name: 'hairstyle', backend: 'hairstyle.pt' },
    { name: 'hair', backend: 'hair.pt' },
  ]);
  state = state.update({ effects: installCompletion.of(refined) }).state;
  assert.equal(state.field(completionSession)!.selected, 1);
  assert.equal(state.field(completionSession)!.id, visible.id);
  assert.equal(state.doc.toString(), 'hai');
  assert.equal(state.selection.main.head, 3);
});

test('a missing refined identity resets to rank first and does not create explicit selection intent', () => {
  let state = completionState('han');
  const original = sessionResult(state, [{ name: 'hands', backend: 'hands.pt' }]);
  state = state.update({ effects: installCompletion.of(original) }).state;
  state = state.update({
    changes: { from: 2, to: 3 },
    selection: { anchor: 2 },
    userEvent: 'delete.backward',
  }).state;
  state = refreshCompletionState(state);
  const missing = sessionResult(state, [{ name: 'hair', backend: 'hair.pt' }]);
  state = state.update({ effects: installCompletion.of(missing) }).state;
  assert.equal(state.field(completionSession)!.selected, 0);
  const ranked = sessionResult(state, [
    { name: 'hands', backend: 'hands.pt' },
    { name: 'hair', backend: 'hair.pt' },
  ]);
  state = state.update({ effects: installCompletion.of(ranked) }).state;
  assert.equal(state.field(completionSession)!.selected, 0);
  assert.equal(state.field(completionSession)!.result.options[0]!.label, 'hands');
});

test('an outside native edit retires the old session before the current query publishes', () => {
  let state = completionState('hands, han');
  const original = sessionResult(state, [{ name: 'hands', backend: 'hands.pt' }]);
  state = state.update({ effects: installCompletion.of(original) }).state;
  const visible = state.field(completionSession)!;
  state = state.update({
    changes: { from: 0, insert: 'x' },
    selection: { anchor: 11 },
    userEvent: 'input.type',
  }).state;
  assert.equal(state.field(completionSession), null);
  state = refreshCompletionState(state);
  const current = sessionResult(state);
  state = state.update({ effects: installCompletion.of(current) }).state;
  assert.notEqual(state.field(completionSession)!.id, visible.id);
  assert.equal(state.field(completionSession)!.selected, 0);
});

test('retaining an implicit row through refinement leaves later metadata-only publication rank first', () => {
  let state = completionState('han');
  const original = sessionResult(state, [{ name: 'hands', backend: 'hands.pt' }]);
  state = state.update({ effects: installCompletion.of(original) }).state;
  state = state.update({
    changes: { from: 2, to: 3 },
    selection: { anchor: 2 },
    userEvent: 'delete.backward',
  }).state;
  state = refreshCompletionState(state);
  const refined = sessionResult(state);
  state = state.update({ effects: installCompletion.of(refined) }).state;
  assert.equal(state.field(completionSession)!.selected, 1);
  const before = state.selection;
  const refreshed = sessionResult(state);
  state = state.update({ effects: installCompletion.of(refreshed) }).state;
  assert.equal(state.field(completionSession)!.selected, 0);
  assert.equal(state.field(completionSession)!.result.options[0]!.label, 'hair');
  assert.equal(state.selection, before);
  assert.equal(state.doc.toString(), 'ha');
});
