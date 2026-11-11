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
/** Independent literal coordinates characterize real native adaptation, not mounted browser interaction. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, EditorState, StateEffect } from '@codemirror/state';
import { prepareDocumentPromptScopes } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { prepareDocumentValues } from '../../../../../../frontend/sugarprompt/core/src/document/values.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import { PreparedSourceEdits } from '../../../../../../frontend/sugarprompt/editor/src/prepared-edits.ts';
import {
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import { prepareDocumentValueEdit } from '../../../../../../frontend/sugarprompt/editor/src/document-value-edit-issuance.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  currentNativeDocumentValue,
  nativeDocumentValueAt,
  prepareNativeDocumentValues,
} from '../../../../../../frontend/sugarprompt/editor/src/document-values.ts';
import {
  valueState,
  valueContext,
  valueTarget,
  decodedEdit,
  acceptedEdit,
  editedState,
} from './document-value-fixture.ts';

test('quoted scalar/escape/CRLF inverse gaps and complete container padding have literal logical coordinates', () => {
  const { state } = valueState('h\r\n" a""😀\r\nb\ud800 " \t');
  const context = valueContext(state),
    entry = context.values.entries[1]!;
  assert.equal(readExactSource(state) === 'h\r\n" a""😀\r\nb\ud800 " \t', true);
  assert.deepEqual(entry.logicalStorage, { from: 2, to: 16 });
  assert.deepEqual(entry.semanticMapping!.decodedToLogical, [4, 5, 7, null, 9, null, 10, 11, 12]);
  assert.deepEqual(entry.semanticMapping!.logicalToDecoded, [0, 1, null, 2, null, 4, 6, 7, 8]);
  assert.deepEqual(entry.fullMapping!.decodedToLogical, [
    3,
    4,
    5,
    7,
    null,
    9,
    null,
    10,
    11,
    12,
    13,
  ]);
  for (const position of [6, 8])
    assert.equal(
      nativeDocumentValueAt(state, context, { from: position, to: position }).status,
      'rejected',
    );
  assert.equal(nativeDocumentValueAt(state, context, { from: 0, to: 0 }).status, 'rejected');
  assert.equal(nativeDocumentValueAt(state, context, { from: 3, to: 3 }).status, 'rejected');
  const target = valueTarget(state, context, 4);
  const edit = acceptedEdit(state, decodedEdit(context, target, 0, 1, 'z'));
  assert.equal(readExactSource(editedState(edit)) === 'h\r\n" z""😀\r\nb\ud800 " \t', true);
});

test('all physical TXT endings, blank destinations, trim and empty EOF anchors map without joined grammar', () => {
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
    const { state } = valueState(` {a} ${ending}\t${ending}`, 'wildcard-txt');
    const context = valueContext(state);
    assert.deepEqual(
      context.values.scopes.scopes.map((scope) => scope.mapping.text.length),
      [3, 0, 0],
    );
    assert.deepEqual(
      context.values.entries.map((entry) => entry.semanticMapping!.decodedToLogical),
      [[1, 2, 3, 4], [7], [8]],
    );
    const empty = valueTarget(state, context, 8);
    const edit = acceptedEdit(state, decodedEdit(context, empty, 0, 0, 'x'));
    assert.equal(readExactSource(editedState(edit)) === ` {a} ${ending}\t${ending}x`, true);
  }
});

test('headers/quote storage/padding/cross-cell/malformed absence pair with valid fresh decoded edits', () => {
  const { state } = valueState('h,h\n" a ",b');
  const context = valueContext(state);
  for (const [from, to, reason] of [
    [0, 1, 'header'],
    [4, 4, 'unmapped'],
    [5, 5, 'padding'],
    [6, 10, 'cross-value'],
  ] as const) {
    assert.deepEqual(nativeDocumentValueAt(state, context, { from, to }), {
      status: 'rejected',
      reason,
    });
    assert.equal(
      acceptedEdit(state, decodedEdit(context, valueTarget(state, context, 6), 0, 1, 'q'))
        .transactions.length,
      1,
    );
  }
  const broken = valueState('h\n"bad').state;
  const malformed = valueContext(broken);
  assert.deepEqual(malformed.values.scopes.scopes, []);
  assert.deepEqual(nativeDocumentValueAt(broken, malformed, { from: 3, to: 3 }), {
    status: 'rejected',
    reason: 'malformed',
  });
  const repaired = broken.update(exactReplacement(broken, 2, 6, 'good')).state;
  const valid = valueContext(repaired);
  assert.equal(
    acceptedEdit(repaired, decodedEdit(valid, valueTarget(repaired, valid, 2), 0, 4, 'q'))
      .transactions.length,
    1,
  );
});

test('selection-only preparation reuses populated maps/scopes but old command authority rejects', () => {
  const { state } = valueState('h\na');
  const context = valueContext(state),
    target = valueTarget(state, context, 2);
  const selected = state.update({
    selection: EditorSelection.single(3, 2),
  }).state;
  const reused = prepareNativeDocumentValues(
    selected,
    context.values.scopes,
    { style: 'curly' },
    context.values,
  );
  assert.equal(reused, context.values);
  assert.equal(reused.entries[1]!.fullMapping, context.values.entries[1]!.fullMapping);
  assert.deepEqual(currentNativeDocumentValue(selected, { ...context, values: reused }, target), {
    status: 'rejected',
    reason: 'stale-context',
  });
  const fresh = valueTarget(selected, { ...context, values: reused }, 2);
  assert.equal(
    acceptedEdit(selected, decodedEdit({ ...context, values: reused }, fresh, 0, 1, 'b'))
      .transactions.length,
    1,
  );
});

test('source/context/kind/foreign/profile/disabled/permission/composition refusals preserve authority and allow genuine followups', () => {
  const { state, kindSlot, permission, profile } = valueState('h\na');
  const context = valueContext(state),
    target = valueTarget(state, context, 2);
  const changed = state.update(exactReplacement(state, 2, 3, 'b')).state;
  const states = [
    changed,
    state.update({ effects: invalidateCommandContext.of(null) }).state,
    state.update({
      effects: profile.reconfigure(
        featureProfile.of(Object.freeze({ ...state.facet(featureProfile) })),
      ),
    }).state,
    state.update({
      effects: kindSlot.reconfigure(documentKind.of('wildcard-txt')),
    }).state,
  ];
  for (const [index, next] of states.entries()) {
    const fresh = valueContext(next);
    assert.deepEqual(currentNativeDocumentValue(next, fresh, target), {
      status: 'rejected',
      reason: index === 0 ? 'stale-source' : 'stale-context',
    });
    assert.equal(
      acceptedEdit(next, decodedEdit(fresh, valueTarget(next, fresh, 2), 0, 1, 'z')).transactions
        .length,
      1,
    );
  }
  const foreign = valueContext(state);
  assert.deepEqual(currentNativeDocumentValue(state, foreign, target), {
    status: 'rejected',
    reason: 'foreign',
  });
  assert.equal(
    acceptedEdit(state, decodedEdit(foreign, valueTarget(state, foreign, 2), 0, 1, 'b'))
      .transactions.length,
    1,
  );
  const syntax = { style: 'double-underscore' } as const;
  const profiled = valueContext(state, syntax);
  assert.deepEqual(currentNativeDocumentValue(state, profiled, target), {
    status: 'rejected',
    reason: 'foreign',
  });
  assert.equal(
    acceptedEdit(state, decodedEdit(profiled, valueTarget(state, profiled, 2), 0, 1, 'b'))
      .transactions.length,
    1,
  );
  for (const reason of ['disabled', 'composing'] as const) {
    const unavailable = {
      ...context,
      enabled: reason !== 'disabled',
      composing: reason === 'composing',
    };
    assert.deepEqual(currentNativeDocumentValue(state, unavailable, target), {
      status: 'rejected',
      reason,
    });
    assert.equal(
      acceptedEdit(state, decodedEdit(context, valueTarget(state, context, 2), 0, 1, 'b'))
        .transactions.length,
      1,
    );
  }
  const locked = state.update({
    effects: permission.reconfigure(EditorState.readOnly.of(true)),
  }).state;
  assert.deepEqual(currentNativeDocumentValue(locked, valueContext(locked), target), {
    status: 'rejected',
    reason: 'read-only',
  });
  const unlocked = locked.update({ effects: permission.reconfigure([]) }).state;
  const ready = valueContext(unlocked);
  assert.equal(
    acceptedEdit(unlocked, decodedEdit(ready, valueTarget(unlocked, ready, 2), 0, 1, 'b'))
      .transactions.length,
    1,
  );
  const invalidValues = prepareDocumentValues('other', 'wildcard-csv');
  assert.throws(
    () =>
      prepareNativeDocumentValues(
        state,
        prepareDocumentPromptScopes(invalidValues, { style: 'curly' }),
        { style: 'curly' },
      ),
    /source-mismatch/u,
  );
  assert.throws(
    () => prepareNativeDocumentValues(state, context.values.scopes, syntax),
    /profile-mismatch/u,
  );
  assert.equal(state.update({ effects: StateEffect.appendConfig.of([]) }).state.doc, state.doc);
});

test('the existing public raw edit owner keeps exact CSV storage and CRLF units independently of decoded admission', () => {
  const { state } = valueState('h\r\n"a",b'),
    raw = new PreparedSourceEdits();
  try {
    assert.throws(() => raw.prepare(state, false, 2, 2, 'x'), RangeError);
    const prepared = raw.prepare(state, false, 0, 1, '"q"');
    assert.equal(prepared.status, 'ready');
    if (prepared.status !== 'ready') return;
    const taken = raw.take(state, false, prepared.command);
    assert.equal(taken.status, 'ready');
    if (taken.status !== 'ready') return;
    const next = state.update(taken.transaction).state;
    assert.equal(readExactSource(next) === '"q"\r\n"a",b', true);
    const context = valueContext(next);
    assert.equal(nativeDocumentValueAt(next, context, { from: 1, to: 1 }).status, 'rejected');
    assert.equal(
      acceptedEdit(next, decodedEdit(context, valueTarget(next, context, 5), 0, 1, 'z'))
        .transactions.length,
      1,
    );
  } finally {
    raw.dispose();
  }
});

test('copied command targets cannot alter decoded authority while the genuine current target remains usable', () => {
  const { state } = valueState('h\n"{a}"');
  const context = valueContext(state),
    target = valueTarget(state, context, 3);
  for (const foreign of [{ ...target }, { ...target, decodedRange: { from: 1, to: 2 } }]) {
    assert.deepEqual(currentNativeDocumentValue(state, context, foreign), {
      status: 'rejected',
      reason: 'foreign',
    });
    assert.deepEqual(prepareDocumentValueEdit(state, decodedEdit(context, foreign, 0, 0, 'x')), {
      status: 'rejected',
      reason: 'foreign',
    });
  }
  assert.equal(acceptedEdit(state, decodedEdit(context, target, 0, 0, 'x')).transactions.length, 1);
});

test('single-width source storage reuses canonical coordinate tables without losing quoted scalar gaps', () => {
  const { state } = valueState('h\n" a""😀\rb " \t');
  const context = valueContext(state),
    entry = context.values.entries[1]!,
    map = entry.semanticMapping!;
  assert.deepEqual(map.decodedToLogical, [4, 5, 7, null, 9, 10, 11]);
  assert.deepEqual(map.logicalToDecoded, [0, 1, null, 2, null, 4, 5, 6]);
  assert.equal(map.decodedToLogical, map.mapping.decodedToSource);
  assert.equal(map.logicalToDecoded, map.mapping.sourceToDecoded);
  assert.equal(map.logicalRange, map.mapping.sourceRange);
  assert.ok(Object.isFrozen(map.decodedToLogical));
  assert.ok(Object.isFrozen(map.logicalToDecoded));
});
