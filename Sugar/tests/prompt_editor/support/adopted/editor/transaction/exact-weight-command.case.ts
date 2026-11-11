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
import { EditorSelection, EditorState, StateEffect } from '@codemirror/state';
import {
  semanticStructures,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { richProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
/** Exact numeric completion contracts use native source/history state independently of browser draft input. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undo, redo, undoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { prepareWeightChange } from '../../../../../../frontend/sugarprompt/editor/src/weight-command.ts';
import { weightStateFor } from './weight-command-state.ts';

test('exact completion moves after the token without source history for an unchanged value and rejects stale captured intent', () => {
  const initial = weightStateFor('(cat:1.20)\r\nnext', 14, 11);
  const request = {
    owner: 'exact',
    completion: 'keyboard',
    neutral: false,
    freshness: initial.field(editorFreshness),
    kind: 'emphasis',
    range: { from: 1, to: 4 },
  } as const;
  const same = prepareWeightChange(initial, { kind: 'set', weight: '1.20' }, request).at(-1)!.state;
  assert.equal(same.selection.main.head, 10);
  assert.equal(undoDepth(same), 0);
  const changed = prepareWeightChange(initial, { kind: 'set', weight: '-.5' }, request).at(
    -1,
  )!.state;
  assert.equal(readExactSource(changed), '(cat:-0.50)\r\nnext');
  assert.equal(changed.selection.main.head, 11);
  assert.equal(undoDepth(changed), 1);
  assert.deepEqual(prepareWeightChange(changed, { kind: 'set', weight: '2.00' }, request), []);
});

test('captured exact neutral intent survives later pointer selection without adopting its enclosing shell', () => {
  const initial = weightStateFor('(cat dog:1.10)', 12);
  const request = {
    owner: 'exact',
    completion: 'outside',
    neutral: true,
    freshness: initial.field(editorFreshness),
    kind: 'emphasis',
    range: { from: 1, to: 4 },
  } as const;
  const changed = prepareWeightChange(initial, { kind: 'set', weight: '0.95' }, request).at(
    -1,
  )!.state;
  assert.equal(readExactSource(changed), '((cat:0.95) dog:1.10)');
  // The later pointer caret inside the outer weight is kept, mapped past the inserted shell.
  assert.equal(changed.selection.main.head, 19);
});

test('unchanged exact neutral entry retains the inner boundary when its content equals the parent content', () => {
  const initial = weightStateFor('(cat:1.10)', 9);
  const unchanged = prepareWeightChange(
    initial,
    { kind: 'set', weight: '1.00' },
    {
      owner: 'exact',
      completion: 'keyboard',
      neutral: true,
      freshness: initial.field(editorFreshness),
      kind: 'emphasis',
      range: { from: 1, to: 4 },
    },
  ).at(-1)!.state;
  assert.equal(readExactSource(unchanged), '(cat:1.10)');
  assert.equal(unchanged.selection.main.head, 4);
  assert.equal(undoDepth(unchanged), 0);
});

test('CSV canonical weight edits conserve quoted Unicode/newline content, neighbors, native directional history and populated no-op output', () => {
  const source = 'H,Z\r\n"  (🙂:1.20), a""b\r\nc  ",tail\rend,stay\n';
  let state = weightStateFor(source, 0, 0, 'wildcard-csv');
  const prepared = state.field(semanticStructures).native!,
    entry = prepared.nativeValues.entries[2]!,
    span = entry.scope!.document.emphasisSpans[0]!,
    map = entry.semanticMapping!;
  const request = {
    owner: 'exact',
    completion: 'keyboard',
    neutral: false,
    freshness: state.field(editorFreshness),
    kind: 'emphasis',
    range: {
      from: map.decodedToLogical[span.contentRange.from]!,
      to: map.decodedToLogical[span.contentRange.to]!,
    },
  } as const;
  const selection = EditorSelection.single(state.doc.length, state.doc.length - 4);
  state = state.update({ selection }).state;
  const original = state;
  const transactions = prepareWeightChange(state, { kind: 'set', weight: '2.00' }, request);
  assert.equal(transactions.length, 2);
  state = transactions.at(-1)!.state;
  assert.equal(readExactSource(state) === source.replace('1.20', '2.00'), true);
  assert.equal(
    state.field(semanticStructures).native!.nativeValues.scopes.values,
    state.field(semanticStructures).native!.documentSemantics.preparedValues,
  );
  assert.equal(prepareNativeStructure(state), state.field(semanticStructures).native);
  assert.equal(state.field(richProjection).sourceIdentity, state.field(editorFreshness).source);
  assert.equal(undoDepth(state), 1);
  const changedSelection = state.selection;
  assert.equal(
    undo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(state) === source, true);
  assert.equal(state.selection.eq(original.selection), true);
  assert.equal(
    redo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(state) === source.replace('1.20', '2.00'), true);
  assert.equal(state.selection.eq(changedSelection), true);
  assert.equal(prepareWeightChange(state, { kind: 'set', weight: '3.00' }, request).length, 0);
  state = transactions.at(-1)!.state;
  const current = state.field(semanticStructures).native!,
    freshEntry = current.nativeValues.byId[entry.scope!.id]!,
    freshSpan = freshEntry.scope!.document.emphasisSpans[0]!;
  const same = prepareWeightChange(
    state,
    { kind: 'set', weight: '2.00' },
    {
      ...request,
      freshness: state.field(editorFreshness),
      range: {
        from: freshEntry.semanticMapping!.decodedToLogical[freshSpan.contentRange.from]!,
        to: freshEntry.semanticMapping!.decodedToLogical[freshSpan.contentRange.to]!,
      },
    },
  ).at(-1)!.state;
  assert.equal(same.field(semanticStructures).native, current);
  assert.equal(same.field(semanticStructures).native!.nativeValues, current.nativeValues);
  assert.equal(
    same.field(semanticStructures).native!.scopes.scopes[0]!.document.emphasisSpans.length,
    1,
  );
  assert.equal(undoDepth(same), 1);
});

test('CSV native admission refuses headers, padding, cross-value, malformed, scalar/quote gaps and foreign requests with valid current followup', () => {
  const source = 'H,Z\r\n"  (🙂:1.20), a""b  ",tail';
  const state = weightStateFor(source, 0, 0, 'wildcard-csv'),
    prepared = state.field(semanticStructures).native!,
    entry = prepared.nativeValues.entries[2]!,
    span = entry.scope!.document.emphasisSpans[0]!,
    map = entry.semanticMapping!;
  const from = map.decodedToLogical[span.contentRange.from]!,
    to = map.decodedToLogical[span.contentRange.to]!;
  for (const [anchor, head] of [
    [0, 1],
    [entry.logicalStorage.from + 1, entry.logicalStorage.from + 2],
    [from, state.doc.length],
    [from + 1, from + 1],
    [state.doc.toString().indexOf('""') + 1, state.doc.toString().indexOf('""') + 1],
  ] as const) {
    const selected = state.update({ selection: { anchor, head } }).state;
    assert.equal(prepareWeightChange(selected, { kind: 'step', direction: 'increase' }).length, 0);
    assert.equal(readExactSource(selected) === source, true);
    assert.equal(undoDepth(selected), 0);
  }
  const malformed = weightStateFor('H\r\n"unclosed', 4, 4, 'wildcard-csv');
  assert.equal(prepareWeightChange(malformed, { kind: 'step', direction: 'increase' }).length, 0);
  const foreign = weightStateFor(source, 0, 0, 'wildcard-csv');
  assert.equal(
    prepareWeightChange(
      state,
      { kind: 'set', weight: '2.00' },
      {
        owner: 'exact',
        completion: 'keyboard',
        neutral: false,
        kind: 'emphasis',
        range: { from, to },
        freshness: foreign.field(editorFreshness),
      },
    ).length,
    0,
  );
  const changed = prepareWeightChange(
    state,
    { kind: 'set', weight: '2.00' },
    {
      owner: 'exact',
      completion: 'keyboard',
      neutral: false,
      kind: 'emphasis',
      range: { from, to },
      freshness: state.field(editorFreshness),
    },
  ).at(-1)!.state;
  assert.equal(readExactSource(changed) === source.replace('1.20', '2.00'), true);
  const filtered = state.update({
    effects: StateEffect.appendConfig.of(
      EditorState.transactionFilter.of((transaction) =>
        transaction.docChanged
          ? [
              { changes: { from: 0, insert: 'x' } },
              { sequential: true, selection: transaction.newSelection },
            ]
          : transaction,
      ),
    ),
  }).state;
  assert.throws(
    () =>
      prepareWeightChange(
        filtered,
        { kind: 'set', weight: '2.00' },
        {
          owner: 'exact',
          completion: 'keyboard',
          neutral: false,
          kind: 'emphasis',
          range: { from, to },
          freshness: filtered.field(editorFreshness),
        },
      ),
    /decoded-edit-actual-result-mismatch/u,
  );
});
