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
/** The public state field owns intrinsic exposure configuration across updates and compartment lifetimes. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, EditorState, type Extension } from '@codemirror/state';
import { exactSourceExtension } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { loraPresentation } from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import { loraMedia } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import { sceneErrors } from '../../../../../../frontend/sugarprompt/editor/src/scene-errors.ts';
import {
  richProjection,
  richProjectionExtension,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import {
  heldProjection,
  retireRichProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/projection-exposure.ts';

/** Supply the established source/resource prerequisites, without installing the new exposure field. */
function configure(projection: Extension): EditorState {
  return EditorState.create({
    doc: 'plain',
    extensions: [
      exactSourceExtension('plain'),
      editorFreshness,
      loraPresentation,
      loraMedia,
      sceneErrors,
      projection,
    ],
  });
}

for (const [name, projection] of [
  ['bare state field', richProjection],
  ['mounted rendering extension', richProjectionExtension],
  ['repeated supported configuration', [richProjection, richProjectionExtension]],
] as const) {
  test(`${name} includes the intrinsic held-token field through every update`, () => {
    let state = configure(projection);
    assert.equal(state.field(heldProjection), null);
    state = state.update({}).state;
    assert.equal(state.field(heldProjection), null);
    state = state.update({ selection: { anchor: 1 } }).state;
    assert.equal(state.field(heldProjection), null);
    state = state.update({ changes: { from: 1, insert: 'x' } }).state;
    assert.equal(state.field(heldProjection), null);
    state = state.update({ effects: retireRichProjection.of(null) }).state;
    assert.equal(state.field(heldProjection), null);
    assert.equal(state.field(richProjection).revealed, null);
    assert.equal(state.doc.length, 6);
  });
}

test('removing the projection configuration removes its exposure owner and reinstallation starts fresh', () => {
  const compartment = new Compartment();
  let state = configure(compartment.of(richProjection));
  assert.equal(state.field(heldProjection), null);
  state = state.update({ effects: compartment.reconfigure([]) }).state;
  assert.equal(state.field(richProjection, false), undefined);
  assert.equal(state.field(heldProjection, false), undefined);
  state = state.update({
    effects: compartment.reconfigure(richProjection),
  }).state;
  assert.equal(state.field(heldProjection), null);
  assert.equal(state.doc.length, 5);
  state = state.update({ selection: { anchor: 2 } }).state;
  assert.equal(state.field(heldProjection), null);
});
