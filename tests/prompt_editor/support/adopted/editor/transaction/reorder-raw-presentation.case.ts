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
/** Raw reorder permission and issuance use real prepared structure independently of presentation. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, EditorState } from '@codemirror/state';
import { undo, redo, undoDepth } from '@codemirror/commands';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  defaultPreferences,
  resolveFeatureProfile,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import { structureState } from './document-structure-fixture.ts';
import { captureReorder, transactionState } from './reorder-fixture.ts';
import { authorshipExtension } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import {
  initializeNativeStructure,
  semanticStructures,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  prepareNativeReorder,
  prepareReorderCommit,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-command.ts';
import { currentDocumentValueEdit } from '../../../../../../frontend/sugarprompt/editor/src/document-value-edit-issuance.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** Raw presentation uses prepared structure and service permission; plain CM history proves source transactions, while native prompt history owns redo selection. */
function rawReorderState(enabled = true) {
  const profile = resolveFeatureProfile(
    {
      ...defaultPreferences,
      allowed: { ...defaultPreferences.allowed, segment_reorder: enabled },
    },
    { mode: 'library' },
  );
  const fixture = structureState('a, b, c', 'prompt', [
    authorshipExtension,
    richRendering.of(false),
    featureProfile.of(profile),
  ]);
  const state = initializeNativeStructure(
    fixture.state.update({ selection: EditorSelection.range(1, 0) }).state,
  );
  return { ...fixture, state };
}

test('raw presentation issues canonical reorder with reverse selection and one undo redo', () => {
  const { state } = rawReorderState();
  assert.equal(state.facet(richRendering), false);
  const capture = captureReorder(state);
  const order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
    kind: 'row',
    row: 0,
    insertion: 2,
  })!;
  const result = prepareReorderCommit(state, capture, order);
  assert.equal(result.status, 'ready');
  assert.equal(result.edit.transactions.length, 1);
  assert.equal(currentDocumentValueEdit(state, capture.context, result.edit), null);
  const native = transactionState(state);
  native.dispatch(result.edit.transactions[0]!);
  assert.equal(readExactSource(native.state), 'b, c, a');
  assert.deepEqual([native.state.selection.main.anchor, native.state.selection.main.head], [7, 6]);
  assert.equal(undoDepth(native.state), 1);
  assert.equal(undo(native), true);
  assert.equal(readExactSource(native.state), 'a, b, c');
  assert.deepEqual([native.state.selection.main.anchor, native.state.selection.main.head], [1, 0]);
  assert.equal(redo(native), true);
  assert.equal(readExactSource(native.state), 'b, c, a');
});

test('raw presentation retains disabled read only composition and stale admission', () => {
  const fixture = rawReorderState();
  const capture = captureReorder(fixture.state);
  const disabled = rawReorderState(false).state;
  const readOnly = fixture.state.update({
    effects: fixture.permission.reconfigure(EditorState.readOnly.of(true)),
  }).state;
  for (const state of [disabled, readOnly]) {
    const semantic = state.field(semanticStructures).native!;
    assert.equal(
      prepareNativeReorder(
        state,
        {
          values: semantic.nativeValues,
          semantics: semantic.documentSemantics,
          enabled: true,
          composing: false,
        },
        { from: 0, to: 0 },
      ),
      null,
    );
  }
  assert.equal(
    prepareNativeReorder(
      fixture.state,
      { ...capture.context, composing: true },
      { from: 0, to: 0 },
    ),
    null,
  );
  assert.equal(
    prepareNativeReorder(fixture.state, { ...capture.context, enabled: false }, { from: 0, to: 0 }),
    null,
  );
  const order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
    kind: 'row',
    row: 0,
    insertion: 2,
  })!;
  assert.equal(prepareReorderCommit(readOnly, capture, order).status, 'rejected');
  const changed = fixture.state.update({
    changes: { from: 0, to: 1, insert: 'z' },
  }).state;
  assert.equal(prepareReorderCommit(changed, capture, order).status, 'rejected');
});
