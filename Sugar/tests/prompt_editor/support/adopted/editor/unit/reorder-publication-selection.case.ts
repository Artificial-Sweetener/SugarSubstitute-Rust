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
/** Supported publication/focus sequencing preserves current CM selection; native proof owns actual browser repair causality. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  EditorSelection,
  EditorState,
  StateEffect,
  Transaction,
  type TransactionSpec,
} from '@codemirror/state';
import type { ViewUpdate } from '@codemirror/view';
import { undoDepth } from '@codemirror/commands';
import {
  installReorderPreview,
  reorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  selectionContents,
  selectionMatches,
} from '../../../../../../frontend/sugarprompt/editor/src/native-selection.ts';
import { fixture, key } from './reorder-session-fixture.ts';

/** Drive the original finite entry request and its supported deferred publication. */
function enter(f: ReturnType<typeof fixture>): void {
  assert.equal(f.owner.keydown(key('Alt')), true);
  f.measure();
  f.task();
}

test('native paint focus follows source-neutral publication and preserves every current directional selection value', () => {
  const f = fixture();
  try {
    f.view.dispatch({
      effects: StateEffect.appendConfig.of(EditorState.allowMultipleSelections.of(true)),
    });
    f.view.dispatch({
      selection: EditorSelection.create(
        [EditorSelection.cursor(0, -1, 2, NaN), EditorSelection.range(3, 4, 17, 1)],
        1,
      ),
    });
    const before = f.state,
      contents = selectionContents(before.selection),
      calls: boolean[] = [];
    f.view.focus = () => {
      const published = f.state.field(reorderPreview).value !== null;
      calls.push(published);
      assert.equal(f.state.doc, before.doc);
      assert.equal(selectionMatches(f.state.selection, contents), true);
    };
    f.observe((tx) => {
      if (tx.effects.some((effect) => effect.is(installReorderPreview) && effect.value !== null)) {
        assert.equal(tx.selection, undefined);
        assert.equal(tx.docChanged, false);
        assert.equal(tx.annotation(Transaction.addToHistory), false);
      }
    });
    enter(f);
    assert.deepEqual(calls, [false, true]);
    assert.equal(f.state.field(reorderPreview).native, true);
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

for (const boundary of ['before', 'after'] as const)
  test(`foreign selection ${boundary} publication remains authoritative and retires the gesture without focus restoration`, () => {
    const f = fixture();
    let focuses = 0,
      changed = false;
    try {
      f.view.focus = () => {
        focuses++;
      };
      const foreign = EditorSelection.create([EditorSelection.cursor(1, 1, 2, 19)]);
      if (boundary === 'after')
        f.observe((tx) => {
          if (
            !changed &&
            tx.effects.some((effect) => effect.is(installReorderPreview) && effect.value !== null)
          ) {
            changed = true;
            f.view.dispatch({ selection: foreign, userEvent: 'select' });
          }
        });
      else {
        const dispatch = f.view.dispatch.bind(f.view);
        f.view.dispatch = (
          ...inputs: readonly (Transaction | readonly Transaction[] | TransactionSpec)[]
        ) => {
          const input = inputs[0];
          const transaction =
            input instanceof Transaction
              ? input
              : Array.isArray(input)
                ? (input[0] as Transaction | undefined)
                : undefined;
          const effects = transaction
            ? transaction.effects
            : (input as TransactionSpec | undefined)?.effects;
          const published =
            effects &&
            (Array.isArray(effects) ? effects : [effects]).some(
              (effect) => effect.is(installReorderPreview) && effect.value !== null,
            );
          if (!changed && published) {
            changed = true;
            dispatch({ selection: foreign, userEvent: 'select' });
          }
          if (input instanceof Transaction) dispatch(input);
          else if (Array.isArray(input)) dispatch(input as readonly Transaction[]);
          else dispatch(...(inputs as readonly TransactionSpec[]));
        };
      }
      enter(f);
      assert.equal(changed, true);
      assert.equal(focuses, 1);
      assert.equal(selectionMatches(f.state.selection, selectionContents(foreign)), true);
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(f.owner.blocksInput(), false);
      assert.equal(readExactSource(f.state) === 'a, b, c', true);
      assert.equal(undoDepth(f.state), 0);
      while (f.tasks.length) f.task();
      assert.equal(f.state.selection.main.head, 1);
    } finally {
      f.destroy();
    }
  });

test('foreign focus after publication is never retaken and the following native focus update retires ownership', () => {
  const f = fixture();
  let focuses = 0;
  try {
    f.view.focus = () => {
      focuses++;
    };
    f.observe((tx) => {
      if (tx.effects.some((effect) => effect.is(installReorderPreview) && effect.value !== null))
        f.setFocused(false);
    });
    enter(f);
    assert.equal(focuses, 1);
    assert.equal(f.view.hasFocus, false);
    f.owner.update({ state: f.state, focusChanged: true } as ViewUpdate);
    assert.equal(f.owner.blocksInput(), false);
    while (f.tasks.length) f.task();
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(f.state.selection.main.head, 0);
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('foreign selection during public focus remains current and retires the publication', () => {
  const f = fixture();
  let focuses = 0;
  const foreign = EditorSelection.create([EditorSelection.cursor(2, -1, 1, 23)]);
  try {
    f.view.focus = () => {
      if (++focuses === 2) f.view.dispatch({ selection: foreign, userEvent: 'select' });
    };
    enter(f);
    assert.equal(focuses, 2);
    assert.equal(selectionMatches(f.state.selection, selectionContents(foreign)), true);
    assert.equal(f.owner.blocksInput(), false);
    while (f.tasks.length) f.task();
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('unexpected post-publication native focus failure remains observable and retires every finite owner', () => {
  const f = fixture(),
    cause = new Error('native-publication-focus-failed');
  let focuses = 0;
  try {
    f.view.focus = () => {
      if (++focuses === 2) throw cause;
    };
    f.owner.keydown(key('Alt'));
    f.measure();
    assert.throws(
      () => f.task(),
      (error) => error === cause,
    );
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(f.frames.size, 0);
    assert.equal(f.captures.size, 0);
    assert.equal(readExactSource(f.state) === 'a, b, c', true);
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});
