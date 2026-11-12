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
/** Genuine reorder construction preserves generated and literal provenance through native undo/redo. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undo, redo } from '@codemirror/commands';
import {
  moveReorderChip,
  type ReorderDropTarget,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  authorship,
  installAuthorship,
} from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import {
  preparedAuthorship,
  generatedShells,
  literalIntents,
} from '../../../../../../frontend/sugarprompt/editor/src/authorship-ranges.ts';
import { prepareReorderAuthorship } from '../../../../../../frontend/sugarprompt/editor/src/reorder-command.ts';
import { prepareReorderCommit } from '../../../../../../frontend/sugarprompt/editor/src/reorder-command.ts';
import { currentDocumentValueEdit } from '../../../../../../frontend/sugarprompt/editor/src/document-value-edit-issuance.ts';
import { prepareDocumentValueEdit } from '../../../../../../frontend/sugarprompt/editor/src/document-value-edit-issuance.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { captureReorder, reorderState, transactionState } from './reorder-fixture.ts';

test('one generated grouped shell reopens by actual original span identity and native history restores its original index', () => {
  let { state } = reorderState('(a, b:1), tail', 1);
  const before = captureReorder(state),
    span = before.target.scope.document.emphasisSpans[0]!;
  const metadata = preparedAuthorship(
    [
      {
        from: span.outerRange.from,
        to: span.outerRange.to,
        contentFrom: span.contentRange.from,
        contentTo: span.contentRange.to,
        nestingDepth: 1,
      },
    ],
    [],
  );
  state = state.update({ effects: installAuthorship.of(metadata) }).state;
  const capture = captureReorder(state),
    order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 2,
    })!,
    result = prepareReorderCommit(state, capture, order);
  assert.equal(result.status, 'ready');
  const tx = result.edit.transactions[0]!;
  assert.equal(readExactSource(tx.state) === '(b:1), tail, (a:1)', true);
  assert.deepEqual(
    generatedShells(tx.state.field(authorship)).map((s) => [
      s.from,
      s.to,
      s.contentFrom,
      s.contentTo,
      s.nestingDepth,
    ]),
    [
      [0, 5, 1, 2, 1],
      [13, 18, 14, 15, 1],
    ],
  );
  assert.equal(currentDocumentValueEdit(state, capture.context, result.edit), null);
  const native = transactionState(state);
  native.dispatch(tx);
  assert.equal(undo(native), true);
  assert.equal(native.state.field(authorship), metadata);
  assert.equal(redo(native), true);
  assert.equal(generatedShells(native.state.field(authorship)).length, 2);
});

test('equal manual bodies and weights never inherit another original generated shell identity', () => {
  let { state } = reorderState('(a:1), (a:1), tail', 1);
  const initial = captureReorder(state),
    first = initial.target.scope.document.emphasisSpans[0]!;
  state = state.update({
    effects: installAuthorship.of(
      preparedAuthorship(
        [
          {
            from: first.outerRange.from,
            to: first.outerRange.to,
            contentFrom: first.contentRange.from,
            contentTo: first.contentRange.to,
            nestingDepth: 1,
          },
        ],
        [],
      ),
    ),
  }).state;
  const capture = captureReorder(state),
    order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 2,
    })!,
    result = prepareReorderCommit(state, capture, order);
  assert.equal(result.status, 'ready');
  assert.deepEqual(
    generatedShells(result.edit.transactions[0]!.state.field(authorship)).map((s) => [
      s.from,
      s.to,
    ]),
    [[13, 18]],
  );
});

test('copied literal escaped parentheses retain exact intent while authored source outside the value remains native-mapped', () => {
  let { state } = reorderState('a\\(x\\), b', 0);
  state = state.update({
    effects: installAuthorship.of(preparedAuthorship([], [{ from: 0, to: 6 }])),
  }).state;
  const capture = captureReorder(state),
    order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 1,
    })!,
    result = prepareReorderCommit(state, capture, order);
  assert.equal(result.status, 'ready');
  assert.equal(readExactSource(result.edit.transactions[0]!.state) === 'b, a\\(x\\)', true);
  assert.deepEqual(literalIntents(result.edit.transactions[0]!.state.field(authorship)), [
    { from: 3, to: 9 },
  ]);
});

test('intact nested generated spans map by exact bytes and current post-parse bounds', () => {
  let { state } = reorderState('((a:1):1), b', 2);
  const initial = captureReorder(state);
  const spans = initial.target.scope.document.emphasisSpans;
  state = state.update({
    effects: installAuthorship.of(
      preparedAuthorship(
        spans.map((span, index) => ({
          from: span.outerRange.from,
          to: span.outerRange.to,
          contentFrom: span.contentRange.from,
          contentTo: span.contentRange.to,
          nestingDepth: index + 1,
        })),
        [],
      ),
    ),
  }).state;
  const capture = captureReorder(state),
    target: ReorderDropTarget = { kind: 'row', row: 0, insertion: 1 },
    order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, target)!,
    result = prepareReorderCommit(state, capture, order);
  assert.equal(result.status, 'ready');
  assert.equal(readExactSource(result.edit.transactions[0]!.state) === 'b, ((a:1):1)', true);
  assert.equal(generatedShells(result.edit.transactions[0]!.state.field(authorship)).length, 2);
});

test('literal endpoint becoming an output CRLF interior refuses atomically without guessing and genuine no-op remains reusable', () => {
  let { state } = reorderState('alpha\r\nbeta\rgamma\n', 11);
  state = state.update({
    effects: installAuthorship.of(preparedAuthorship([], [{ from: 6, to: 11 }])),
  }).state;
  const capture = captureReorder(state),
    order = moveReorderChip(capture.prepared, capture.prepared.initial, 2, {
      kind: 'row',
      row: 0,
      insertion: 0,
    })!;
  const result = prepareReorderCommit(state, capture, order);
  assert.deepEqual(result, { status: 'rejected', reason: 'unsafe-origin' });
  assert.equal(readExactSource(state) === 'alpha\r\nbeta\rgamma\n', true);
  assert.equal(prepareReorderCommit(state, capture, capture.prepared.initial).status, 'unchanged');
});

test('foreign authored DTO cannot grant reorder provenance and genuine following command still issues', () => {
  const { state } = reorderState('a, b'),
    capture = captureReorder(state);
  const result = prepareDocumentValueEdit(state, {
    context: capture.context,
    target: capture.target,
    range: { from: 0, to: 4 },
    text: 'b, a',
    selection: {
      ranges: [{ kind: 'decoded', anchor: 3, head: 3 }],
      mainIndex: 0,
    },
    publication: { purpose: 'reorder', reorder: { identity: Symbol() } },
  });
  assert.deepEqual(result, { status: 'rejected', reason: 'unsafe-origin' });
  const order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
    kind: 'row',
    row: 0,
    insertion: 1,
  })!;
  assert.equal(prepareReorderCommit(state, capture, order).status, 'ready');
});

test('structured multiline move keeps exact trailing CRLF despite post-scope trimming and preserves actual generated span', () => {
  let { state } = reorderState('h,k\r\n"(a, b:1)\r\nc",keep', 6, 6, 'wildcard-csv');
  const initial = captureReorder(state),
    span = initial.target.scope.document.emphasisSpans[0]!,
    map = initial.target.entry.semanticMapping!;
  state = state.update({
    effects: installAuthorship.of(
      preparedAuthorship(
        [
          {
            from: map.decodedToLogical[span.outerRange.from]!,
            to: map.decodedToLogical[span.outerRange.to]!,
            contentFrom: map.decodedToLogical[span.contentRange.from]!,
            contentTo: map.decodedToLogical[span.contentRange.to]!,
            nestingDepth: 1,
          },
        ],
        [],
      ),
    ),
  }).state;
  const capture = captureReorder(state),
    order = moveReorderChip(capture.prepared, capture.prepared.initial, 1, {
      kind: 'row',
      row: 0,
      insertion: 0,
    })!,
    result = prepareReorderCommit(state, capture, order);
  assert.equal(result.status, 'ready');
  assert.equal(
    readExactSource(result.edit.transactions[0]!.state) === 'h,k\r\n"c, (a, b:1)\r\n",keep',
    true,
  );
  assert.equal(generatedShells(result.edit.transactions[0]!.state.field(authorship)).length, 1);
});

test('literal interval including real structured padding conserves separate original padding and moved body images', () => {
  let { state } = reorderState('h,k\r\n" a\\(x\\), b ",keep', 6, 6, 'wildcard-csv');
  state = state.update({
    effects: installAuthorship.of(preparedAuthorship([], [{ from: 5, to: 12 }])),
  }).state;
  const capture = captureReorder(state),
    order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 1,
    })!,
    result = prepareReorderCommit(state, capture, order);
  assert.equal(result.status, 'ready');
  const next = result.edit.transactions[0]!.state;
  assert.equal(readExactSource(next) === 'h,k\r\n" b, a\\(x\\) ",keep', true);
  assert.deepEqual(literalIntents(next.field(authorship)), [
    { from: 5, to: 6 },
    { from: 9, to: 15 },
  ]);
});

for (const variant of ['extra-output', 'insertion-only'] as const)
  test(`a genuine reorder receipt refuses ${variant} issuer input and retains a valid exact followup`, () => {
    let { state } = reorderState('(a, b:1), tail', 1);
    const initial = captureReorder(state),
      span = initial.target.scope.document.emphasisSpans[0]!;
    state = state.update({
      effects: installAuthorship.of(
        preparedAuthorship(
          [
            {
              from: span.outerRange.from,
              to: span.outerRange.to,
              contentFrom: span.contentRange.from,
              contentTo: span.contentRange.to,
              nestingDepth: 1,
            },
          ],
          [],
        ),
      ),
    }).state;
    const capture = captureReorder(state),
      order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
        kind: 'row',
        row: 0,
        insertion: 2,
      })!;
    const prepared = prepareReorderAuthorship(state, capture, order)!;
    const outcome = prepareDocumentValueEdit(state, {
      context: capture.context,
      target: capture.target,
      range: {
        from: 0,
        to: variant === 'insertion-only' ? 0 : capture.prepared.scope.document.sourceText.length,
      },
      text:
        prepared.output.text +
        (variant === 'extra-output' ? ', manual' : variant === 'insertion-only' ? ', ' : ''),
      selection: {
        ranges: [{ kind: 'decoded', anchor: 0, head: 0 }],
        mainIndex: 0,
      },
      publication: { purpose: 'reorder', reorder: prepared.provenance },
    });
    assert.deepEqual(outcome, { status: 'rejected', reason: 'unsafe-origin' });
    const genuine = prepareReorderCommit(state, capture, order);
    assert.equal(genuine.status, 'ready');
    assert.equal(currentDocumentValueEdit(state, capture.context, genuine.edit), null);
    assert.equal(readExactSource(genuine.edit.transactions[0]!.state), prepared.output.text);
    assert.equal(generatedShells(genuine.edit.transactions[0]!.state.field(authorship)).length, 2);
  });

test('equal complete post-source from a partial native replacement cannot borrow a whole-scope reorder receipt', () => {
  const { state } = reorderState('a, b, tail'),
    capture = captureReorder(state);
  const order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
    kind: 'row',
    row: 0,
    insertion: 1,
  })!;
  const prepared = prepareReorderAuthorship(state, capture, order)!;
  assert.equal(prepared.output.text, 'b, a, tail');
  const outcome = prepareDocumentValueEdit(state, {
    context: capture.context,
    target: capture.target,
    range: { from: 0, to: 4 },
    text: 'b, a',
    selection: {
      ranges: [{ kind: 'decoded', anchor: 0, head: 0 }],
      mainIndex: 0,
    },
    publication: { purpose: 'reorder', reorder: prepared.provenance },
  });
  assert.deepEqual(outcome, { status: 'rejected', reason: 'unsafe-origin' });
  assert.equal(prepareReorderCommit(state, capture, order).status, 'ready');
});
