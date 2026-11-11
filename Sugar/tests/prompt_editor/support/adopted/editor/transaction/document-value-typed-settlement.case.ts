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
/** Native input settlement timing, semantic publication and observer reentry preserve canonical history. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect, Transaction, type EditorState } from '@codemirror/state';
import { redo, undo, undoDepth } from '@codemirror/commands';
import { EditorView } from '@codemirror/view';
import {
  historyTransactions,
  promptHistory,
} from '../../../../../../frontend/sugarprompt/editor/src/prompt-history.ts';
import { normalizationJoin } from '../../../../../../frontend/sugarprompt/editor/src/input-descriptor.ts';
import { authorshipExtension } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import { TypedNormalization } from '../../../../../../frontend/sugarprompt/editor/src/typed-normalization.ts';
import { weightStateFor } from './weight-command-state.ts';
import { prepareWeightChange } from '../../../../../../frontend/sugarprompt/editor/src/weight-command.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { richProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** A nominal supported dispatch boundary observes the real input owner with exact native transaction callbacks. */
function typedOwnerFixture(source = '(cat', head = 4) {
  let state: EditorState = weightStateFor(source, head).update({
    effects: StateEffect.appendConfig.of([authorshipExtension, promptHistory]),
  }).state;
  const published: Transaction[] = [];
  const boundary: unknown = Object.create(EditorView.prototype);
  if (!(boundary instanceof EditorView)) throw new Error('fixture-typed-view');
  const view = boundary;
  let observer: ((transaction: Transaction) => void) | null = null;
  /** Native input and all publication links apply their original actual transaction. */
  const apply = (transaction: Transaction): void => {
    assert.equal(transaction.startState, state);
    const startState = state;
    state = transaction.state;
    owner.update({ startState, state, transactions: [transaction] });
  };
  Object.defineProperties(view, {
    state: { get: () => state },
    compositionStarted: { value: false, writable: true },
    composing: { value: false },
    requestMeasure: { value: () => {} },
    dispatch: {
      value: (transaction: Transaction) => {
        assert.ok(transaction instanceof Transaction);
        apply(transaction);
        published.push(transaction);
        observer?.(transaction);
      },
    },
  });
  const owner = new TypedNormalization(view, () => undefined);
  return {
    owner,
    view,
    published,
    state: () => state,
    apply,
    /** Observe the real publication boundary without changing its input semantics. */
    observe(callback: (transaction: Transaction) => void) {
      observer = callback;
    },
  };
}

for (const [source, expectedSource, expectedDepth] of [
  ['((cat))', '(cat:1.21)', 5],
  ['(((cat)))', '(cat:1.331)', 7],
] as const)
  test('actual individually settled typing preserves one native event per punctuation and one complete word', () => {
    const f = typedOwnerFixture('', 0);
    try {
      let time = 100;
      for (const key of source) {
        f.owner.keyDown({ key, code: key });
        const selection = f.state().selection.main;
        f.apply(
          f.state().update({
            changes: { from: selection.from, to: selection.to, insert: key },
            selection: { anchor: selection.from + key.length },
            userEvent: 'input.type',
            annotations: Transaction.time.of(time++),
          }),
        );
        f.owner.keyUp({ key, code: key });
        f.owner.reserveCommand().finish();
      }
      assert.equal(readExactSource(f.state()) === expectedSource, true);
      assert.equal(undoDepth(f.state()), expectedDepth);
    } finally {
      f.owner.destroy();
    }
  });

test('every native input settlement partition preserves nested normalization and native history', () => {
  for (const [source, expectedSource, expectedDepth] of [
    ['((cat))', '(cat:1.21)', 5],
    ['(((cat)))', '(cat:1.331)', 7],
  ] as const) {
    for (let mask = 0; mask < 2 ** (source.length - 1); mask++) {
      const f = typedOwnerFixture('', 0);
      try {
        for (let index = 0; index < source.length; index++) {
          const key = source[index]!;
          f.owner.keyDown({ key, code: key });
          const selection = f.state().selection.main;
          f.apply(
            f.state().update({
              changes: { from: selection.from, to: selection.to, insert: key },
              selection: { anchor: selection.from + key.length },
              userEvent: 'input.type',
              annotations: Transaction.time.of(100 + index),
            }),
          );
          f.owner.keyUp({ key, code: key });
          if (index === source.length - 1 || (mask & (1 << index)) !== 0)
            f.owner.reserveCommand().finish();
        }
        assert.equal(readExactSource(f.state()) === expectedSource, true);
        assert.equal(
          undoDepth(f.state()),
          expectedDepth,
          `length=${source.length};partition=${mask}`,
        );
      } finally {
        f.owner.destroy();
      }
    }
  }
});

test('pending prefix settlement precedes closing input and Undo restores the actual pre-key source', () => {
  const f = typedOwnerFixture('', 0);
  try {
    const prefix = '((cat)), (dog';
    // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
    for (const [index, key] of [...prefix].entries()) {
      f.owner.keyDown({ key, code: key });
      const selection = f.state().selection.main;
      f.apply(
        f.state().update({
          changes: { from: selection.from, to: selection.to, insert: key },
          selection: { anchor: selection.from + key.length },
          userEvent: 'input.type',
          annotations: Transaction.time.of(100 + index),
        }),
      );
      f.owner.keyUp({ key, code: key });
    }
    const premature = readExactSource(f.state()),
      depth = undoDepth(f.state());
    assert.equal(premature, '((cat)), (dog');
    f.owner.reserveCommand().finish();
    assert.equal(readExactSource(f.state()), '(cat:1.21), (dog');
    assert.equal(undoDepth(f.state()), depth);
    const beforeClosing = f.state();
    f.owner.keyDown({ key: ')', code: ')' });
    f.apply(
      beforeClosing.update({
        changes: { from: beforeClosing.selection.main.head, insert: ')' },
        selection: { anchor: beforeClosing.selection.main.head + 1 },
        userEvent: 'input.type',
        annotations: Transaction.time.of(100 + prefix.length),
      }),
    );
    f.owner.keyUp({ key: ')', code: ')' });
    f.owner.reserveCommand().finish();
    assert.equal(readExactSource(f.state()), '(cat:1.21), (dog:1.10)');
    assert.equal(undoDepth(f.state()), depth + 1);
    const dispatch = (transaction: Transaction) => {
      for (const accepted of historyTransactions([transaction])) f.apply(accepted);
    };
    assert.equal(undo({ state: f.state(), dispatch }), true);
    assert.equal(readExactSource(f.state()), '(cat:1.21), (dog');
    assert.notEqual(readExactSource(f.state()), premature);
    assert.equal(f.state().selection.eq(beforeClosing.selection), true);
    assert.equal(undoDepth(f.state()), depth);
    assert.equal(redo({ state: f.state(), dispatch }), true);
    assert.equal(readExactSource(f.state()), '(cat:1.21), (dog:1.10)');
    assert.equal(f.state().selection.main.head, '(cat:1.21), (dog:1.10)'.length);
    assert.equal(undoDepth(f.state()), depth + 1);
  } finally {
    f.owner.destroy();
  }
});

test('unchanged typed settlement carries annotations without inventing a native selection history event', () => {
  const f = typedOwnerFixture('', 0);
  try {
    // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
    for (const [index, key] of [...'((cat))'].entries()) {
      f.owner.keyDown({ key, code: key });
      const selection = f.state().selection.main;
      f.apply(
        f.state().update({
          changes: { from: selection.from, to: selection.to, insert: key },
          selection: { anchor: selection.from + key.length },
          userEvent: 'input.type',
          annotations: Transaction.time.of(100 + index),
        }),
      );
      f.owner.keyUp({ key, code: key });
      if (index === 2) {
        f.owner.reserveCommand().finish();
        const followup = f.published.find(
          (transaction) => transaction.annotation(normalizationJoin) === true,
        );
        assert.ok(followup);
        assert.equal(followup.docChanged, false);
        assert.equal(followup.effects.length, 0);
        assert.equal(followup.selection, undefined);
        assert.equal(followup.state.doc, followup.startState.doc);
        assert.equal(followup.state.selection, followup.startState.selection);
        assert.equal(followup.annotation(Transaction.time), 102);
        assert.equal(followup.annotation(Transaction.userEvent), 'input.type');
      }
    }
    f.owner.reserveCommand().finish();
    assert.equal(readExactSource(f.state()) === '(cat:1.21)', true);
    assert.equal(f.state().selection.main.head, 10);
    assert.equal(undoDepth(f.state()), 5);
  } finally {
    f.owner.destroy();
  }
});

test('actual typed settlement publishes seeded semantics before an immediate weight command while projection waits for measurement', () => {
  const f = typedOwnerFixture();
  try {
    f.owner.keyDown({ key: ')', code: 'Digit0' });
    f.apply(
      f.state().update({
        changes: { from: 4, insert: ')' },
        selection: { anchor: 5 },
        userEvent: 'input.type',
        annotations: Transaction.time.of(100),
      }),
    );
    assert.equal(readExactSource(f.state()), '(cat)');
    assert.equal(f.state().field(semanticStructures).native, null);
    const settlement = f.owner.reserveCommand();
    settlement.finish();
    assert.equal(f.published.length, 3);
    assert.equal(readExactSource(f.state()), '(cat:1.10)');
    assert.equal(f.state().selection.main.head, 10);
    const semantic = f.state().field(semanticStructures).native!;
    assert.equal(semantic.scopes.scopes[0]!.document.emphasisSpans.length, 1);
    assert.equal(semantic.nativeValues.scopes, semantic.scopes);
    assert.equal(semantic.documentSemantics.preparedValues, semantic.scopes.values);
    assert.equal(f.state().field(richProjection).sourceIdentity, null);
    assert.equal(undoDepth(f.state()), 1);
    const commands = prepareWeightChange(f.state(), {
      kind: 'step',
      direction: 'increase',
    });
    assert.equal(commands.length, 2);
    for (const command of commands) f.view.dispatch(command);
    assert.equal(readExactSource(f.state()), '(cat:1.15)');
    assert.equal(f.state().selection.main.head, 4);
    assert.equal(undoDepth(f.state()), 2);
    let state = f.state();
    assert.equal(
      undo({
        state,
        dispatch: (transaction) => {
          state = transaction.state;
        },
      }),
      true,
    );
    assert.equal(readExactSource(state), '(cat:1.10)');
    assert.equal(
      undo({
        state,
        dispatch: (transaction) => {
          state = transaction.state;
        },
      }),
      true,
    );
    assert.equal(readExactSource(state), '(cat');
    settlement.finish();
    assert.equal(f.published.length, 5);
  } finally {
    f.owner.destroy();
  }
});

test('actual typed observer reentry at either publication boundary preserves the latest choice and retires seeded adoption', () => {
  for (const publication of [1, 2]) {
    const f = typedOwnerFixture();
    try {
      f.apply(
        f.state().update({
          changes: { from: 4, insert: ')' },
          selection: { anchor: 5 },
          userEvent: 'input.type',
        }),
      );
      f.observe(() => {
        if (f.published.length === publication)
          f.apply(f.state().update({ selection: { anchor: 0 } }));
      });
      f.owner.reserveCommand().finish();
      assert.equal(f.published.length, publication);
      assert.equal(f.state().selection.main.head, 0);
      assert.equal(readExactSource(f.state()), publication === 1 ? '(cat)' : '(cat:1.10)');
      assert.equal(undoDepth(f.state()), 1);
      assert.equal(
        f.state().field(semanticStructures).native,
        publication === 1 ? f.published[0]!.state.field(semanticStructures).native : null,
      );
    } finally {
      f.owner.destroy();
    }
  }
});
