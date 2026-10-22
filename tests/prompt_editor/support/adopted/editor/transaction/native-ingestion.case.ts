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
/** Genuine native transaction chains prove finite publication and stale rejection without browser-input claims. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, Transaction, StateEffect } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { undo, redo, undoDepth } from '@codemirror/commands';
import { structureState } from './document-structure-fixture.ts';
import {
  authorshipExtension,
  installAuthorship,
} from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import { preparedAuthorship } from '../../../../../../frontend/sugarprompt/editor/src/authorship-ranges.ts';
import {
  enteredTextInsertion,
  prepareNativeIngestionState,
  type PreparedNativeIngestion,
} from '../../../../../../frontend/sugarprompt/editor/src/ingestion.ts';
import { dispatchNativeIngestion } from '../../../../../../frontend/sugarprompt/editor/src/native-ingestion.ts';
import {
  readExactSource,
  exactReplacement,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  initializeNativeStructure,
  prepareNativeStructure,
  installSemanticStructure,
  semanticStructures,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { richProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';

/** An unmounted nominal publication boundary delegates every state transition to an actual CodeMirror transaction. */
function ingestionFixture(source: string, caret = 0) {
  const initial = structureState(source, 'prompt', authorshipExtension);
  let state = initial.state.update({ selection: { anchor: caret } }).state;
  const transactions: Transaction[] = [];
  let observer: ((transaction: Transaction) => void) | null = null;
  // Exact external test boundary; there is no browser, input surface or competing text buffer.
  const boundary: unknown = Object.create(EditorView.prototype);
  if (!(boundary instanceof EditorView)) throw new Error('ingestion-fixture-view');
  Object.defineProperties(boundary, {
    state: { get: () => state },
    compositionStarted: { value: false, writable: true },
    dispatch: {
      value: (transaction: Transaction) => {
        assert.ok(transaction instanceof Transaction);
        assert.equal(transaction.startState, state);
        state = transaction.state;
        transactions.push(transaction);
        observer?.(transaction);
      },
    },
  });
  return {
    view: boundary,
    transactions,
    permission: initial.permission,
    /** Observe the native tip retained by this finite boundary. */
    state: () => state,
    /** Model an independent actual native transaction during a synchronous observer callback. */
    change(next: Transaction) {
      assert.equal(next.startState, state);
      state = next.state;
    },
    /** Install one synchronous external observer; it owns no editable state. */
    observe(next: (transaction: Transaction) => void) {
      observer = next;
    },
  };
}

test('ordinary insertion publishes the actual metadata and command identities with one native undo event', () => {
  const f = ingestionFixture('tail\r\nrest');
  const before = f.state();
  const plan = enteredTextInsertion(before, 0, 0, '(cat) ');
  assert.equal(plan.transactions.length, 2);
  assert.equal(plan.transactions[1], plan.edit?.transactions[0]);
  assert.equal(dispatchNativeIngestion(f.view, plan), true);
  assert.deepEqual(f.transactions, plan.transactions);
  assert.equal(readExactSource(f.state()), '(cat:1.10) tail\r\nrest');
  assert.equal(undoDepth(f.state()), 1);
  assert.equal(undo(f.view), true);
  assert.equal(readExactSource(f.state()), 'tail\r\nrest');
  assert.equal(f.state().selection.main.head, before.selection.main.head);
  assert.equal(redo(f.view), true);
  assert.equal(readExactSource(f.state()), '(cat:1.10) tail\r\nrest');
});

test('a genuine unchanged insertion publishes canonical metadata without a source history event', () => {
  const f = ingestionFixture('cat', 3);
  const plan = enteredTextInsertion(f.state(), 0, 3, 'cat');
  assert.ok(plan.edit);
  assert.equal(dispatchNativeIngestion(f.view, plan), true);
  assert.equal(readExactSource(f.state()), 'cat');
  assert.equal(f.state().selection.main.head, 3);
  assert.equal(undoDepth(f.state()), 0);
  assert.deepEqual(f.transactions, plan.transactions);
});

test('permission, composition and unrelated state changes refuse publication before the first link', () => {
  for (const reason of ['permission', 'composition', 'source'] as const) {
    const f = ingestionFixture('cat');
    const plan = enteredTextInsertion(f.state(), 0, 0, 'dog, ');
    if (reason === 'permission')
      f.change(
        f.state().update({
          effects: f.permission.reconfigure(EditorState.readOnly.of(true)),
        }),
      );
    else if (reason === 'composition')
      Object.defineProperty(f.view, 'compositionStarted', { value: true });
    else f.change(f.state().update(exactReplacement(f.state(), 0, 3, 'bird')));
    const source = readExactSource(f.state());
    assert.equal(dispatchNativeIngestion(f.view, plan), false);
    assert.equal(f.transactions.length, 0);
    assert.equal(readExactSource(f.state()) === source, true);
  }
});

test('synchronous native metadata reentry retires the remaining source command', () => {
  for (const reason of ['permission', 'composition', 'source'] as const) {
    const f = ingestionFixture('cat');
    const plan = enteredTextInsertion(f.state(), 0, 0, 'dog, ');
    f.observe(() => {
      if (reason === 'permission')
        f.change(
          f.state().update({
            effects: f.permission.reconfigure(EditorState.readOnly.of(true)),
          }),
        );
      else if (reason === 'composition')
        Object.defineProperty(f.view, 'compositionStarted', { value: true });
      else f.change(f.state().update(exactReplacement(f.state(), 0, 3, 'bird')));
    });
    assert.equal(dispatchNativeIngestion(f.view, plan), false);
    assert.equal(f.transactions.length, 1);
    assert.equal(f.transactions[0], plan.transactions[0]);
    assert.equal(readExactSource(f.state()), reason === 'source' ? 'bird' : 'cat');
    assert.equal(undoDepth(f.state()), reason === 'source' ? 1 : 0);
  }
});

test('mutation during metadata publication permanently retires the original genuine effect', () => {
  const f = ingestionFixture('');
  const plan = enteredTextInsertion(f.state(), 0, 0, '(cat)');
  const effect = plan.edit?.transactions[0]?.effects.find((value) => value.is(installAuthorship));
  assert.ok(effect);
  const original = effect.value;
  f.observe(() =>
    Object.defineProperty(effect, 'value', {
      value: preparedAuthorship([], []),
    }),
  );
  assert.equal(dispatchNativeIngestion(f.view, plan), false);
  assert.equal(readExactSource(f.state()), '');
  assert.equal(undoDepth(f.state()), 0);
  Object.defineProperty(effect, 'value', { value: original });
  const remaining: PreparedNativeIngestion = {
    startState: f.state(),
    transactions: plan.edit!.transactions,
    edit: plan.edit,
  };
  assert.equal(dispatchNativeIngestion(f.view, remaining), false);
  assert.equal(f.transactions.length, 1);
});

test('invalid chains cannot publish a prefix, while ready-state preparation uses its genuine one-link command', () => {
  const f = ingestionFixture('cat');
  const plan = enteredTextInsertion(f.state(), 0, 0, 'dog, ');
  const first = plan.transactions[0]!,
    last = plan.transactions[1]!;
  const sparse = new Array<Transaction>(2);
  sparse[1] = last;
  const changed = f.state().update({ changes: { from: 0, insert: 'x' } });
  for (const transactions of [[last, first], [first, first, last], sparse, [changed, last]]) {
    assert.equal(dispatchNativeIngestion(f.view, { ...plan, transactions }), false);
    assert.equal(f.transactions.length, 0);
  }
  const initialized = initializeNativeStructure(f.state());
  f.change(f.state().update({ effects: [] }));
  // The separately initialized speculative state cannot act for the actual view.
  const speculative = enteredTextInsertion(initialized, 0, 0, 'dog, ');
  assert.equal(dispatchNativeIngestion(f.view, speculative), false);
  const current = enteredTextInsertion(f.state(), 0, 0, 'dog, ');
  assert.equal(dispatchNativeIngestion(f.view, current), true);
  for (const transaction of prepareNativeIngestionState(f.state())) f.change(transaction);
  const next = enteredTextInsertion(f.state(), 0, 0, 'bird, ');
  assert.equal(next.transactions.length, 1);
  assert.equal(dispatchNativeIngestion(f.view, next), true);
  assert.equal(f.transactions.at(-1), next.edit?.transactions[0]);
  assert.equal(readExactSource(f.state()), 'bird, dog, cat');
});

/** The actual issued command's canonical output alone supplies its trailing semantic publication. */
function seededPlan(state: EditorState): PreparedNativeIngestion {
  const prepared = enteredTextInsertion(state, 0, 0, '(cat)');
  const edit = prepared.edit;
  if (!edit) throw new Error('fixture-seeded-command-missing');
  const after = prepared.transactions.at(-1)!.state;
  const publication = after.update({
    effects: installSemanticStructure.of({
      purpose: 'native',
      prepared: prepareNativeStructure(after, edit),
    }),
    annotations: Transaction.addToHistory.of(false),
  });
  return { ...prepared, transactions: [...prepared.transactions, publication] };
}

test('authenticated pre-command-post publication retains actual populated output, native source history and pending rendering', () => {
  const f = ingestionFixture('');
  const plan = seededPlan(f.state());
  assert.equal(plan.transactions.length, 3);
  assert.equal(dispatchNativeIngestion(f.view, plan), true);
  assert.deepEqual(f.transactions, plan.transactions);
  assert.equal(readExactSource(f.state()), '(cat:1.10)');
  assert.equal(f.state().field(semanticStructures).native!.nativeValues, plan.edit!.postValues);
  assert.equal(f.state().field(semanticStructures).native!.documentSemantics, plan.edit!.semantics);
  assert.equal(
    f.state().field(semanticStructures).native!.scopes.scopes[0]!.document.emphasisSpans.length,
    1,
  );
  assert.equal(f.state().field(richProjection, false)?.sourceIdentity ?? null, null);
  assert.equal(undoDepth(f.state()), 1);
});

test('synchronous reentry at preparation and command publication retires the remaining semantic chain', () => {
  for (const boundary of [1, 2])
    for (const reason of ['selection', 'source', 'context', 'permission', 'composition'] as const) {
      const f = ingestionFixture('');
      const plan = seededPlan(f.state());
      f.observe(() => {
        if (f.transactions.length !== boundary) return;
        if (reason === 'composition')
          Object.defineProperty(f.view, 'compositionStarted', { value: true });
        else
          f.change(
            f.state().update(
              reason === 'source'
                ? exactReplacement(f.state(), 0, readExactSource(f.state()).length, 'latest')
                : reason === 'context'
                  ? { effects: invalidateCommandContext.of(null) }
                  : reason === 'permission'
                    ? {
                        effects: f.permission.reconfigure(EditorState.readOnly.of(true)),
                      }
                    : { selection: { anchor: 0 } },
            ),
          );
      });
      assert.equal(dispatchNativeIngestion(f.view, plan), false);
      assert.equal(f.transactions.length, boundary);
      assert.equal(
        readExactSource(f.state()),
        reason === 'source' ? 'latest' : boundary === 1 ? '' : '(cat:1.10)',
      );
      assert.equal(f.state().readOnly, reason === 'permission');
      assert.equal(
        f.state().field(semanticStructures).native === null,
        boundary === 2 || reason === 'source',
      );
    }
});

test('foreign, source-changing, selection-changing and arbitrary trailing metadata cannot publish a chain prefix', () => {
  const f = ingestionFixture('');
  const plan = seededPlan(f.state());
  const commandState = plan.edit!.transactions[0]!.state;
  const prepared = prepareNativeStructure(commandState, plan.edit!);
  const arbitrary = StateEffect.define<null>();
  for (const trailing of [
    commandState.update({
      effects: arbitrary.of(null),
      annotations: Transaction.addToHistory.of(false),
    }),
    commandState.update({
      effects: installSemanticStructure.of({
        purpose: 'native',
        prepared: { ...prepared },
      }),
      annotations: Transaction.addToHistory.of(false),
    }),
    commandState.update({
      effects: installSemanticStructure.of({ purpose: 'native', prepared }),
      selection: { anchor: 0 },
      annotations: Transaction.addToHistory.of(false),
    }),
    commandState.update({
      effects: installSemanticStructure.of({ purpose: 'native', prepared }),
      changes: { from: 0, insert: 'x' },
      annotations: Transaction.addToHistory.of(false),
    }),
    commandState.update({
      effects: installSemanticStructure.of({ purpose: 'native', prepared }),
    }),
  ]) {
    assert.equal(
      dispatchNativeIngestion(f.view, {
        ...plan,
        transactions: [...plan.transactions.slice(0, -1), trailing],
      }),
      false,
    );
    assert.equal(f.transactions.length, 0);
    assert.equal(readExactSource(f.state()), '');
    assert.equal(undoDepth(f.state()), 0);
  }
});

test('synchronous observer mutation of the admitted DTO array cannot substitute, remove, insert or reorder authenticated native links', () => {
  for (const boundary of [1, 2])
    for (const mutation of ['replace', 'remove', 'insert', 'reorder', 'hole'] as const) {
      const f = ingestionFixture('');
      const original = seededPlan(f.state());
      const transactions = [...original.transactions];
      const plan = { ...original, transactions };
      const authentic = [...transactions];
      let foreign: Transaction | null = null;
      f.observe(() => {
        if (f.transactions.length !== boundary) return;
        foreign = f.state().update({
          changes: { from: 0, insert: 'foreign' },
          userEvent: 'input.host',
        });
        if (mutation === 'replace') transactions[boundary] = foreign;
        else if (mutation === 'remove') transactions.splice(boundary, 1);
        else if (mutation === 'insert') transactions.splice(boundary, 0, foreign);
        else if (mutation === 'reorder')
          [transactions[boundary - 1], transactions[boundary]] = [
            transactions[boundary]!,
            transactions[boundary - 1]!,
          ];
        // eslint-disable-next-line @typescript-eslint/no-array-delete -- Deliberately supply a sparse array to prove fail-closed input validation.
        else Reflect.deleteProperty(transactions, boundary);
      });
      assert.equal(dispatchNativeIngestion(f.view, plan), false);
      assert.equal(f.transactions.length, boundary);
      assert.deepEqual(f.transactions, authentic.slice(0, boundary));
      assert.ok(!f.transactions.includes(foreign!));
      assert.equal(readExactSource(f.state()), boundary === 1 ? '' : '(cat:1.10)');
      assert.equal(undoDepth(f.state()), boundary === 1 ? 0 : 1);
      assert.equal(f.state().field(semanticStructures).native === null, boundary === 2);
    }
});
