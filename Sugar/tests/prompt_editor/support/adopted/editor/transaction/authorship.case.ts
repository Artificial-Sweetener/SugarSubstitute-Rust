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
/** Actual native indexes and history prove finite authoring provenance, touched-range mapping and reused output. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState } from '@codemirror/state';
import { history, undo, redo } from '@codemirror/commands';
import { normalizeForStorage } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { prepareDocumentValueTypedNormalization } from '../../../../../../frontend/sugarprompt/editor/src/document-value-ingestion.ts';
import { prepareCompletionScope } from '../../../../../../frontend/sugarprompt/editor/src/completion-splice.ts';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import { exactSourceExtension } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  authorship,
  authorshipExtension,
  installAuthorship,
  normalizedAuthorship,
} from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import {
  generatedShells,
  hasLiteralIntent,
  preparedAuthorship,
} from '../../../../../../frontend/sugarprompt/editor/src/authorship-ranges.ts';
import {
  enteredTextInsertion,
  type PreparedNativeIngestion,
} from '../../../../../../frontend/sugarprompt/editor/src/ingestion.ts';

import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  semanticStructures,
  initializeNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';

/** Consume exact actual transactions in their prepared order without reconstructing specifications. */
function applyIngestion(plan: PreparedNativeIngestion): EditorState {
  let state = plan.startState;
  for (const transaction of plan.transactions) {
    assert.equal(transaction.startState, state);
    state = transaction.state;
  }
  return state;
}
/** Prepare identity normalization through actual canonical membership and the sole finite helper. */
function identityAuthorship(
  state: EditorState,
): import('../../../../../../frontend/sugarprompt/editor/src/authorship-ranges.ts').Authorship {
  const current = initializeNativeStructure(state),
    scope = prepareCompletionScope(current)!;
  const provenance = prepareDocumentValueTypedNormalization(
    scope.target,
    scope.target.decodedRange,
    current.field(authorship),
    { deletion: false, manual: false, protected: true },
  );
  return normalizedAuthorship(
    current,
    scope.target,
    provenance,
    scope.context.values.scopes,
    current.changes([]),
    encodeSource(current.doc.toString()),
  );
}

/** Construct the actual native metadata and history owners without browser-input claims. */
function initial(source: string): EditorState {
  return initializeNativeStructure(
    EditorState.create({
      doc: source,
      extensions: [
        exactSourceExtension(source),
        editorFreshness,
        semanticStructures,
        authorshipExtension,
        history(),
      ],
    }),
  );
}

test('native generated indexes retain actual unrelated values and map content offsets without retaining source', () => {
  const source = '(cat:1.10), (dog:1.21)',
    shells = [
      { from: 0, to: 10, contentFrom: 1, contentTo: 4, nestingDepth: 1 },
      { from: 12, to: 22, contentFrom: 13, contentTo: 16, nestingDepth: 2 },
    ];
  let state = initial(source).update({
    effects: installAuthorship.of(preparedAuthorship(shells, [])),
  }).state;
  const original = state.field(authorship),
    firstValue = original.generated.iter().value!,
    secondValue = original.generated.iter(12).value!;
  state = state.update({
    changes: { from: 1, insert: 'X' },
    userEvent: 'input.type',
  }).state;
  assert.deepEqual(generatedShells(state.field(authorship)), [
    { from: 0, to: 11, contentFrom: 1, contentTo: 5, nestingDepth: 1 },
    { from: 13, to: 23, contentFrom: 14, contentTo: 17, nestingDepth: 2 },
  ]);
  assert.notEqual(state.field(authorship).generated.iter().value, firstValue);
  assert.equal(state.field(authorship).generated.iter(13).value, secondValue);
  state = state.update({
    changes: { from: 6, to: 10, insert: '2.00' },
    userEvent: 'input.type',
  }).state;
  assert.equal(state.field(authorship).generated.size, 1);
  assert.equal(state.field(authorship).generated.iter().value, secondValue);
  const prepared = state.field(authorship);
  state = state.update({ changes: { from: 23, insert: ' unrelated' } }).state;
  assert.equal(state.field(authorship), prepared);
});

test('large native indexes reuse actual distant shell values when a preceding insertion shifts their positions', () => {
  const count = 4000,
    source = '(x:1.10), '.repeat(count);
  const generated = Array.from({ length: count }, (_, index) => ({
    from: index * 10,
    to: index * 10 + 8,
    contentFrom: index * 10 + 1,
    contentTo: index * 10 + 2,
    nestingDepth: 1,
  }));
  let state = initial(source).update({
    effects: installAuthorship.of(preparedAuthorship(generated, [])),
  }).state;
  const first = state.field(authorship).generated.iter().value,
    last = state.field(authorship).generated.iter((count - 1) * 10).value;
  state = state.update({ changes: { from: 0, insert: 'prefix ' } }).state;
  const current = state.field(authorship).generated;
  assert.equal(current.size, count);
  assert.equal(current.iter(7).value, first);
  assert.equal(current.iter((count - 1) * 10 + 7).value, last);
  assert.equal(current.iter((count - 1) * 10 + 7).from, (count - 1) * 10 + 7);
});

test('manual interval queries preserve contained edits and release comma changes or complete replacement', () => {
  let state = initial('(cat), tail').update({
    effects: installAuthorship.of(preparedAuthorship([], [{ from: 0, to: 5 }])),
  }).state;
  assert.equal(hasLiteralIntent(state.field(authorship), 1, 4), true);
  assert.equal(hasLiteralIntent(state.field(authorship), 7, 8), false);
  state = state.update({ changes: { from: 2, insert: 'X' } }).state;
  assert.equal(hasLiteralIntent(state.field(authorship), 0, 6), true);
  const withIntent = state;
  assert.equal(
    withIntent.update({ changes: { from: 2, insert: ',' } }).state.field(authorship).literals.size,
    0,
  );
  assert.equal(
    withIntent.update({ changes: { from: 0, to: 6, insert: '(dog)' } }).state.field(authorship)
      .literals.size,
    0,
  );
});

test("generated shell provenance and native history restore each shell's independent depth", () => {
  let state = initial('');
  state = applyIngestion(enteredTextInsertion(state, 0, 0, '(cat) ((dog))'));
  const prepared = state.field(authorship);
  assert.deepEqual(
    generatedShells(prepared).map(({ from, to, nestingDepth }) => ({
      from,
      to,
      nestingDepth,
    })),
    [
      { from: 0, to: 10, nestingDepth: 1 },
      { from: 11, to: 21, nestingDepth: 2 },
    ],
  );
  assert.equal(identityAuthorship(state), prepared);
  const target = {
    /** Native history always observes the currently accepted state. */
    get state() {
      return state;
    },
    /** Accept the transaction supplied by CodeMirror's own undo/redo command. */
    dispatch(transaction: import('@codemirror/state').Transaction) {
      state = transaction.state;
    },
  };
  assert.equal(undo(target), true);
  assert.equal(state.doc.length, 0);
  assert.equal(state.field(authorship).generated.size, 0);
  assert.equal(redo(target), true);
  assert.equal(state.field(authorship), prepared);
  const normalization = normalizeForStorage('(cat) ((dog))');
  assert.equal(normalization.generatedEmphases.length, 2);
});

test('metadata-only normalized insertion forms its own undo event even when exact source is unchanged', () => {
  let state = initial('(cat:1.10)');
  state = applyIngestion(enteredTextInsertion(state, 0, 10, '(cat)'));
  assert.equal(state.field(authorship).generated.size, 1);
  assert.equal(state.doc.toString(), '(cat:1.10)');
  const target = {
    /** Delegate current state observation to native history. */
    get state() {
      return state;
    },
    /** Accept only the native history transaction. */
    dispatch(transaction: import('@codemirror/state').Transaction) {
      state = transaction.state;
    },
  };
  assert.equal(undo(target), true);
  assert.equal(state.doc.toString(), '(cat:1.10)');
  assert.equal(state.field(authorship).generated.size, 0);
  assert.equal(redo(target), true);
  assert.equal(state.field(authorship).generated.size, 1);
});
