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
/** Actual immutable completion state and the native blur owner qualify focus loss independently of delayed CM update flags. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { EditorView } from '@codemirror/view';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { CompletionPreparation } from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion.ts';
import {
  completionSession,
  installCompletion,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';
import { completionState, sessionResult } from '../transaction/completion-session-fixture.ts';

/** Native focus is an independent observation; real CM state owns every suggestion and history transition. */
function fixture() {
  let state = completionState();
  const result = sessionResult(state);
  state = state.update({ effects: installCompletion.of(result) }).state;
  let focused = false,
    dispatches = 0;
  const view = {
    get state() {
      return state;
    },
    get hasFocus() {
      return focused;
    },
    plugin() {
      return owner;
    },
    dispatch(spec: Parameters<EditorView['state']['update']>[0]) {
      dispatches++;
      state = state.update(spec).state;
    },
  } as unknown as EditorView;
  const owner: CompletionPreparation = new CompletionPreparation(view);
  return {
    owner,
    view,
    focus(value: boolean) {
      focused = value;
    },
    dispatches: () => dispatches,
  };
}

test('native blur retires a genuine installed session without any focus-change update and preserves source, selection and history', async () => {
  const { owner, view, dispatches } = fixture(),
    before = view.state,
    original = before.field(completionSession);
  assert.ok(original);
  try {
    owner.blur();
    assert.equal(view.state, before);
    await Promise.resolve();
    assert.equal(view.state.field(completionSession), null);
    assert.equal(dispatches(), 1);
    assert.equal(view.state.doc, before.doc);
    assert.equal(view.state.selection, before.selection);
    assert.deepEqual(
      [undoDepth(view.state), redoDepth(view.state)],
      [undoDepth(before), redoDepth(before)],
    );
  } finally {
    owner.destroy();
  }
});

test('real refocus before deferred dismissal preserves the exact installed session and native state', async () => {
  const { owner, view, focus, dispatches } = fixture(),
    before = view.state;
  try {
    owner.blur();
    focus(true);
    await Promise.resolve();
    assert.equal(view.state, before);
    assert.equal(dispatches(), 0);
    assert.ok(view.state.field(completionSession));
  } finally {
    owner.destroy();
  }
});

test('mounted retirement prevents a queued native blur from publishing', async () => {
  const { owner, view, dispatches } = fixture(),
    before = view.state;
  owner.blur();
  owner.destroy();
  await Promise.resolve();
  assert.equal(view.state, before);
  assert.equal(dispatches(), 0);
});

test('native and observed focus loss converge on one actual dismissal without another source mutation', async () => {
  const { owner, view, dispatches } = fixture();
  try {
    owner.blur();
    owner.blur();
    await Promise.resolve();
    assert.equal(dispatches(), 1);
    assert.equal(view.state.field(completionSession), null);
    owner.blur();
    await Promise.resolve();
    assert.equal(dispatches(), 1);
  } finally {
    owner.destroy();
  }
});
