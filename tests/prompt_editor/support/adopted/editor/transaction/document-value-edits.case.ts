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
/** Decoded transformations preserve authored storage, scalar coordinates and native directional history. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, Transaction } from '@codemirror/state';
import { undo, redo, undoDepth } from '@codemirror/commands';
import { prepareDocumentSemantics } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { prepareDocumentValueEdit } from '../../../../../../frontend/sugarprompt/editor/src/document-value-edit-issuance.ts';
import {
  valueState,
  valueContext,
  valueTarget,
  decodedEdit,
  acceptedEdit,
  editedState,
} from './document-value-fixture.ts';
import {
  prepareNativeStructure,
  semanticStructures,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { StateEffect } from '@codemirror/state';

test('quoted emphasis and region preparation use actual canonical mapped ownership', () => {
  let { state } = valueState('h\r\n" (a:1.00) "');
  state = state.update({
    effects: StateEffect.appendConfig.of(semanticStructures),
  }).state;
  const context = valueContext(state),
    entry = context.values.entries[1]!;
  const document = entry.scope!.document;
  assert.equal(document.emphasisSpans.length, 1);
  assert.deepEqual(
    entry.semanticMapping!.decodedToLogical.slice(0, 9),
    [4, 5, 6, 7, 8, 9, 10, 11, 12],
  );
  const regional = valueState('h\n"x\n[SEP|A]\ny"').state.update({
    effects: StateEffect.appendConfig.of(semanticStructures),
  }).state;
  const region = valueContext(regional).values.entries[1]!;
  assert.equal(region.scope!.document.regions.separators.length, 1);
  assert.deepEqual(
    region.semanticMapping!.decodedToLogical,
    [3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
  );
  const canonical = prepareNativeStructure(regional);
  assert.equal(canonical.regions.length, 1);
  const mappedRegion = canonical.regions[0]!;
  assert.deepEqual(
    [mappedRegion.from, mappedRegion.to, mappedRegion.nameFrom, mappedRegion.nameTo],
    [5, 12, 10, 11],
  );
  assert.deepEqual(mappedRegion.separator.tokenRange, { from: 2, to: 9 });
  assert.deepEqual(mappedRegion.separator.nameRange, { from: 7, to: 8 });
  assert.equal(mappedRegion.scopeId, canonical.scopes.scopes[0]!.id);
  const sourceMap = canonical.scopes.scopes[0]!.mapping.decodedToSource;
  assert.deepEqual([sourceMap[2], sourceMap[9], sourceMap[7], sourceMap[8]], [5, 12, 10, 11]);
  const intent = decodedEdit(context, valueTarget(state, context, 4), 3, 7, '2.00', 8, 0);
  const edit = acceptedEdit(state, intent),
    next = editedState(edit);
  assert.equal(readExactSource(next) === 'h\r\n" (a:2.00) "', true);
  assert.equal(next.selection.main.anchor, 12);
  assert.equal(next.selection.main.head, 4);
  assert.equal(edit.transactions.length, 1);
  assert.equal(edit.transactions[0]!.startState, state);
  assert.equal(edit.preparedValues, edit.replacement.preparedValues);
  assert.equal(edit.postValues.scopes.values, edit.preparedValues);
  assert.equal(edit.semantics.preparedValues, edit.preparedValues);
  assert.equal(
    prepareDocumentSemantics(
      edit.preparedValues.source,
      'wildcard-csv',
      edit.semantics,
      edit.preparedValues,
    ),
    edit.semantics,
  );
});

test('quote/comma/embedded endings reencode with authored padding, untouched cells/raw seams and full-container directional selection', () => {
  const { state } = valueState('H,Z\r\n"  a  " \t,keep\rb,tail\n');
  const context = valueContext(state),
    target = valueTarget(state, context, 7);
  const edit = acceptedEdit(state, decodedEdit(context, target, 0, 1, 'q,"r\r\ns', 9, -2));
  const next = editedState(edit);
  assert.equal(readExactSource(next) === 'H,Z\r\n"  q,""r\r\ns  " \t,keep\rb,tail\n', true);
  assert.deepEqual(next.selection.main, EditorSelection.range(16, 5));
  assert.equal(edit.preparedValues.csv!.records.length, 3);
  assert.deepEqual(
    edit.preparedValues.csv!.records.map((row) => row.length),
    [2, 2, 2],
  );
  assert.equal(edit.preparedValues.csv!.records[1]![1]!.value === 'keep', true);
  assert.equal(edit.preparedValues.csv!.records[2]![0]!.value === 'b', true);
  assert.equal(edit.postValues.entries[3]!.scope!.document.sourceText === 'keep', true);
  assert.equal(undoDepth(next), 1);
  let restored = next;
  assert.equal(
    undo({
      state: restored,
      dispatch: (transaction) => {
        restored = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(restored) === 'H,Z\r\n"  a  " \t,keep\rb,tail\n', true);
  assert.equal(
    redo({
      state: restored,
      dispatch: (transaction) => {
        restored = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(restored) === readExactSource(next), true);
});

test('appended decoded text and newly added trim retain the old padding origin rather than a new trimmed start', () => {
  const { state } = valueState('h\n"  a  "');
  const context = valueContext(state),
    target = valueTarget(state, context, 5);
  const appended = acceptedEdit(state, decodedEdit(context, target, 1, 1, ',b', 3, -2));
  assert.equal(readExactSource(editedState(appended)) === 'h\n"  a,b  "', true);
  assert.deepEqual(editedState(appended).selection.main, EditorSelection.range(8, 3));
  const padded = acceptedEdit(state, decodedEdit(context, target, 0, 1, ' x ', 0, 3));
  assert.equal(readExactSource(editedState(padded)) === 'h\n"   x   "', true);
  assert.deepEqual(editedState(padded).selection.main, EditorSelection.range(5, 8));
  assert.equal(padded.postValues.scopes.scopes[0]!.container.valueRange.from, 3);
});

test('empty quoted/unquoted/whitespace/final TXT anchors produce genuine edits with unchanged cardinality', () => {
  for (const [source, position, kind, expected] of [
    ['h\n""', 3, 'wildcard-csv', 'h\n"x"'],
    ['h\n,', 2, 'wildcard-csv', 'h\nx,'],
    ['h\n"  "', 5, 'wildcard-csv', 'h\n"  x"'],
    ['a\r\n', 2, 'wildcard-txt', 'a\r\nx'],
    ['', 0, 'wildcard-txt', 'x'],
  ] as const) {
    const { state } = valueState(source, kind),
      context = valueContext(state);
    const edit = acceptedEdit(
      state,
      decodedEdit(context, valueTarget(state, context, position), 0, 0, 'x'),
    );
    assert.equal(readExactSource(editedState(edit)) === expected, true);
    assert.equal(
      edit.preparedValues.containers.length,
      context.values.scopes.values.containers.length,
    );
  }
});

test('old scalar/CRLF gaps and newly formed scalar/CRLF selection seams reject atomically, each with a valid followup', () => {
  for (const [source, from, to, text, anchor, reason] of [
    ['h\n"😀a"', 1, 1, 'x', 2, 'invalid-boundary'],
    ['h\n"a\r\nb"', 2, 2, 'x', 3, 'invalid-boundary'],
    ['h\n"ab"', 0, 1, '😀', 1, 'unsafe-selection'],
    ['h\n"ab"', 0, 1, '\r\n', 1, 'unsafe-selection'],
    ['h\n"\ud800a"', 1, 2, '\udc00', 2, 'invalid-boundary'],
    ['h\n"a\nb"', 0, 1, '\r', 1, 'invalid-boundary'],
  ] as const) {
    const { state } = valueState(source),
      context = valueContext(state),
      target = valueTarget(state, context, 3);
    assert.deepEqual(
      prepareDocumentValueEdit(state, decodedEdit(context, target, from, to, text, anchor)),
      { status: 'rejected', reason },
    );
    assert.equal(readExactSource(state) === source, true);
    assert.equal(undoDepth(state), 0);
    const valid = acceptedEdit(
      state,
      decodedEdit(context, target, 0, target.scope.mapping.text.length, 'z'),
    );
    assert.equal(readExactSource(editedState(valid)) === 'h\n"z"', true);
  }
});

test('TXT semantic newline cardinality rejects for every physical ending; raw exact APIs still admit literal whole-source edits', () => {
  for (const ending of [
    '\n',
    '\r',
    '\r\n',
    '\v',
    '\f',
    '\x1c',
    '\x1d',
    '\x1e',
    '\x85',
    '\u2028',
    '\u2029',
  ]) {
    const { state } = valueState(' a \r\nkeep', 'wildcard-txt'),
      context = valueContext(state),
      target = valueTarget(state, context, 1);
    assert.deepEqual(
      prepareDocumentValueEdit(state, decodedEdit(context, target, 0, 1, `x${ending}y`)),
      { status: 'rejected', reason: 'cardinality' },
    );
    const valid = acceptedEdit(state, decodedEdit(context, target, 0, 1, 'z'));
    assert.equal(readExactSource(editedState(valid)) === ' z \r\nkeep', true);
    const raw = state.update(exactReplacement(state, 0, 1, `q${ending}`)).state;
    assert.equal(readExactSource(raw) === `q${ending}a \r\nkeep`, true);
  }
});

test('no-op retains actual values/scopes/full maps/native output and selection-only transactions avoid history', () => {
  const { state } = valueState('h\n" a "'),
    context = valueContext(state),
    target = valueTarget(state, context, 4);
  const selected = state.update({
    selection: EditorSelection.single(5, 4),
  }).state;
  const fresh = { ...context },
    selectedTarget = valueTarget(selected, fresh, 4);
  const noOp = acceptedEdit(selected, decodedEdit(fresh, selectedTarget, 0, 1, 'a', 1, 0));
  assert.equal(noOp.transactions.length, 0);
  assert.equal(noOp.preparedValues, context.values.scopes.values);
  assert.equal(noOp.postValues, context.values);
  assert.equal(noOp.semantics, context.semantics);
  assert.equal(noOp.postValues.entries[1]!.fullMapping, context.values.entries[1]!.fullMapping);
  const moved = acceptedEdit(state, decodedEdit(context, target, 0, 1, 'a', 1, 0));
  assert.equal(moved.transactions.length, 1);
  assert.equal(moved.transactions[0]!.docChanged, false);
  assert.equal(moved.transactions[0]!.annotation(Transaction.addToHistory), false);
  assert.equal(moved.postValues, context.values);
  assert.equal(undoDepth(editedState(moved)), 0);
});

test('outside directional selections follow actual ChangeDesc; main range remains decoded and unsafe choices have valid followups', () => {
  const initial = valueState('h,z\na,b').state;
  const state = initial.update({
    selection: EditorSelection.create(
      [EditorSelection.range(3, 0), EditorSelection.range(4, 5), EditorSelection.range(7, 6)],
      1,
    ),
  }).state;
  const context = valueContext(state),
    target = valueTarget(state, context, 4);
  const base = decodedEdit(context, target, 0, 1, 'long', 4, 0);
  const intent = {
    ...base,
    selection: {
      ranges: [
        { kind: 'outside', index: 0 },
        { kind: 'decoded', anchor: 4, head: 0 },
        { kind: 'outside', index: 2 },
      ] as const,
      mainIndex: 1,
    },
  };
  const edit = acceptedEdit(state, intent),
    next = editedState(edit);
  assert.deepEqual(
    next.selection.ranges.map((range) => [range.anchor, range.head]),
    [
      [3, 0],
      [8, 4],
      [10, 9],
    ],
  );
  assert.equal(next.selection.mainIndex, 1);
  for (const selection of [
    { ranges: [{ kind: 'outside', index: 1 }] as const, mainIndex: 0 },
    { ranges: [{ kind: 'outside', index: 9 }] as const, mainIndex: 0 },
    {
      ranges: [{ kind: 'decoded', anchor: -1, head: 0 }] as const,
      mainIndex: 0,
    },
    { ranges: [], mainIndex: 0 },
  ]) {
    assert.deepEqual(prepareDocumentValueEdit(state, { ...base, selection }), {
      status: 'rejected',
      reason: 'unsafe-selection',
    });
    assert.equal(acceptedEdit(state, intent).transactions.length, 1);
  }
});

test('outside scalar interiors cannot gain safe selection authority through an encoded edit', () => {
  const initial = valueState('h,z\na,😀').state;
  const state = initial.update({
    selection: EditorSelection.create([EditorSelection.cursor(4), EditorSelection.cursor(7)], 0),
  }).state;
  const context = valueContext(state),
    target = valueTarget(state, context, 4);
  const base = decodedEdit(context, target, 0, 1, 'long');
  assert.deepEqual(
    prepareDocumentValueEdit(state, {
      ...base,
      selection: {
        ranges: [
          { kind: 'decoded', anchor: 4, head: 0 },
          { kind: 'outside', index: 1 },
        ],
        mainIndex: 0,
      },
    }),
    { status: 'rejected', reason: 'unsafe-selection' },
  );
  assert.equal(acceptedEdit(state, base).transactions.length, 1);
});

test('CSV final-empty records and outer CR/LF seams remain present through actual decoded edits', () => {
  for (const [source, from, to, text, expected] of [
    ['h\nlast', 0, 4, '', 'h\n""'],
    ['h\r\nx\ry', 0, 1, '\n', 'h\r\n"\n"\ry'],
    ['h\nx\r\ny', 0, 1, '\r', 'h\n"\r"\r\ny'],
  ] as const) {
    const { state } = valueState(source),
      context = valueContext(state);
    const target = valueTarget(state, context, 2);
    const edit = acceptedEdit(state, decodedEdit(context, target, from, to, text));
    assert.equal(readExactSource(editedState(edit)) === expected, true);
    assert.equal(
      edit.preparedValues.csv!.records.length,
      context.values.scopes.values.csv!.records.length,
    );
  }
});
