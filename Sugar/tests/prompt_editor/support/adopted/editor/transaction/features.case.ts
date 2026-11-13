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
/** Actual CodeMirror reconfiguration keeps source, history and prepared semantic identity intact. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, EditorState } from '@codemirror/state';
import { history, undoDepth } from '@codemirror/commands';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  semanticStructures,
  installSemanticStructure,
  prepareInspectionStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  exactSourceExtension,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  defaultPreferences,
  resolveFeatureProfile,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';

test('profile reconfiguration observes a service result without invalidating exact-source inspection or history', () => {
  const configuration = new Compartment();
  const first = resolveFeatureProfile(defaultPreferences, { mode: 'library' });
  let state = EditorState.create({
    doc: '(cat:1.20)',
    extensions: [
      history(),
      exactSourceExtension('(cat:1.20)'),
      editorFreshness,
      semanticStructures,
      configuration.of(featureProfile.of(first)),
    ],
  });
  const prepared = prepareInspectionStructure(state, { style: 'curly' });
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'inspection',
      prepared: prepared,
    }),
    selection: { anchor: 2, head: 4 },
  }).state;
  const next = resolveFeatureProfile(defaultPreferences, { mode: 'workflow' });
  state = state.update({
    effects: configuration.reconfigure(featureProfile.of(next)),
  }).state;
  assert.equal(state.facet(featureProfile), next);
  assert.equal(state.field(semanticStructures).inspection, prepared);
  assert.equal(readExactSource(state), '(cat:1.20)');
  assert.deepEqual(
    [state.selection.main.anchor, state.selection.main.head, undoDepth(state)],
    [2, 4, 0],
  );
});
