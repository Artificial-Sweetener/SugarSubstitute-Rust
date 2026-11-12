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
/** Actual CodeMirror source/context epochs prove prepared edits without mocking native transactions or history. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, EditorState, StateEffect, Text } from '@codemirror/state';
import { history, undo, undoDepth } from '@codemirror/commands';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import {
  exactReplacement,
  exactSourceExtension,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  editorFreshness,
  invalidateCommandContext,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  PreparedSourceEdits,
  type PreparedSourceEdit,
} from '../../../../../../frontend/sugarprompt/editor/src/prepared-edits.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import {
  defaultPreferences,
  resolveFeatureProfile,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';

/** Build the real native ownership boundary with exact source and history. */
function stateFor(source = '🙂\r\ncat, dog'): EditorState {
  return EditorState.create({
    doc: Text.of(encodeSource(source).logical.split('\n')),
    extensions: [exactSourceExtension(source), editorFreshness, history()],
  });
}

/** Require successful preparation, keeping expected editor restrictions explicit in their own cases. */
function commandFor(
  owner: PreparedSourceEdits,
  state: EditorState,
  from = 4,
  to = 7,
  source = 'owl',
): PreparedSourceEdit {
  const prepared = owner.prepare(state, false, from, to, source);
  assert.equal(prepared.status, 'ready');
  if (prepared.status !== 'ready') throw new Error('Preparation unexpectedly blocked');
  assert.equal(Object.isFrozen(prepared.command), true);
  assert.equal(JSON.stringify(prepared.command), '{"kind":"source-edit"}');
  return prepared.command;
}

test('prepared exact edits survive selection, use one native history event and are consumed once', () => {
  const owner = new PreparedSourceEdits();
  let state = stateFor();
  const command = commandFor(owner, state);
  state = state.update({ selection: { anchor: 10, head: 7 } }).state;
  const selected = state.selection.main;
  const edit = owner.take(state, false, command);
  assert.equal(edit.status, 'ready');
  if (edit.status !== 'ready') throw new Error('Fresh command rejected');
  state = state.update(edit.transaction).state;
  assert.equal(readExactSource(state), '🙂\r\nowl, dog');
  assert.equal(undoDepth(state), 1);
  assert.equal(state.selection.main.head, 6);
  assert.deepEqual(owner.take(state, false, command), {
    status: 'rejected',
    reason: 'consumed-command',
  });
  assert.equal(
    undo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(state), '🙂\r\ncat, dog');
  assert.equal(state.selection.main.anchor, selected.anchor);
  assert.equal(state.selection.main.head, selected.head);
});

test('equal bytes restored by undo or baseline cannot revive a prepared source target', () => {
  for (const mode of ['edit', 'undo', 'baseline', 'newlines'] as const) {
    const owner = new PreparedSourceEdits();
    let state = stateFor();
    const command = commandFor(owner, state);
    if (mode === 'baseline') state = stateFor();
    else {
      state = state.update(
        mode === 'newlines'
          ? exactReplacement(state, 2, 4, '\n')
          : exactReplacement(state, 4, 7, 'fox'),
      ).state;
      if (mode === 'undo')
        undo({
          state,
          dispatch: (transaction) => {
            state = transaction.state;
          },
        });
    }
    const before = state;
    assert.deepEqual(
      owner.take(state, false, command),
      { status: 'rejected', reason: 'stale-source' },
      mode,
    );
    assert.equal(state, before);
  }
});

test('catalog, feature and editability replacement retire context even when an earlier setting is restored', () => {
  const configuration = new Compartment();
  const profile = resolveFeatureProfile(defaultPreferences, {
    mode: 'library',
  });
  let state = stateFor();
  state = state.update({
    effects: StateEffect.appendConfig.of(configuration.of(featureProfile.of(profile))),
  }).state;
  for (const mode of ['catalog', 'profile', 'readonly'] as const) {
    const owner = new PreparedSourceEdits();
    const command = commandFor(owner, state);
    if (mode === 'catalog')
      state = state.update({
        effects: invalidateCommandContext.of(null),
      }).state;
    else if (mode === 'profile') {
      state = state.update({
        effects: configuration.reconfigure(
          featureProfile.of(resolveFeatureProfile(defaultPreferences, { mode: 'workflow' })),
        ),
      }).state;
      state = state.update({
        effects: configuration.reconfigure(featureProfile.of(profile)),
      }).state;
    } else {
      state = state.update({
        effects: configuration.reconfigure(EditorState.readOnly.of(true)),
      }).state;
      assert.deepEqual(owner.take(state, false, command), {
        status: 'rejected',
        reason: 'read-only',
      });
      state = state.update({
        effects: configuration.reconfigure(featureProfile.of(profile)),
      }).state;
    }
    assert.deepEqual(
      owner.take(state, false, command),
      { status: 'rejected', reason: 'stale-context' },
      mode,
    );
  }
});

test('unchanged preparation and acceptance preserve state, selection and history, while invalid boundaries reject', () => {
  const owner = new PreparedSourceEdits();
  const state = stateFor();
  assert.deepEqual(owner.take(state, false, commandFor(owner, state, 4, 7, 'cat')), {
    status: 'unchanged',
  });
  assert.equal(undoDepth(state), 0);
  assert.equal(state.selection.main.head, 0);
  assert.throws(() => owner.prepare(state, false, 3, 4, 'x'), RangeError);
  assert.throws(() => owner.prepare(state, false, 7, 4, 'x'), RangeError);
  assert.throws(() => owner.prepare(state, false, 0, 100, 'x'), RangeError);
});

test('composition, foreign handles and disposal cannot dispatch an edit', () => {
  const owner = new PreparedSourceEdits();
  const state = stateFor();
  const command = commandFor(owner, state);
  const readOnly = state.update({
    effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)),
  }).state;
  assert.deepEqual(owner.prepare(readOnly, false, 4, 7, 'owl'), {
    status: 'blocked',
    reason: 'read-only',
  });
  assert.deepEqual(owner.prepare(state, true, 4, 7, 'owl'), {
    status: 'blocked',
    reason: 'composing',
  });
  assert.deepEqual(owner.take(state, true, command), {
    status: 'rejected',
    reason: 'composing',
  });
  assert.deepEqual(new PreparedSourceEdits().take(state, false, command), {
    status: 'rejected',
    reason: 'foreign-command',
  });
  assert.deepEqual(owner.take(state, false, { ...command }), {
    status: 'rejected',
    reason: 'foreign-command',
  });
  owner.dispose();
  assert.deepEqual(owner.take(state, false, command), {
    status: 'rejected',
    reason: 'disposed',
  });
  assert.deepEqual(owner.prepare(state, false, 4, 7, 'owl'), {
    status: 'blocked',
    reason: 'disposed',
  });
});
