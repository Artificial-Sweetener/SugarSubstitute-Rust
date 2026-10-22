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
import { continuationFixture, rightVariantFixture } from './document-value-continuation-fixture.ts';
import {
  prepareNativeEditContinuation,
  followNativeEditContinuation,
  nativeEditContinuationTransaction,
  prepareNativeCompletionContinuation,
  followNativeCompletionContinuation,
  completionContinuationTransaction,
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

test('native type/delete chain consumes genuine prefix and retains populated original output', () => {
  const { state, scope, result, edit, effect } = continuationFixture();
  const prepared = prepareNativeCompletionContinuation(state, scope, result);
  assert.equal(prepared.status, 'ready');
  if (prepared.status !== 'ready') return;
  const first = state.update({
    changes: { from: 6, insert: 'd' },
    selection: { anchor: 7 },
    userEvent: 'input.type',
  });
  const followed = followNativeCompletionContinuation(prepared.value, first);
  assert.equal(followed.status, 'ready');
  if (followed.status !== 'ready') return;
  const second = first.state.update({
    changes: { from: 6, to: 7 },
    selection: { anchor: 6 },
    userEvent: 'delete.backward',
  });
  const final = followNativeCompletionContinuation(followed.value.continuation, second);
  assert.equal(final.status, 'ready');
  if (final.status !== 'ready') return;
  const accepted = completionContinuationTransaction(
    second.state,
    final.value.continuation,
    result,
    0,
    'pointer',
  );
  assert.equal(accepted.status, 'ready');
  if (accepted.status !== 'ready') return;
  assert.ok(accepted.value.newDoc.eq(edit.transactions[0]!.newDoc));
  assert.equal(accepted.value.effects.at(-1), effect);
  assert.equal(readExactSource(accepted.value.state), '🙂\r\nhands\rtail');
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

test('syntax boundaries and structured source input retire executable completion', () => {
  for (const kind of ['prompt', 'wildcard-csv'] as const) {
    const { state, scope, result } = continuationFixture(kind);
    const prepared = prepareNativeCompletionContinuation(state, scope, result);
    assert.equal(prepared.status, 'ready');
    if (prepared.status !== 'ready') continue;
    const caret = state.selection.main.head;
    const transaction = state.update({
      changes: { from: caret, insert: kind === 'prompt' ? ',' : 'd' },
      selection: { anchor: caret + 1 },
      userEvent: 'input.type',
    });
    assert.equal(followNativeCompletionContinuation(prepared.value, transaction).status, 'retired');
  }
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

test('mutated result membership refuses restored observations and fabricated handles', () => {
  const { state, scope, result } = continuationFixture();
  const prepared = prepareNativeCompletionContinuation(state, scope, result);
  assert.equal(prepared.status, 'ready');
  if (prepared.status !== 'ready') return;
  const options = result.options as (typeof result.options)[number][];
  const original = options.pop()!;
  assert.equal(
    completionContinuationTransaction(state, prepared.value, result, 0, 'keyboard').status,
    'rejected',
  );
  options.push(original);
  assert.equal(
    completionContinuationTransaction(state, prepared.value, result, 0, 'keyboard').status,
    'rejected',
  );
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

test('same-source structured metadata accepts genuine output while unobserved tips refuse', () => {
  const { state, scope, result, edit } = continuationFixture('wildcard-csv');
  const prepared = prepareNativeCompletionContinuation(state, scope, result);
  assert.equal(prepared.status, 'ready');
  if (prepared.status !== 'ready') return;
  const installed = state.update({ effects: installCompletion.of(result) });
  assert.equal(
    completionContinuationTransaction(installed.state, prepared.value, result, 0, 'keyboard')
      .status,
    'rejected',
  );
  const followed = followNativeCompletionContinuation(prepared.value, installed);
  assert.equal(followed.status, 'ready');
  if (followed.status !== 'ready') return;
  const accepted = completionContinuationTransaction(
    installed.state,
    followed.value.continuation,
    result,
    0,
    'keyboard',
  );
  assert.equal(accepted.status, 'ready');
  if (accepted.status !== 'ready') return;
  assert.equal(readExactSource(accepted.value.state), readExactSource(edit.transactions[0]!.state));
  assert.equal(accepted.value.newDoc.sliceString(0, 15), state.doc.sliceString(0, 15));
});

test('consumed right matching chooses its own genuine edit rather than the selected or detached variant', () => {
  const { state, result, left, right, scope } = rightVariantFixture();
  assert.ok(!left.transactions[0]!.newDoc.eq(right.transactions[0]!.newDoc));
  const prepared = prepareNativeCompletionContinuation(state, scope, result);
  assert.equal(prepared.status, 'ready');
  if (prepared.status !== 'ready') return;
  for (const [transaction, expected] of [
    [
      state.update({
        changes: { from: 6, insert: 'd' },
        selection: { anchor: 7 },
        userEvent: 'input.type',
      }),
      left,
    ],
    [
      state.update({
        changes: { from: 5, to: 6 },
        selection: { anchor: 5 },
        userEvent: 'delete.backward',
      }),
      right,
    ],
  ] as const) {
    const followed = followNativeCompletionContinuation(prepared.value, transaction);
    assert.equal(followed.status, 'ready');
    if (followed.status !== 'ready') continue;
    const accepted = completionContinuationTransaction(
      transaction.state,
      followed.value.continuation,
      result,
      0,
      'keyboard',
    );
    assert.equal(accepted.status, 'ready');
    if (accepted.status !== 'ready') continue;
    assert.ok(accepted.value.newDoc.eq(expected.transactions[0]!.newDoc));
    assert.equal(accepted.value.effects.at(-1), expected.transactions[0]!.effects.at(-1));
    assert.ok(accepted.value.newSelection.eq(expected.transactions[0]!.newSelection, true));
  }
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
