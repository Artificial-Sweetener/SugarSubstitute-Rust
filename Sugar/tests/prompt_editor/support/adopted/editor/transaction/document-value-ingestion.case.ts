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
/** Single-use decoded normalization provenance preserves actual CSV edits and native input history. */
import { promptHistory } from '../../../../../../frontend/sugarprompt/editor/src/prompt-history.ts';
import {
  prepareDocumentValuePaste,
  prepareDocumentValueTypedNormalization,
} from '../../../../../../frontend/sugarprompt/editor/src/document-value-ingestion.ts';
import { normalizationJoin } from '../../../../../../frontend/sugarprompt/editor/src/input-descriptor.ts';
import { authoredImplicitDepth } from '../../../../../../frontend/sugarprompt/editor/src/normalization-events.ts';
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect, Transaction } from '@codemirror/state';
import { isolateHistory, undo, undoDepth } from '@codemirror/commands';
import {
  authorship,
  authorshipExtension,
} from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import { literalIntents } from '../../../../../../frontend/sugarprompt/editor/src/authorship-ranges.ts';
import { prepareDocumentValueEdit } from '../../../../../../frontend/sugarprompt/editor/src/document-value-edit-issuance.ts';
import { valueState, valueContext, valueTarget, decodedEdit } from './document-value-fixture.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

test('normalization preparation is single-use actual-target evidence and cannot bless copied or foreign commands', () => {
  const baseline = valueState('header\ncat,dog').state.update({
    effects: StateEffect.appendConfig.of(authorshipExtension),
  }).state;
  const context = valueContext(baseline),
    first = valueTarget(baseline, context, 8),
    second = valueTarget(baseline, context, 12);
  const provenance = prepareDocumentValuePaste(first, { from: 0, to: 3 }, '(cat)');
  const intent = {
    ...decodedEdit(context, first, 0, 3, '(cat:1.10)'),
    publication: { purpose: 'paste' as const, normalization: provenance },
  };
  assert.throws(
    () =>
      prepareDocumentValueEdit(baseline, {
        ...intent,
        publication: { purpose: 'paste', normalization: { ...provenance } },
      }),
    /decoded-normalization-foreign/u,
  );
  const result = prepareDocumentValueEdit(baseline, intent);
  assert.equal(result.status, 'ready');
  assert.equal(readExactSource(result.edit.transactions[0]!.state), 'header\n(cat:1.10),dog');
  assert.throws(() => prepareDocumentValueEdit(baseline, intent), /decoded-normalization-foreign/u);
  const foreign = prepareDocumentValuePaste(first, { from: 0, to: 3 }, '(cat)');
  assert.throws(
    () =>
      prepareDocumentValueEdit(baseline, {
        ...decodedEdit(context, second, 0, 3, '(cat:1.10)'),
        publication: { purpose: 'paste', normalization: foreign },
      }),
    /decoded-normalization-foreign/u,
  );
  const mismatch = prepareDocumentValuePaste(first, { from: 0, to: 3 }, '(cat)');
  assert.throws(
    () =>
      prepareDocumentValueEdit(baseline, {
        ...decodedEdit(context, first, 0, 3, '(dog:1.10)'),
        publication: { purpose: 'paste', normalization: mismatch },
      }),
    /decoded-normalization-post-value/u,
  );
  assert.deepEqual(prepareDocumentValueEdit(baseline, { ...intent, target: { ...first } }), {
    status: 'rejected',
    reason: 'foreign',
  });
});

test('decoded CSV typed normalization joins the genuine input history and retains original time and native metadata', () => {
  const source = 'header\n"(cat",tail';
  const initial = valueState(source).state.update({
    effects: StateEffect.appendConfig.of([authorshipExtension, promptHistory]),
  }).state;
  const input = initial.update({
    changes: { from: 12, insert: ')' },
    selection: { anchor: 13 },
    userEvent: 'input.type',
    annotations: Transaction.time.of(100),
  });
  const state = input.state,
    context = valueContext(state),
    target = valueTarget(state, context, 13);
  const provenance = prepareDocumentValueTypedNormalization(
    target,
    { from: 4, to: 5 },
    state.field(authorship),
    { deletion: false, manual: false, protected: false },
  );
  const outcome = prepareDocumentValueEdit(state, {
    ...decodedEdit(context, target, 0, 5, '(cat:1.10)'),
    publication: {
      purpose: 'typed',
      normalization: provenance,
      input: {
        time: 100,
        userEvent: 'input.type',
        authoredImplicitDepth: null,
      },
    },
  });
  if (outcome.status === 'rejected') throw new Error('Expected typed command');
  const transaction = outcome.edit.transactions[0]!,
    after = transaction.state;
  assert.equal(readExactSource(after), 'header\n"(cat:1.10)",tail');
  assert.equal(transaction.annotation(normalizationJoin), true);
  assert.equal(transaction.annotation(Transaction.time), 100);
  assert.equal(transaction.annotation(isolateHistory), undefined);
  assert.equal(transaction.annotation(authoredImplicitDepth), undefined);
  assert.equal(after.field(authorship).generated.size, 1);
  assert.equal(undoDepth(after), 1);
  let restored = after;
  assert.equal(
    undo({
      state: after,
      dispatch: (transaction) => {
        restored = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(restored), source);
  assert.equal(restored.field(authorship).generated.size, 0);
});

test('metadata-only manual CSV intent stays within its decoded value and joins genuine input undo', () => {
  const source = 'header\n"(cat)",tail';
  const initial = valueState(source).state.update({
    effects: StateEffect.appendConfig.of([authorshipExtension, promptHistory]),
  }).state;
  const input = initial.update({
    changes: { from: 8, insert: '\\' },
    selection: { anchor: 9 },
    userEvent: 'input.type',
    annotations: Transaction.time.of(200),
  });
  const state = input.state,
    context = valueContext(state),
    target = valueTarget(state, context, 9);
  const provenance = prepareDocumentValueTypedNormalization(
    target,
    { from: 0, to: 1 },
    state.field(authorship),
    { deletion: false, manual: true, protected: false },
  );
  const outcome = prepareDocumentValueEdit(state, {
    ...decodedEdit(context, target, 0, 6, '\\(cat)', 1),
    publication: {
      purpose: 'typed',
      normalization: provenance,
      input: {
        time: 200,
        userEvent: 'input.type',
        authoredImplicitDepth: null,
      },
    },
  });
  assert.equal(outcome.status, 'unchanged');
  const transaction = outcome.edit.transactions[0]!,
    after = transaction.state;
  assert.equal(readExactSource(after), 'header\n"\\(cat)",tail');
  assert.equal(transaction.docChanged, false);
  assert.deepEqual(literalIntents(after.field(authorship)), [{ from: 8, to: 14 }]);
  assert.equal(transaction.annotation(normalizationJoin), true);
  assert.equal(undoDepth(after), 1);
  let restored = after;
  assert.equal(
    undo({
      state: after,
      dispatch: (transaction) => {
        restored = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(restored), source);
  assert.equal(restored.field(authorship).literals.size, 0);
});
