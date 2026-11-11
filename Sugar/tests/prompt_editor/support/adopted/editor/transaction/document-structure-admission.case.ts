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
/** Genuine producer seeds alone acquire structure admission and supported publication authority. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection } from '@codemirror/state';
import { undo, redo, undoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  nativeDocumentValueAt,
  prepareNativeDocumentValues,
} from '../../../../../../frontend/sugarprompt/editor/src/document-values.ts';
import {
  prepareDocumentValueEdit,
  currentDocumentValueEdit,
} from '../../../../../../frontend/sugarprompt/editor/src/document-value-edit-issuance.ts';
import {
  prepareNativeStructure,
  prepareInspectionStructure,
  semanticStructures,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { structureState, publishStructure } from './document-structure-fixture.ts';
import {
  prepareDocumentPromptScopes,
  prepareDocumentSemantics,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { context, editAt } from './document-structure-fixture.ts';

for (const kind of ['prompt', 'wildcard-txt', 'wildcard-csv'] as const)
  test(`${kind} post-edit publication adopts genuine output without recreation or history/selection changes`, () => {
    let { state } = structureState(kind === 'wildcard-csv' ? 'h\r\n"{one}"' : '{one}', kind);
    const before = prepareNativeStructure(state);
    state = publishStructure(state, before);
    const position = kind === 'wildcard-csv' ? 3 : 0;
    const edit = editAt(state, before, position, '🙂, ');
    const transaction = edit.transactions[0]!;
    state = transaction.state;
    const prepared = prepareNativeStructure(state, edit);
    assert.equal(prepared.documentSemantics, edit.semantics);
    assert.equal(prepared.scopes, edit.postValues.scopes);
    assert.equal(prepared.scopes.values, edit.preparedValues);
    assert.equal(prepared.nativeValues, edit.postValues);
    assert.equal(prepared.scopes.scopes[0]!.document, edit.postValues.scopes.scopes[0]!.document);
    assert.equal(
      prepared.scopes.scopes[0]!.wildcardGroups,
      edit.postValues.scopes.scopes[0]!.wildcardGroups,
    );
    const selection = state.selection,
      depth = undoDepth(state);
    state = publishStructure(state, prepared);
    assert.equal(state.selection, selection);
    assert.equal(undoDepth(state), depth);
    assert.deepEqual(
      [state.selection.main.anchor, state.selection.main.head],
      kind === 'wildcard-csv' ? [7, 3] : [4, 0],
    );
    assert.throws(() => prepareNativeStructure(state, edit), /semantic-structure-seed-mismatch/u);
    assert.equal(
      undo({
        state,
        dispatch: (transaction) => {
          state = transaction.state;
        },
      }),
      true,
    );
    assert.equal(readExactSource(state) === before.scopes.values.source, true);
    assert.equal(state.field(semanticStructures).native, null);
    assert.equal(
      redo({
        state,
        dispatch: (transaction) => {
          state = transaction.state;
        },
      }),
      true,
    );
    assert.equal(readExactSource(state) === edit.preparedValues.source, true);
    assert.equal(state.field(semanticStructures).native, null);
  });

test('foreign resulting state and mismatched actual seed members reject without retiring current output', () => {
  let { state } = structureState('h\r\n"{one}"', 'wildcard-csv');
  const before = prepareNativeStructure(state);
  state = publishStructure(state, before);
  const edit = editAt(state, before, 3, 'a');
  const result = edit.transactions[0]!.state;
  assert.throws(() => prepareNativeStructure(state, edit), /semantic-structure-seed-mismatch/u);
  assert.throws(
    () => prepareNativeStructure(result.update({ selection: { anchor: 0 } }).state, edit),
    /semantic-structure-seed-mismatch/u,
  );
  assert.throws(
    () =>
      prepareNativeStructure(result, {
        ...edit,
        semantics: before.documentSemantics,
      }),
    /semantic-structure-seed-mismatch/u,
  );
  assert.throws(
    () => prepareInspectionStructure(result, { style: 'double-underscore' }, edit),
    /semantic-structure-seed-mismatch/u,
  );
  assert.equal(state.field(semanticStructures).native, before);
});

test('complete producer issuance refuses reconstructed, separately prepared and shifted seed maps on the genuine result state', () => {
  let { state } = structureState('h\r\n"{a}"', 'wildcard-csv');
  const before = prepareNativeStructure(state);
  state = publishStructure(state, before);
  const edit = editAt(state, before, 3, '');
  const result = edit.transactions[0]!.state;
  const scopes = prepareDocumentPromptScopes(edit.preparedValues, {
    style: 'curly',
  });
  const separatelyPrepared = prepareNativeDocumentValues(result, scopes, {
    style: 'curly',
  });
  const entry = edit.postValues.entries[1]!;
  const shiftedEntry = Object.freeze({
    ...entry,
    semanticMapping: Object.freeze({
      ...entry.semanticMapping!,
      decodedToLogical: Object.freeze(
        entry.semanticMapping!.decodedToLogical.map((position) =>
          position === null ? null : position + 1,
        ),
      ),
    }),
  });
  const shiftedValues = Object.freeze({
    ...edit.postValues,
    entries: Object.freeze([edit.postValues.entries[0]!, shiftedEntry]),
    byId: Object.freeze({
      ...edit.postValues.byId,
      [entry.container.id]: shiftedEntry,
    }),
  });
  for (const foreign of [
    { ...edit },
    { ...edit, postValues: { ...edit.postValues } },
    { ...edit, postValues: separatelyPrepared },
    { ...edit, postValues: shiftedValues },
    {
      ...edit,
      postValues: separatelyPrepared,
      semantics: prepareDocumentSemantics(
        edit.preparedValues.source,
        'wildcard-csv',
        undefined,
        edit.preparedValues,
      ),
    },
    { ...edit, transactions: [state.update({ selection: result.selection })] },
  ]) {
    assert.throws(
      () => prepareNativeStructure(result, foreign),
      /semantic-structure-seed-mismatch/u,
    );
    assert.equal(currentDocumentValueEdit(state, context(before), foreign), 'foreign');
    assert.equal(result.field(semanticStructures).native, before);
  }
  assert.equal(
    Reflect.set(edit.postValues.entries[1]!.semanticMapping!.decodedToLogical, '0', 4),
    false,
  );
  const genuine = prepareNativeStructure(result, edit);
  const bounds: number[][] = [];
  genuine.pointerTargets.between(0, result.doc.length, (from, to, target) => {
    if (target.kind === 'wildcard')
      bounds.push([from, to, from + target.contentFrom, from + target.contentTo]);
  });
  assert.deepEqual(bounds, [[3, 6, 4, 5]]);
  assert.equal(genuine.nativeValues, edit.postValues);
});

test('mutated actual transaction issuance retires permanently and quoted-region shifted replacements cannot acquire source authority', () => {
  let { state } = structureState('h\n"x\n[SEP|A]\ny"', 'wildcard-csv');
  const before = prepareNativeStructure(state);
  state = publishStructure(state, before);
  const edit = editAt(state, before, 3, 'q');
  const transaction = edit.transactions[0]!,
    result = transaction.state;
  const genuine = prepareNativeStructure(result, edit);
  assert.deepEqual(
    genuine.regions.map((region) => [region.from, region.to, region.nameFrom, region.nameTo]),
    [[6, 13, 11, 12]],
  );
  const entry = edit.postValues.entries[1]!;
  const shifted = Object.freeze({
    ...entry,
    semanticMapping: Object.freeze({
      ...entry.semanticMapping!,
      decodedToLogical: Object.freeze(
        entry.semanticMapping!.decodedToLogical.map((position) =>
          position === null ? null : position + 1,
        ),
      ),
    }),
  });
  const foreign = {
    ...edit,
    postValues: Object.freeze({
      ...edit.postValues,
      entries: Object.freeze([edit.postValues.entries[0]!, shifted]),
      byId: Object.freeze({
        ...edit.postValues.byId,
        [entry.container.id]: shifted,
      }),
    }),
  };
  assert.throws(() => prepareNativeStructure(result, foreign), /semantic-structure-seed-mismatch/u);
  assert.equal(result.field(semanticStructures).native, null);
  const effects = transaction.effects;
  assert.equal(Reflect.set(transaction, 'effects', [...effects]), true);
  assert.throws(() => prepareNativeStructure(result, edit), /semantic-structure-seed-mismatch/u);
  assert.equal(Reflect.set(transaction, 'effects', effects), true);
  assert.throws(() => prepareNativeStructure(result, edit), /semantic-structure-seed-mismatch/u);
  assert.equal(currentDocumentValueEdit(state, context(before), edit), 'foreign');
  const followup = editAt(state, before, 3, 'r');
  assert.equal(
    prepareNativeStructure(followup.transactions[0]!.state, followup).nativeValues,
    followup.postValues,
  );
});

for (const kind of ['prompt', 'wildcard-txt', 'wildcard-csv'] as const)
  test(`${kind} authentic inspection-profile edits retain actual seeded scope/query/group/map output`, () => {
    let { state } = structureState(
      kind === 'wildcard-csv' ? 'h\n"%%a%%, {b}"' : '%%a%%, {b}',
      kind,
    );
    const native = prepareNativeStructure(state);
    state = publishStructure(state, native);
    const syntax = {
      style: 'custom',
      prefix: '%%',
      suffix: '%%',
      alsoRecognizeCurly: false,
    } as const;
    const inspection = prepareInspectionStructure(state, syntax);
    state = publishStructure(state, inspection, 'inspection');
    for (const text of ['', 'q, ']) {
      const edit = editAt(state, inspection, kind === 'wildcard-csv' ? 3 : 0, text);
      const result = edit.transactions.at(-1)?.state ?? state;
      assert.throws(
        () => prepareNativeStructure(result, edit),
        /semantic-structure-seed-mismatch/u,
      );
      const prepared = prepareInspectionStructure(result, syntax, edit);
      assert.equal(prepared.nativeValues, edit.postValues);
      assert.equal(prepared.scopes, edit.postValues.scopes);
      assert.equal(prepared.documentSemantics, edit.semantics);
      assert.equal(prepared.scopes.scopes[0]!.wildcardGroups.length, 1);
      assert.equal(
        prepared.scopes.scopes[0]!.wildcardGroups,
        edit.postValues.scopes.scopes[0]!.wildcardGroups,
      );
      assert.equal(
        prepared.scopes.scopes[0]!.wildcardQueries,
        edit.postValues.scopes.scopes[0]!.wildcardQueries,
      );
      assert.equal(
        prepared.nativeValues.entries.at(-1)!.semanticMapping,
        edit.postValues.entries.at(-1)!.semanticMapping,
      );
      if (!text) assert.equal(prepared, inspection);
      const installed = publishStructure(result, prepared, 'inspection');
      assert.equal(installed.field(semanticStructures).inspection, prepared);
      assert.equal(installed.selection, result.selection);
      assert.equal(undoDepth(installed), undoDepth(result));
    }
    assert.equal(state.field(semanticStructures).native, native);
  });

test('nested source and directional selection mutation cannot install canonical edit seeds; restored edits remain retired', () => {
  for (const mutation of ['endings', 'selection'] as const) {
    let { state } = structureState('h\r\n"{a}, (b:2)"', 'wildcard-csv');
    const before = prepareNativeStructure(state);
    state = publishStructure(state, before);
    const edit = editAt(state, before, 3, 'q'),
      transaction = edit.transactions[0]!,
      result = transaction.state;
    assert.equal(prepareNativeStructure(result, edit).nativeValues, edit.postValues);
    let restore: () => void;
    if (mutation === 'endings') {
      const payload: unknown = transaction.effects[0]!.value;
      assert.ok(Array.isArray(payload));
      const original: unknown = payload[0];
      assert.equal(Reflect.set(payload, '0', '\n'), true);
      assert.equal(readExactSource(result) === edit.preparedValues.source, false);
      restore = () => void Reflect.set(payload, '0', original);
    } else {
      const selection = transaction.selection!,
        original = selection.ranges[0]!;
      assert.equal(Reflect.set(selection.ranges, '0', EditorSelection.cursor(0)), true);
      assert.deepEqual([result.selection.main.anchor, result.selection.main.head], [0, 0]);
      restore = () => void Reflect.set(selection.ranges, '0', original);
    }
    assert.throws(() => prepareNativeStructure(result, edit), /semantic-structure-seed-mismatch/u);
    assert.equal(result.field(semanticStructures).native, null);
    assert.equal(currentDocumentValueEdit(state, context(before), edit), 'foreign');
    restore();
    assert.throws(() => prepareNativeStructure(result, edit), /semantic-structure-seed-mismatch/u);
    const followup = editAt(state, before, 3, 'r'),
      next = followup.transactions[0]!.state,
      prepared = prepareNativeStructure(next, followup);
    assert.equal(prepared.nativeValues, followup.postValues);
    assert.equal(prepared.scopes, followup.postValues.scopes);
    assert.equal(prepared.scopes.scopes[0]!.document.wildcardSpans.length, 1);
    assert.equal(prepared.scopes.scopes[0]!.document.emphasisSpans.length, 1);
    const installed = publishStructure(next, prepared);
    assert.equal(installed.field(semanticStructures).native, prepared);
    assert.equal(installed.selection, next.selection);
    assert.equal(undoDepth(installed), 1);
  }
});

test('sparse newline and nonfinite/null goal mutation refuse real seed adoption and publication with populated valid followup', () => {
  for (const mutation of ['ending-hole', 'goal-NaN', 'goal-null'] as const) {
    let { state } = structureState('h\r\n"{a}, (b:2)"', 'wildcard-csv');
    const before = prepareNativeStructure(state);
    state = publishStructure(state, before);
    const edit = editAt(state, before, 3, 'q'),
      transaction = edit.transactions[0]!,
      result = transaction.state;
    const admitted = prepareNativeStructure(result, edit);
    assert.equal(admitted.nativeValues, edit.postValues);
    assert.equal(result.selection.main.goalColumn, undefined);
    let restore: () => void;
    if (mutation === 'ending-hole') {
      const payload: unknown = transaction.effects[0]!.value;
      assert.ok(Array.isArray(payload));
      const original: unknown = payload[0];
      assert.equal(Reflect.deleteProperty(payload, '0'), true);
      assert.equal(Object.hasOwn(payload, '0'), false);
      restore = () => void Reflect.set(payload, '0', original);
    } else {
      assert.equal(
        Reflect.set(result.selection.main, 'goalColumn', mutation === 'goal-NaN' ? NaN : null),
        true,
      );
      restore = () => void Reflect.set(result.selection.main, 'goalColumn', undefined);
    }
    assert.throws(
      () => publishStructure(result, prepareNativeStructure(result, edit)),
      /semantic-structure-seed-mismatch/u,
    );
    assert.equal(result.field(semanticStructures).native, null);
    assert.equal(currentDocumentValueEdit(state, context(before), edit), 'foreign');
    restore();
    assert.throws(() => prepareNativeStructure(result, edit), /semantic-structure-seed-mismatch/u);
    assert.equal(currentDocumentValueEdit(state, context(before), edit), 'foreign');
    const followup = editAt(state, before, 3, 'r'),
      next = followup.transactions[0]!.state,
      prepared = prepareNativeStructure(next, followup),
      installed = publishStructure(next, prepared);
    assert.equal(installed.field(semanticStructures).native, prepared);
    assert.equal(prepared.nativeValues, followup.postValues);
    assert.equal(prepared.scopes.scopes[0]!.document.wildcardSpans.length, 1);
    assert.equal(prepared.scopes.scopes[0]!.document.emphasisSpans.length, 1);
    assert.equal(installed.selection.main.goalColumn, undefined);
    assert.equal(installed.selection, next.selection);
    assert.equal(undoDepth(installed), 1);
    const ready = nativeDocumentValueAt(installed, context(prepared), {
      from: 3,
      to: 3,
    });
    assert.equal(ready.status, 'ready');
    if (ready.status !== 'ready') throw new Error('fixture-no-op-target');
    const unchanged = prepareDocumentValueEdit(installed, {
      context: context(prepared),
      target: ready.target,
      range: { from: 0, to: 0 },
      text: '',
      selection: {
        ranges: [{ kind: 'decoded', anchor: 1, head: 0 }],
        mainIndex: 0,
      },
    });
    assert.equal(unchanged.status, 'unchanged');
    const noOp = unchanged.edit;
    assert.equal(noOp.transactions.length, 0);
    assert.equal(currentDocumentValueEdit(installed, context(prepared), noOp), null);
    assert.equal(prepareNativeStructure(installed, noOp), prepared);
    assert.equal(publishStructure(installed, prepared).field(semanticStructures).native, prepared);
  }
});
