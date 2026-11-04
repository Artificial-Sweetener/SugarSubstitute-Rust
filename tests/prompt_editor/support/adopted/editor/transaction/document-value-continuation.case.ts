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
/** Actual native transitions prove finite continuation authority and complete publication identity. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect, Transaction, EditorSelection } from '@codemirror/state';
import { undo, redo } from '@codemirror/commands';
import { continuationFixture } from './document-value-continuation-fixture.ts';
import {
  prepareNativeEditContinuation,
  followNativeEditContinuation,
  nativeEditContinuationTransaction,
} from '../../../../../../frontend/sugarprompt/editor/src/document-value-continuation.ts';
import {
  installCompletion,
  selectCompletion,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';
import {
  completionOptionTransaction,
  completionAcceptanceTransaction,
} from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-acceptance.ts';
import {
  readExactSource,
  exactReplacement,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { installAuthorship } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';

test('actual completion installation advances metadata and preserves original effect/history/output', () => {
  const { state, result, edit, effect } = continuationFixture();
  let current = state.update({ effects: installCompletion.of(result) }).state;
  current = current.update({
    effects: selectCompletion.of({ result, index: 0 }),
  }).state;
  const transaction = completionOptionTransaction(current, result, 0, 'keyboard');
  const direct = completionAcceptanceTransaction(current, result.options[0]!, 'keyboard');
  assert.ok(direct);
  assert.ok(direct.newDoc.eq(edit.transactions[0]!.newDoc));
  assert.equal(direct.effects.at(-1), effect);
  assert.ok(transaction);
  assert.equal(transaction.startState, current);
  assert.equal(transaction.effects.at(-1), effect);
  assert.equal(transaction.effects.at(-1)!.value, effect.value);
  assert.equal(transaction.annotation(Transaction.userEvent), 'input.complete');
  assert.equal(readExactSource(transaction.state), readExactSource(edit.transactions[0]!.state));
  current = transaction.state;
  /** Native history commands publish only their actual transaction. */
  const dispatch = (next: Transaction) => {
    current = next.state;
  };
  assert.ok(
    undo({
      /** Observe the current native history tip. */
      get state() {
        return current;
      },
      dispatch,
    }),
  );
  assert.equal(readExactSource(current), readExactSource(state));
  assert.ok(
    redo({
      /** Observe the current native history tip. */
      get state() {
        return current;
      },
      dispatch,
    }),
  );
  assert.equal(readExactSource(current), readExactSource(transaction.state));
  current = current.update({
    changes: { from: current.selection.main.head, insert: '!' },
    userEvent: 'input.type',
  }).state;
  assert.equal(current.doc.sliceString(3, 9), 'hands!');
});

test('typing inside the query leaves visible rows stale until fresh suggestions are published', () => {
  const { state, result } = continuationFixture();
  const installed = state.update({ effects: installCompletion.of(result) }).state;
  assert.ok(completionOptionTransaction(installed, result, 0, 'pointer'));
  const typed = installed.update({
    changes: { from: 6, insert: 'd' },
    selection: { anchor: 7 },
    userEvent: 'input.type',
  }).state;
  assert.equal(completionOptionTransaction(typed, result, 0, 'pointer'), null);
  const restored = typed.update({
    changes: { from: 6, to: 7 },
    selection: { anchor: 6 },
    userEvent: 'delete.backward',
  }).state;
  assert.equal(completionOptionTransaction(restored, result, 0, 'pointer'), null);
  assert.equal(completionAcceptanceTransaction(restored, result.options[0]!, 'pointer'), null);
});
test('menu metadata links are actual persistent native branches with exact publication', () => {
  const { state, context, edit, effect } = continuationFixture();
  const prepared = prepareNativeEditContinuation(state, context, edit);
  assert.equal(prepared.status, 'ready');
  if (prepared.status !== 'ready') return;
  const metadata = StateEffect.define<null>();
  const speculative = state.update({ selection: { anchor: 0 } });
  assert.equal(followNativeEditContinuation(prepared.value, speculative).status, 'retired');
  const open = state.update({ effects: metadata.of(null) });
  const first = followNativeEditContinuation(prepared.value, open);
  assert.equal(first.status, 'ready');
  if (first.status !== 'ready') return;
  const close = open.state.update({ effects: metadata.of(null) });
  const second = followNativeEditContinuation(first.value, close);
  assert.equal(second.status, 'ready');
  if (second.status !== 'ready') return;
  assert.equal(nativeEditContinuationTransaction(close.state, prepared.value).status, 'rejected');
  const accepted = nativeEditContinuationTransaction(close.state, second.value);
  assert.equal(accepted.status, 'ready');
  if (accepted.status !== 'ready') return;
  assert.equal(accepted.value.effects.at(-1), effect);
  assert.ok(accepted.value.newDoc.eq(edit.transactions[0]!.newDoc));
});

test('unrelated source, selection, context, authorship and equal-source restore retire metadata', () => {
  const { state, context, edit } = continuationFixture();
  const prepared = prepareNativeEditContinuation(state, context, edit);
  assert.equal(prepared.status, 'ready');
  if (prepared.status !== 'ready') return;
  for (const [index, transaction] of [
    state.update({
      changes: { from: 0, insert: 'x' },
      userEvent: 'input.type',
    }),
    state.update({ selection: EditorSelection.cursor(0, 1) }),
    state.update({ effects: invalidateCommandContext.of(null) }),
    state.update({
      effects: installAuthorship.of(continuationFixture().effect.value),
    }),
    state.update(exactReplacement(state, 0, readExactSource(state).length, readExactSource(state))),
  ].entries())
    assert.equal(
      followNativeEditContinuation(prepared.value, transaction).status,
      'retired',
      `retirement-${index}`,
    );
});

test('mutated issued effects permanently refuse while a genuine following command remains usable', () => {
  const { state, context, edit } = continuationFixture();
  const prepared = prepareNativeEditContinuation(state, context, edit);
  assert.equal(prepared.status, 'ready');
  if (prepared.status !== 'ready') return;
  const effects = edit.transactions[0]!.effects as StateEffect<unknown>[];
  const original = effects.pop()!;
  assert.equal(nativeEditContinuationTransaction(state, prepared.value).status, 'rejected');
  effects.push(original);
  assert.equal(nativeEditContinuationTransaction(state, prepared.value).status, 'rejected');
  const fresh = continuationFixture();
  assert.equal(
    prepareNativeEditContinuation(fresh.state, fresh.context, fresh.edit).status,
    'ready',
  );
});

test('copied results, out-of-range rows and fabricated handles refuse acceptance', () => {
  const { state, result } = continuationFixture();
  const installed = state.update({ effects: installCompletion.of(result) }).state;
  assert.equal(completionOptionTransaction(installed, { ...result }, 0, 'keyboard'), null);
  for (const index of [-1, NaN, 0.5, 1])
    assert.equal(completionOptionTransaction(installed, result, index, 'keyboard'), null);
  assert.equal(
    nativeEditContinuationTransaction(
      state,
      {} as Parameters<typeof nativeEditContinuationTransaction>[1],
    ).status,
    'rejected',
  );
});
test('genuine metadata no-op reuses populated native values without source history', () => {
  const { state, result, edit, effect, context } = continuationFixture('prompt', true);
  assert.ok(edit.currentValues === context.values);
  assert.ok(edit.postValues === context.values);
  assert.ok(edit.postValues.scopes.scopes[0]!.document.segments.length);
  assert.equal(edit.transactions[0]!.docChanged, false);
  const installed = state.update({
    effects: installCompletion.of(result),
  }).state;
  const accepted = completionOptionTransaction(installed, result, 0, 'keyboard');
  assert.ok(accepted);
  assert.equal(accepted.docChanged, false);
  assert.equal(accepted.effects[0], effect);
  assert.equal(accepted.annotation(Transaction.addToHistory), false);
  assert.equal(accepted.newSelection, state.selection);
  assert.equal(readExactSource(accepted.state), readExactSource(state));
  assert.equal(
    undo({
      state: accepted.state,
      /** A genuine metadata no-op must not create an undo publication. */
      dispatch() {
        throw new Error('unexpected-noop-history');
      },
    }),
    false,
  );
});

test('structured CSV acceptance issues the genuine value edit for the installed state', () => {
  const { state, result, edit } = continuationFixture('wildcard-csv');
  const installed = state.update({ effects: installCompletion.of(result) }).state;
  const accepted = completionOptionTransaction(installed, result, 0, 'keyboard');
  assert.ok(accepted);
  assert.equal(accepted.startState, installed);
  assert.equal(readExactSource(accepted.state), readExactSource(edit.transactions[0]!.state));
  assert.equal(accepted.newDoc.sliceString(0, 15), state.doc.sliceString(0, 15));
});
test('observed sparse native origin selection retires the exact receipt after restoration', () => {
  const { state, context, edit } = continuationFixture();
  const prepared = prepareNativeEditContinuation(state, context, edit);
  assert.equal(prepared.status, 'ready');
  if (prepared.status !== 'ready') return;
  const ranges = state.selection.ranges as (typeof state.selection.main)[];
  const original = ranges[0]!;
  // eslint-disable-next-line @typescript-eslint/no-array-delete -- Deliberately supply a sparse array to prove fail-closed input validation.
  delete ranges[0];
  assert.equal(nativeEditContinuationTransaction(state, prepared.value).status, 'rejected');
  ranges[0] = original;
  assert.equal(nativeEditContinuationTransaction(state, prepared.value).status, 'rejected');
  const fresh = continuationFixture();
  assert.equal(
    prepareNativeEditContinuation(fresh.state, fresh.context, fresh.edit).status,
    'ready',
  );
});
