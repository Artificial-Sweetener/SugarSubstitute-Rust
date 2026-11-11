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
/** One native owner distinguishes automatic rank-first selection from explicit user choice across prepared publication. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect } from '@codemirror/state';
import { completionState, sessionResult } from './completion-session-fixture.ts';
import {
  completionSession,
  installCompletion,
  selectCompletion,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';

/** Real issued candidate edits support independently declared host ranking, without another command producer. */
function fixture() {
  const state = completionState('so');
  const tags = sessionResult(state, [{ name: 'soft custom', backend: 'tag' }]);
  const ranked = sessionResult(state, [
    { name: 'soft trigger', backend: 'upstream' },
    { name: 'soft custom', backend: 'tag' },
  ]);
  const combined = {
    ...ranked,
    options: ranked.options.map((option, index) =>
      index === 0 ? { ...option, kind: 'trigger' as const, backendValue: 'upstream' } : option,
    ),
  };
  return { state, tags, combined };
}

test('automatic selection follows newly higher-ranked trigger rows while explicit tag choice retains its identity', () => {
  for (const explicit of [false, true]) {
    const f = fixture();
    let state = f.state.update({ effects: installCompletion.of(f.tags) }).state;
    const visible = state.field(completionSession);
    if (explicit) {
      state = state.update({
        effects: selectCompletion.of({ result: f.tags, index: 0 }),
      }).state;
      assert.equal(state.field(completionSession), visible);
    }
    const combined = state.update({
      effects: installCompletion.of(f.combined),
    }).state;
    assert.equal(combined.field(completionSession)!.selected, explicit ? 1 : 0);
    assert.equal(
      combined.field(completionSession)!.result.options[
        combined.field(completionSession)!.selected
      ]!.kind,
      explicit ? 'tag' : 'trigger',
    );
    assert.equal(combined.selection, state.selection);
  }
});

test("invalid and stale selection effects preserve rank-first intent; independent state branches cannot choose each other's rows", () => {
  const f = fixture();
  const original = f.state.update({
    effects: installCompletion.of(f.tags),
  }).state;
  const visible = original.field(completionSession);
  const explicit = original.update({
    effects: selectCompletion.of({ result: f.tags, index: 0 }),
  }).state;
  const invalid = original.update({
    effects: [
      selectCompletion.of({ result: f.tags, index: -1 }),
      selectCompletion.of({ result: f.combined, index: 0 }),
      StateEffect.define<null>().of(null),
    ],
  }).state;
  assert.equal(explicit.field(completionSession), visible);
  assert.equal(invalid.field(completionSession), visible);
  assert.equal(
    invalid.update({ effects: installCompletion.of(f.combined) }).state.field(completionSession)!
      .selected,
    0,
  );
  assert.equal(
    explicit.update({ effects: installCompletion.of(f.combined) }).state.field(completionSession)!
      .selected,
    1,
  );
});

test('dismissal and a missing chosen row release explicit intent for the next ranking', () => {
  const f = fixture();
  let state = f.state.update({ effects: installCompletion.of(f.tags) }).state;
  state = state.update({
    effects: selectCompletion.of({ result: f.tags, index: 0 }),
  }).state;
  const onlyTrigger = { ...f.combined, options: [f.combined.options[0]!] };
  const reset = state.update({
    effects: installCompletion.of(onlyTrigger),
  }).state;
  assert.equal(reset.field(completionSession)!.selected, 0);
  const reordered = {
    ...f.combined,
    options: [f.combined.options[1]!, f.combined.options[0]!],
  };
  assert.equal(
    reset.update({ effects: installCompletion.of(reordered) }).state.field(completionSession)!
      .selected,
    0,
  );
  state = state.update({ effects: installCompletion.of(null) }).state;
  assert.equal(state.field(completionSession), null);
  assert.equal(
    state.update({ effects: installCompletion.of(f.combined) }).state.field(completionSession)!
      .selected,
    0,
  );
});
