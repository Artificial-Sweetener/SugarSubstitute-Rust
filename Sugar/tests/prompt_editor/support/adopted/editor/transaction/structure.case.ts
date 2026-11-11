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
/** Real state-field proof for bounded semantic memoization and exact-source invalidation. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, Text } from '@codemirror/state';
import { history, undo } from '@codemirror/commands';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  exactReplacement,
  exactSourceExtension,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  semanticStructures,
  installSemanticStructure,
  prepareInspectionStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { promptStructure } from './document-structure-fixture.ts';

/** Build an actual CodeMirror state with exact newline metadata and lazy semantics. */
function stateFor(source: string): EditorState {
  return EditorState.create({
    doc: Text.of(encodeSource(source).logical.split('\n')),
    extensions: [exactSourceExtension(source), editorFreshness, history(), semanticStructures],
  });
}

test('typing and selection invalidate or reuse derived state without discovering syntax', () => {
  let state = stateFor('(cat:1.20), {animals}');
  assert.equal(state.field(semanticStructures).inspection, null);
  state = state.update({ changes: { from: 0, insert: '🙂, ' } }).state;
  assert.equal(state.field(semanticStructures).inspection, null);
  const prepared = promptStructure(prepareInspectionStructure(state, { style: 'curly' }));
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'inspection',
      prepared: prepared,
    }),
  }).state;
  assert.equal(prepared.structure.document.emphasisSpans.length, 1);
  assert.equal(Object.isFrozen(prepared.structure.document.emphasisSpans[0]), true);
  for (const anchor of [0, 3, 8, state.doc.length]) {
    state = state.update({ selection: { anchor } }).state;
    assert.equal(state.field(semanticStructures).inspection, prepared);
    assert.equal(prepareInspectionStructure(state, { style: 'curly' }), prepared);
  }
  state = state.update(exactReplacement(state, 5, 15, 'plain')).state;
  assert.equal(state.field(semanticStructures).inspection, null);
  const changed = promptStructure(prepareInspectionStructure(state, { style: 'curly' }));
  assert.notEqual(changed.structure, prepared.structure);
  assert.equal(changed.structure.document.emphasisSpans.length, 0);
});

test('metadata-only newline changes, history and baseline recreation cannot retain old ranges', () => {
  let state = stateFor('a\n(cat:1.20)');
  const first = prepareInspectionStructure(state, { style: 'curly' });
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'inspection',
      prepared: first,
    }),
  }).state;
  state = state.update(exactReplacement(state, 1, 2, '\r\n')).state;
  assert.equal(state.field(semanticStructures).inspection, null);
  const changed = promptStructure(prepareInspectionStructure(state, { style: 'curly' }));
  assert.equal(changed.structure.document.emphasisSpans[0]?.outerRange.from, 3);
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'inspection',
      prepared: changed,
    }),
  }).state;
  assert.equal(
    undo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    }),
    true,
  );
  assert.equal(state.field(semanticStructures).inspection, null);
  assert.equal(
    promptStructure(prepareInspectionStructure(state, { style: 'curly' })).structure.document
      .emphasisSpans[0]?.outerRange.from,
    2,
  );
  assert.equal(stateFor('baseline').field(semanticStructures).inspection, null);
});

test('the inspection cache slot includes all wrapper inputs and rejects invalid configuration', () => {
  let state = stateFor('%%animal%%, {animal}');
  const curly = prepareInspectionStructure(state, { style: 'curly' });
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'inspection',
      prepared: curly,
    }),
  }).state;
  const custom = {
    style: 'custom',
    prefix: '%%',
    suffix: '%%',
    alsoRecognizeCurly: false,
  } as const;
  const exclusive = promptStructure(prepareInspectionStructure(state, custom));
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'inspection',
      prepared: exclusive,
    }),
  }).state;
  assert.equal(exclusive.structure.document.wildcardSpans[0]?.outerRange.from, 0);
  assert.equal(state.field(semanticStructures).inspection, exclusive);
  assert.notEqual(state.field(semanticStructures).inspection, curly);
  assert.equal(prepareInspectionStructure(state, { ...custom }), exclusive);
  const inclusive = promptStructure(
    prepareInspectionStructure(state, {
      ...custom,
      alsoRecognizeCurly: true,
    }),
  );
  assert.equal(inclusive.structure.document.wildcardSpans.length, 2);
  assert.notEqual(inclusive, exclusive);
  assert.throws(
    () =>
      prepareInspectionStructure(state, {
        ...custom,
        prefix: '(',
        suffix: ')',
      }),
    RangeError,
  );
  assert.equal(state.field(semanticStructures).inspection, exclusive);
});

test('a late semantic installation cannot republish ranges from an edited or restored source', () => {
  let state = stateFor('(cat:1.20)');
  const stale = prepareInspectionStructure(state, { style: 'curly' });
  state = state.update(exactReplacement(state, 1, 4, 'dog')).state;
  const fresh = prepareInspectionStructure(state, { style: 'curly' });
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'inspection',
      prepared: fresh,
    }),
  }).state;
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'inspection',
      prepared: stale,
    }),
  }).state;
  assert.equal(state.field(semanticStructures).inspection, fresh);
  state = stateFor('(cat:1.20)');
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'inspection',
      prepared: stale,
    }),
  }).state;
  assert.equal(state.field(semanticStructures).inspection, null);
});

test('native grammar and the latest read-only inspection share matching results and retain independent bounded slots', () => {
  let state = stateFor('%%alpha,beta%%, tail');
  const native = promptStructure(prepareNativeStructure(state));
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'native',
      prepared: native,
    }),
  }).state;
  assert.equal(prepareInspectionStructure(state, { style: 'curly' }), native);
  const custom = promptStructure(
    prepareInspectionStructure(state, {
      style: 'custom',
      prefix: '%%',
      suffix: '%%',
      alsoRecognizeCurly: false,
    }),
  );
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'inspection',
      prepared: custom,
    }),
  }).state;
  assert.equal(custom.structure.document.segments.length, 2);
  assert.equal(custom.sceneContexts, native.sceneContexts);
  assert.equal(custom.sceneQueries, native.sceneQueries);
  assert.equal(custom.structure.scenes, native.structure.scenes);
  assert.equal(native.structure.document.segments.length, 3);
  assert.equal(prepareNativeStructure(state), native);
  assert.equal(state.field(semanticStructures).inspection, custom);
  assert.throws(
    () =>
      state
        .update({
          effects: installSemanticStructure.of({
            purpose: 'native',
            prepared: custom,
          }),
        })
        .state.field(semanticStructures),
    /Native semantic grammar mismatch/u,
  );
  const replacement = promptStructure(
    prepareInspectionStructure(state, {
      style: 'double-underscore',
    }),
  );
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'inspection',
      prepared: replacement,
    }),
  }).state;
  assert.equal(state.field(semanticStructures).native, native);
  assert.equal(state.field(semanticStructures).inspection, replacement);
  assert.equal(replacement.sceneQueries, native.sceneQueries);
  const previousSlots = state.field(semanticStructures);
  state = state.update(exactReplacement(state, 0, 0, '🙂\r\n')).state;
  const retiredSlots = state.field(semanticStructures);
  assert.equal(retiredSlots.native, null);
  assert.equal(retiredSlots.inspection, null);
  assert.notEqual(retiredSlots.identity, previousSlots.identity);
  assert.notEqual(retiredSlots.inspectionIdentity, previousSlots.inspectionIdentity);
  for (const purpose of ['native', 'inspection'] as const)
    state = state.update({
      effects: installSemanticStructure.of({ purpose, prepared: native }),
    }).state;
  assert.equal(state.field(semanticStructures), retiredSlots);
});
