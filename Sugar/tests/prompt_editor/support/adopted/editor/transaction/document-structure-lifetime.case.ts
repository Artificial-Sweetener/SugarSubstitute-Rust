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
/** Native slot and profile lifetime proof conserves current output and retires old authority. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { exactReplacement } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  nativeDocumentValueAt,
  currentNativeDocumentValue,
  prepareNativeDocumentValues,
} from '../../../../../../frontend/sugarprompt/editor/src/document-values.ts';
import { prepareDocumentValueEdit } from '../../../../../../frontend/sugarprompt/editor/src/document-value-edit-issuance.ts';
import {
  prepareNativeStructure,
  prepareInspectionStructure,
  semanticStructures,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import { structureState, publishStructure } from './document-structure-fixture.ts';
import { prepareDocumentPromptScopes } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { context, editAt } from './document-structure-fixture.ts';

test('kind return and source restoration retire slots and reject old or reconstructed publication', () => {
  const fixture = structureState('{one}');
  let state = fixture.state;
  const old = prepareNativeStructure(state);
  state = publishStructure(state, old);
  state = state.update({ effects: fixture.kindSlot.reconfigure([]) }).state;
  assert.equal(state.field(semanticStructures).native, old);
  state = state.update({
    effects: fixture.kindSlot.reconfigure(documentKind.of('wildcard-txt')),
  }).state;
  state = state.update({
    effects: fixture.kindSlot.reconfigure(documentKind.of('prompt')),
  }).state;
  state = publishStructure(state, old);
  assert.equal(state.field(semanticStructures).native, null);
  const current = prepareNativeStructure(state);
  state = publishStructure(state, { ...current });
  assert.equal(state.field(semanticStructures).native, null);
  state = publishStructure(state, current);
  state = state.update(exactReplacement(state, 0, 5, '{two}')).state;
  state = state.update(exactReplacement(state, 0, 5, '{one}')).state;
  state = publishStructure(state, current);
  assert.equal(state.field(semanticStructures).native, null);
});

test('no-op edit and context retirement retain populated maps without renewing issued target authority', () => {
  const fixture = structureState('{one}');
  let state = fixture.state;
  const before = prepareNativeStructure(state);
  state = publishStructure(state, before);
  const current = context(before);
  const target = nativeDocumentValueAt(state, current, { from: 0, to: 0 });
  if (target.status !== 'ready') throw new Error('fixture-target-unavailable');
  const outcome = prepareDocumentValueEdit(state, {
    context: current,
    target: target.target,
    range: { from: 0, to: 0 },
    text: '',
    selection: {
      ranges: [{ kind: 'decoded', anchor: 0, head: 0 }],
      mainIndex: 0,
    },
  });
  if (outcome.status === 'rejected') throw new Error('fixture-noop-rejected');
  assert.equal(outcome.status, 'unchanged');
  assert.equal(outcome.edit.transactions.length, 0);
  assert.equal(prepareNativeStructure(state, outcome.edit), before);
  assert.equal(outcome.edit.postValues, before.nativeValues);
  assert.equal(outcome.edit.semantics, before.documentSemantics);
  state = state.update({
    effects: fixture.permission.reconfigure(EditorState.readOnly.of(true)),
  }).state;
  state = state.update({
    effects: fixture.permission.reconfigure(EditorState.readOnly.of(false)),
  }).state;
  state = state.update({ effects: invalidateCommandContext.of(null) }).state;
  assert.equal(prepareNativeStructure(state), before);
  const retired = currentNativeDocumentValue(state, current, target.target);
  assert.deepEqual(retired, { status: 'rejected', reason: 'stale-context' });
});

test('retired inspection profiles cannot republish or resurrect old populated output', () => {
  let { state } = structureState('%%one%%, {two}');
  const native = prepareNativeStructure(state);
  state = publishStructure(state, native);
  const customProfile = {
    style: 'custom',
    prefix: '%%',
    suffix: '%%',
    alsoRecognizeCurly: false,
  } as const;
  const custom = prepareInspectionStructure(state, customProfile);
  state = publishStructure(state, custom, 'inspection');
  const alternate = prepareInspectionStructure(state, {
    style: 'double-underscore',
  });
  state = publishStructure(state, alternate, 'inspection');
  state = publishStructure(state, custom, 'inspection');
  assert.equal(state.field(semanticStructures).inspection, alternate);
  const returned = prepareInspectionStructure(state, customProfile);
  assert.notEqual(returned, custom);
  assert.notEqual(returned.scopes, custom.scopes);
  state = publishStructure(state, returned, 'inspection');
  assert.equal(state.field(semanticStructures).inspection, returned);
  assert.equal(prepareNativeStructure(state), native);
  assert.equal(prepareInspectionStructure(state, { style: 'curly' }), native);
});

test('selection-only no-op adapter seeds preserve every populated semantic and native mapping object', () => {
  let { state } = structureState('h\r\n" {one} "', 'wildcard-csv');
  const before = prepareNativeStructure(state);
  state = publishStructure(state, before);
  const current = context(before);
  const target = nativeDocumentValueAt(state, current, { from: 4, to: 4 });
  if (target.status !== 'ready') throw new Error('fixture-noop-target-unavailable');
  const result = prepareDocumentValueEdit(state, {
    context: current,
    target: target.target,
    range: { from: 0, to: 0 },
    text: '',
    selection: {
      ranges: [{ kind: 'decoded', anchor: 5, head: 0 }],
      mainIndex: 0,
    },
  });
  if (result.status === 'rejected') throw new Error('fixture-noop-unavailable');
  assert.equal(result.status, 'unchanged');
  const transaction = result.edit.transactions[0]!;
  state = transaction.state;
  assert.equal(prepareNativeStructure(state, result.edit), before);
  assert.equal(result.edit.postValues, before.nativeValues);
  assert.equal(result.edit.postValues.scopes, before.scopes);
  assert.equal(result.edit.semantics, before.documentSemantics);
  const previous = before.nativeValues.entries[1]!,
    next = result.edit.postValues.entries[1]!;
  assert.equal(next.fullMapping, previous.fullMapping);
  assert.equal(next.semanticMapping, previous.semanticMapping);
  assert.equal(next.semanticMapping!.decodedToLogical, previous.semanticMapping!.decodedToLogical);
  assert.equal(next.semanticMapping!.logicalToDecoded, previous.semanticMapping!.logicalToDecoded);
  assert.equal(next.scope!.document, previous.scope!.document);
  assert.equal(next.scope!.wildcardGroups, previous.scope!.wildcardGroups);
  assert.equal(next.scope!.tagQueries, previous.scope!.tagQueries);
  assert.equal(next.scope!.loraQueries, previous.scope!.loraQueries);
  assert.equal(next.scope!.wildcardQueries, previous.scope!.wildcardQueries);
  assert.deepEqual([state.selection.main.anchor, state.selection.main.head], [9, 4]);
  assert.equal(undoDepth(state), 0);
});

test('genuine foreign current preparation and retired profile/no-op epochs cannot supply installed slot authority', () => {
  const fixture = structureState('h\r\n"{a}"', 'wildcard-csv');
  let state = fixture.state;
  const before = prepareNativeStructure(state);
  state = publishStructure(state, before);
  const foreignScopes = prepareDocumentPromptScopes(before.scopes.values, {
    style: 'curly',
  });
  const foreignValues = prepareNativeDocumentValues(state, foreignScopes, {
    style: 'curly',
  });
  const foreignContext = { ...context(before), values: foreignValues };
  const target = nativeDocumentValueAt(state, foreignContext, {
    from: 3,
    to: 3,
  });
  if (target.status !== 'ready') throw new Error('fixture-foreign-target');
  const foreign = prepareDocumentValueEdit(state, {
    context: foreignContext,
    target: target.target,
    range: { from: 0, to: 0 },
    text: 'x',
    selection: {
      ranges: [{ kind: 'decoded', anchor: 1, head: 0 }],
      mainIndex: 0,
    },
  });
  if (foreign.status === 'rejected') throw new Error('fixture-foreign-edit');
  assert.throws(
    () => prepareNativeStructure(foreign.edit.transactions[0]!.state, foreign.edit),
    /semantic-structure-seed-mismatch/u,
  );
  const clonedContext = {
    ...context(before),
    semantics: { ...before.documentSemantics },
  };
  const clonedTarget = nativeDocumentValueAt(state, clonedContext, {
    from: 3,
    to: 3,
  });
  if (clonedTarget.status !== 'ready') throw new Error('fixture-cloned-context-target');
  const cloned = prepareDocumentValueEdit(state, {
    context: clonedContext,
    target: clonedTarget.target,
    range: { from: 0, to: 0 },
    text: '',
    selection: {
      ranges: [{ kind: 'decoded', anchor: 3, head: 0 }],
      mainIndex: 0,
    },
  });
  if (cloned.status === 'rejected') throw new Error('fixture-cloned-context-edit');
  assert.throws(
    () => prepareNativeStructure(cloned.edit.transactions[0]!.state, cloned.edit),
    /semantic-structure-seed-mismatch/u,
  );
  const genuine = editAt(state, before, 3, '');
  const selected = genuine.transactions[0]!.state;
  assert.equal(prepareNativeStructure(selected, genuine), before);
  for (const changed of [
    selected.update({ effects: invalidateCommandContext.of(null) }).state,
    selected.update(exactReplacement(selected, 1, 3, '\n')).state,
    selected.update({
      effects: fixture.kindSlot.reconfigure(documentKind.of('wildcard-txt')),
    }).state,
    selected.update({
      effects: fixture.permission.reconfigure(EditorState.readOnly.of(true)),
    }).state,
  ])
    assert.throws(
      () => prepareNativeStructure(changed, genuine),
      /semantic-structure-seed-mismatch/u,
    );
  assert.equal(state.field(semanticStructures).native, before);
});
