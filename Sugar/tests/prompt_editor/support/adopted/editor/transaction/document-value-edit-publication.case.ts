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
/** Actual producer transactions conserve bounded purpose and authoring metadata before issuance. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, StateEffect, Transaction } from '@codemirror/state';
import { isolateHistory, undo, redo, undoDepth } from '@codemirror/commands';
import {
  authorship,
  authorshipExtension,
  installAuthorship,
} from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import {
  preparedAuthorship,
  generatedShells,
  literalIntents,
} from '../../../../../../frontend/sugarprompt/editor/src/authorship-ranges.ts';
import {
  prepareDocumentValueEdit,
  currentDocumentValueEdit,
} from '../../../../../../frontend/sugarprompt/editor/src/document-value-edit-issuance.ts';
import type { DocumentValueEditPublication } from '../../../../../../frontend/sugarprompt/editor/src/document-value-edit-publication.ts';
import {
  valueState,
  valueContext,
  valueTarget,
  decodedEdit,
  acceptedEdit,
  editedState,
} from './document-value-fixture.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

for (const [purpose, event, scroll] of [
  ['completion', 'input.complete', true],
  ['weight', 'input.weight', false],
  ['region', 'input.region', false],
  ['trigger', 'input.exact', true],
] as const) {
  test(`${purpose} publication is part of the actual issued encoded transaction`, () => {
    const { state } = valueState('h\r\n"a",keep');
    const context = valueContext(state),
      target = valueTarget(state, context, 3);
    const edit = acceptedEdit(state, {
      ...decodedEdit(context, target, 0, 1, 'q,"x'),
      publication: { purpose },
    });
    const transaction = edit.transactions[0]!;
    assert.equal(transaction.startState, state);
    assert.equal(transaction.annotation(Transaction.userEvent), event);
    assert.equal(transaction.annotation(isolateHistory), 'full');
    assert.equal(transaction.scrollIntoView, scroll);
    assert.equal(readExactSource(transaction.state) === 'h\r\n"q,""x",keep', true);
    assert.equal(currentDocumentValueEdit(state, context, edit), null);
    assert.equal(undoDepth(transaction.state), 1);
  });
}

test('the exact prepared authorship effect survives encoded acceptance and native undo/redo', () => {
  const original = valueState('a', 'prompt').state;
  const state = original.update({
    effects: StateEffect.appendConfig.of(authorshipExtension),
  }).state;
  const context = valueContext(state),
    target = valueTarget(state, context, 0);
  const metadata = preparedAuthorship(
    [{ from: 0, to: 8, contentFrom: 1, contentTo: 2, nestingDepth: 1 }],
    [],
  );
  const effect = installAuthorship.of(metadata);
  const edit = acceptedEdit(state, {
    ...decodedEdit(context, target, 0, 1, '(a:1.10)'),
    publication: { purpose: 'completion', authorship: effect },
  });
  const transaction = edit.transactions[0]!;
  assert.equal(transaction.effects.at(-1), effect);
  assert.equal(transaction.state.field(authorship), metadata);
  assert.equal(currentDocumentValueEdit(state, context, edit), null);
  assert.deepEqual(generatedShells(transaction.state.field(authorship)), generatedShells(metadata));
  let next = editedState(edit);
  assert.equal(
    undo({
      state: next,
      dispatch: (transaction) => {
        next = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(next) === 'a', true);
  assert.equal(next.field(authorship).generated.size, 0);
  assert.equal(
    redo({
      state: next,
      dispatch: (transaction) => {
        next = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(next) === '(a:1.10)', true);
  assert.deepEqual(generatedShells(next.field(authorship)), generatedShells(metadata));
});

test('foreign metadata effects refuse before a transaction is issued', () => {
  const state = valueState('a', 'prompt').state;
  const context = valueContext(state),
    target = valueTarget(state, context, 0);
  const foreign = StateEffect.define<ReturnType<typeof preparedAuthorship>>();
  const effect = foreign.of(preparedAuthorship([], []));
  assert.throws(
    () =>
      prepareDocumentValueEdit(state, {
        ...decodedEdit(context, target, 0, 1, 'b'),
        publication: { purpose: 'completion', authorship: effect },
      }),
    /decoded-edit-authorship-effect-mismatch/u,
  );
  assert.equal(readExactSource(state) === 'a', true);
  assert.equal(undoDepth(state), 0);
});

test('mutated publication intent cannot alter issued metadata and effect payload mutation permanently retires issuance', () => {
  const original = valueState('a', 'prompt').state;
  const state = original.update({
    effects: StateEffect.appendConfig.of(authorshipExtension),
  }).state;
  const context = valueContext(state),
    target = valueTarget(state, context, 0);
  const metadata = preparedAuthorship([], [{ from: 0, to: 1 }]);
  const effect = installAuthorship.of(metadata);
  const publication: DocumentValueEditPublication = {
    purpose: 'completion',
    authorship: effect,
  };
  const edit = acceptedEdit(state, {
    ...decodedEdit(context, target, 0, 1, 'b'),
    publication,
  });
  assert.equal(Reflect.set(publication, 'purpose', 'region'), true);
  assert.equal(edit.transactions[0]!.annotation(Transaction.userEvent), 'input.complete');
  assert.equal(currentDocumentValueEdit(state, context, edit), null);
  assert.equal(Reflect.set(effect, 'value', preparedAuthorship([], [])), true);
  assert.equal(currentDocumentValueEdit(state, context, edit), 'foreign');
  assert.equal(Reflect.set(effect, 'value', metadata), true);
  assert.equal(currentDocumentValueEdit(state, context, edit), 'foreign');
});

test('metadata-only no-op publication retains populated source output and remains outside source history', () => {
  const original = valueState('a', 'prompt').state;
  const state = original.update({
    effects: StateEffect.appendConfig.of(authorshipExtension),
  }).state;
  const context = valueContext(state),
    target = valueTarget(state, context, 0);
  const metadata = preparedAuthorship([], [{ from: 0, to: 1 }]);
  const effect = installAuthorship.of(metadata);
  const outcome = prepareDocumentValueEdit(state, {
    ...decodedEdit(context, target, 0, 1, 'a'),
    publication: { purpose: 'completion', authorship: effect },
  });
  assert.equal(outcome.status, 'unchanged');
  const transaction = outcome.edit.transactions[0]!;
  assert.equal(transaction.docChanged, false);
  assert.equal(transaction.state.field(authorship), metadata);
  assert.deepEqual(literalIntents(transaction.state.field(authorship)), [{ from: 0, to: 1 }]);
  assert.equal(outcome.edit.preparedValues, context.values.scopes.values);
  assert.equal(currentDocumentValueEdit(state, context, outcome.edit), null);
  assert.equal(undoDepth(transaction.state), 0);
});

test('metadata-only genuine no-op retains complete native reverse selection and effect identity with populated reuse and current followup', () => {
  const original = valueState('a', 'prompt').state;
  const selection = EditorSelection.create([EditorSelection.range(1, 0, undefined, 2, -1)]);
  const state = original.update({
    selection,
    effects: StateEffect.appendConfig.of(authorshipExtension),
  }).state;
  const context = valueContext(state),
    target = valueTarget(state, context, 0);
  const metadata = preparedAuthorship([], [{ from: 0, to: 1 }]),
    effect = installAuthorship.of(metadata);
  const edit = acceptedEdit(state, {
    ...decodedEdit(context, target, 0, 1, 'a', 1, 0),
    publication: { purpose: 'completion', authorship: effect },
  });
  const transaction = edit.transactions[0]!;
  assert.equal(transaction.selection, state.selection);
  assert.equal(transaction.state.selection, state.selection);
  assert.deepEqual(
    [
      transaction.state.selection.main.anchor,
      transaction.state.selection.main.head,
      transaction.state.selection.main.goalColumn,
      transaction.state.selection.main.bidiLevel,
      transaction.state.selection.main.assoc,
    ],
    [1, 0, undefined, 2, -1],
  );
  assert.equal(transaction.effects.at(-1), effect);
  assert.equal(transaction.state.field(authorship), metadata);
  assert.equal(transaction.docChanged, false);
  assert.equal(transaction.state.doc, state.doc);
  assert.equal(undoDepth(transaction.state), 0);
  assert.equal(edit.postValues, context.values);
  assert.equal(edit.semantics, context.semantics);
  assert.equal(edit.postValues.scopes.scopes[0]!.document.sourceText.length, 1);
  assert.equal(currentDocumentValueEdit(state, context, edit), null);
  assert.equal(Reflect.set(effect, 'value', preparedAuthorship([], [])), true);
  assert.equal(currentDocumentValueEdit(state, context, edit), 'foreign');
  assert.equal(Reflect.set(effect, 'value', metadata), true);
  assert.equal(currentDocumentValueEdit(state, context, edit), 'foreign');
  const followupState = transaction.state,
    followupContext = valueContext(followupState),
    followupTarget = valueTarget(followupState, followupContext, 0);
  const followup = acceptedEdit(followupState, {
    ...decodedEdit(followupContext, followupTarget, 0, 1, 'b', 1, 0),
    publication: { purpose: 'weight' },
  });
  assert.equal(currentDocumentValueEdit(followupState, followupContext, followup), null);
  assert.equal(readExactSource(editedState(followup)) === 'b', true);
  assert.equal(undoDepth(editedState(followup)), 1);
});
