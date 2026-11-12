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
/** Real native state proves one suggestion selection owner and stale derived publication rejection. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, StateEffect } from '@codemirror/state';
import {
  editorFreshness,
  invalidateCommandContext,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  completionSession,
  installCompletion,
  selectCompletion,
  requestCompletionAcceptance,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';
import {
  completionWall,
  installCompletionWall,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-wall-state.ts';
import {
  completionState,
  sessionResult,
  refreshCompletionState,
} from './completion-session-fixture.ts';
import { prepareLoraWallLayout } from '../../../../../../frontend/sugarprompt/editor/src/lora-wall-layout.ts';
import { exactSourceExtension } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { completionAcceptanceTransaction } from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-acceptance.ts';

/** Install independent literal proposals at the native caret; acceptance itself has separate exact-source/history proof. */
function fixture() {
  let state = completionState();
  const result = sessionResult(state);
  state = state.update({ effects: installCompletion.of(result) }).state;
  return { state, result };
}

test('same-session selection is bounded, caret-independent and reuses actual prepared geometry', () => {
  let { state } = fixture();
  const session = state.field(completionSession)!;
  const wall = {
    result: session.result,
    layout: prepareLoraWallLayout([0.72, 0.72], 560),
    from: 0,
    to: 2,
    references: [],
  };
  state = state.update({ effects: installCompletionWall.of(wall) }).state;
  for (const index of [NaN, -1, 2, 0.5]) {
    const next = state.update({
      effects: selectCompletion.of({ result: session.result, index }),
    }).state;
    assert.equal(next.field(completionSession), session);
  }
  state = state.update({
    effects: selectCompletion.of({ result: session.result, index: 1 }),
  }).state;
  assert.equal(state.field(completionSession)!.selected, 1);
  assert.equal(state.field(completionWall), wall);
  assert.equal(state.selection.main.head, 2);
  assert.equal(state.doc.toString(), 'ha');
  state = state.update({ effects: installCompletion.of(null) }).state;
  assert.equal(state.field(completionWall), null);
  state = state.update({
    effects: [
      selectCompletion.of({ result: session.result, index: 0 }),
      installCompletionWall.of(wall),
    ],
  }).state;
  assert.equal(state.field(completionSession), null);
  assert.equal(state.field(completionWall), null);
});

test('compatible typing and deletion retain rows and selected identity, but never make old edits current', () => {
  const { state: initial, result } = fixture();
  let state = initial;
  state = state.update({
    effects: selectCompletion.of({ result, index: 1 }),
  }).state;
  const original = state.field(completionSession)!;
  const layout = prepareLoraWallLayout([0.72, 0.72], 560);
  state = state.update({
    effects: installCompletionWall.of({
      result,
      layout,
      from: 0,
      to: 2,
      references: [],
    }),
  }).state;
  state = state.update({
    changes: { from: 2, insert: 'n' },
    selection: { anchor: 3 },
    userEvent: 'input.type',
  }).state;
  const retained = state.field(completionSession)!;
  assert.equal(retained.id, original.id);
  assert.equal(retained.result, result);
  assert.equal(retained.selected, 1);
  assert.equal(retained.phase, 'refreshing');
  assert.equal(state.field(completionWall)!.layout, layout);
  assert.equal(completionAcceptanceTransaction(state, result.options[1]!, 'keyboard'), null);
  assert.equal(
    state.update({ effects: installCompletion.of(result) }).state.field(completionSession),
    retained,
  );
  state = state.update({
    effects: requestCompletionAcceptance.of({
      result,
      index: 1,
      activation: 'keyboard',
    }),
  }).state;
  assert.deepEqual(state.field(completionSession)!.acceptance, {
    identity: 'tag:hands',
    activation: 'keyboard',
  });
  state = refreshCompletionState(state);
  const next = sessionResult(state);
  state = state.update({ effects: installCompletion.of(next) }).state;
  assert.equal(state.field(completionSession)!.selected, 1);
  assert.equal(state.field(completionSession)!.presentation, original.presentation);
  assert.equal(state.field(completionWall)!.layout, layout);
  assert.equal(state.field(completionWall)!.result, next);
  state = state.update({
    changes: { from: 2, to: 3 },
    selection: { anchor: 2 },
    userEvent: 'delete.backward',
  }).state;
  assert.equal(state.field(completionSession)!.selected, 1);
  assert.equal(state.field(completionSession)!.acceptance, null);
  state = refreshCompletionState(state);
  const reordered = sessionResult(state, [
    { name: 'hands', backend: 'hands.pt' },
    { name: 'hair', backend: 'hair.pt' },
  ]);
  state = state.update({ effects: installCompletion.of(reordered) }).state;
  assert.equal(state.field(completionSession)!.selected, 0);
  assert.notEqual(state.field(completionSession)!.presentation, original.presentation);
  assert.equal(state.field(completionWall), null);
});

test('missing candidates reset selection and incompatible query owners start a fresh session', () => {
  const { state: initial, result } = fixture();
  let state = initial;
  state = state.update({
    effects: selectCompletion.of({ result, index: 1 }),
  }).state;
  const id = state.field(completionSession)!.id;
  state = state.update({
    effects: installCompletion.of({ ...result, options: [result.options[0]!] }),
  }).state;
  assert.equal(state.field(completionSession)!.selected, 0);
  assert.equal(state.field(completionSession)!.id, id);
  state = state.update({
    effects: installCompletion.of({
      ...result,
      query: { ...result.query, from: 1, contentFrom: 1 },
    }),
  }).state;
  assert.notEqual(state.field(completionSession)!.id, id);
  assert.equal(state.field(completionSession)!.selected, 0);
  for (const transaction of [
    {
      changes: { from: 0, insert: 'x' },
      selection: { anchor: 3 },
      userEvent: 'input.type',
    },
    {
      changes: { from: 2, insert: 'n' },
      selection: { anchor: 3 },
      userEvent: 'input.type.compose',
    },
    {
      changes: { from: 2, insert: 'n' },
      selection: { anchor: 3 },
      userEvent: 'input.paste',
    },
    {
      changes: { from: 2, insert: 'n' },
      selection: { anchor: 1 },
      userEvent: 'input.type',
    },
  ])
    assert.equal(state.update(transaction).state.field(completionSession), null);
});

test('deferred pointer intent retains its activation method and further input retires that request', () => {
  const { state: initial, result } = fixture();
  let state = initial.update({
    changes: { from: 2, insert: 'n' },
    selection: { anchor: 3 },
    userEvent: 'input.type',
  }).state;
  state = state.update({
    effects: requestCompletionAcceptance.of({
      result,
      index: 1,
      activation: 'pointer',
    }),
  }).state;
  assert.deepEqual(state.field(completionSession)!.acceptance, {
    identity: 'tag:hands',
    activation: 'pointer',
  });
  state = state.update({
    changes: { from: 3, insert: 'd' },
    selection: { anchor: 4 },
    userEvent: 'input.type',
  }).state;
  assert.equal(state.field(completionSession)!.acceptance, null);
});

test('source, context, caret, directional selection and permission retire candidates and reject their later publication', () => {
  const { state, result } = fixture();
  for (const transaction of [
    { changes: { from: 2, insert: 'x' } },
    { effects: invalidateCommandContext.of(null) },
    { selection: { anchor: 1 } },
    { selection: { anchor: 2, head: 0 } },
    {
      effects: StateEffect.reconfigure.of([
        exactSourceExtension('ha'),
        editorFreshness,
        completionSession,
        completionWall,
        EditorState.readOnly.of(true),
      ]),
    },
  ]) {
    const next = state.update(transaction).state;
    assert.equal(next.field(completionSession), null);
    assert.equal(
      next.update({ effects: installCompletion.of(result) }).state.field(completionSession),
      null,
    );
  }
});
