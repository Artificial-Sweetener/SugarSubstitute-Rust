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
/** Real native state proves copied host keys and source-bound publication independently of browser title rendering. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undo } from '@codemirror/commands';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { exactReplacement } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  installSceneErrors,
  prepareSceneErrors,
  sceneErrors,
} from '../../../../../../frontend/sugarprompt/editor/src/scene-errors.ts';
import { projectionState } from './projection-state.ts';

test('host scene errors capture canonical keys without retaining a mutable caller container', () => {
  const state = projectionState('**First\nbody');
  const keys = ['first'],
    prepared = prepareSceneErrors(state.field(editorFreshness).source, keys);
  keys[0] = 'other';
  const next = state.update({ effects: installSceneErrors.of(prepared) }).state;
  assert.equal(next.field(sceneErrors).keys.has('first'), true);
  assert.equal(next.field(sceneErrors).keys.has('other'), false);
  for (const invalid of [['First'], [''], ['two  words'], [null], Array(1), 'first'])
    assert.throws(
      () => prepareSceneErrors(state.field(editorFreshness).source, invalid as readonly string[]),
      TypeError,
    );
});

test('typing, native undo and baseline recreation cannot adopt old host error publications', () => {
  let state = projectionState('**First\nbody');
  const prepared = prepareSceneErrors(state.field(editorFreshness).source, ['first']);
  state = state.update({ effects: installSceneErrors.of(prepared) }).state;
  state = state.update({ selection: { anchor: 4 } }).state;
  assert.equal(state.field(sceneErrors), prepared);
  state = state.update(exactReplacement(state, 4, 4, 'X')).state;
  state = state.update({ effects: installSceneErrors.of(prepared) }).state;
  assert.equal(state.field(sceneErrors).keys.size, 0);
  assert.equal(
    undo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    }),
    true,
  );
  state = state.update({ effects: installSceneErrors.of(prepared) }).state;
  assert.equal(state.field(sceneErrors).keys.size, 0);
  const baseline = projectionState('**First\nbody');
  assert.equal(
    baseline.update({ effects: installSceneErrors.of(prepared) }).state.field(sceneErrors).keys
      .size,
    0,
  );
});
