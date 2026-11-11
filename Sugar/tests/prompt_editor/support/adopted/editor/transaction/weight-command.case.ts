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
/** Real CodeMirror state/history proves command publication and source-bound session retirement without browser duplication. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, StateEffect, Compartment } from '@codemirror/state';
import { undo, redo, undoDepth, redoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  semanticStructures,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { richProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import {
  weightSession,
  installWeightSession,
  retireWeightSession,
} from '../../../../../../frontend/sugarprompt/editor/src/weight-session.ts';
import { prepareWeightChange } from '../../../../../../frontend/sugarprompt/editor/src/weight-command.ts';
import { weightStateFor } from './weight-command-state.ts';
import { EmphasisWidget } from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';

test('sequential source/render transactions preserve one exact native history entry and reuse prepared semantics after publication', () => {
  let state = weightStateFor('cat\r\nnext', 0, 3);
  const transactions = prepareWeightChange(state, {
    kind: 'step',
    direction: 'increase',
  });
  assert.equal(transactions.length, 2);
  assert.equal(transactions[1]!.startState, transactions[0]!.state);
  state = transactions[1]!.state;
  assert.equal(readExactSource(state), '(cat:1.05)\r\nnext');
  assert.equal(undoDepth(state), 1);
  assert.equal(
    state.field(semanticStructures).native!.sourceIdentity,
    state.field(editorFreshness).source,
  );
  assert.equal(state.field(richProjection).sourceIdentity, state.field(editorFreshness).source);
  assert.deepEqual(state.field(weightSession)!.range, { from: 1, to: 4 });
  assert.equal(prepareNativeStructure(state), state.field(semanticStructures).native);
  const next = prepareWeightChange(state, {
    kind: 'step',
    direction: 'increase',
  });
  state = next[1]!.state;
  assert.equal(undoDepth(state), 2);
  undo({
    state,
    dispatch: (transaction) => {
      state = transaction.state;
    },
  });
  assert.equal(readExactSource(state), '(cat:1.05)\r\nnext');
  assert.equal(state.field(weightSession), null);
});

test('neutral publication uses only derived widgets and an explicit layer intent; continuation preserves its parent', () => {
  let state = weightStateFor('((cat:1.05):1.10)', 2, 5);
  state = prepareWeightChange(state, {
    kind: 'step',
    direction: 'decrease',
  })[1]!.state;
  assert.equal(readExactSource(state), '(cat:1.10)');
  assert.equal(state.field(weightSession)!.neutral, true);
  assert.equal(state.field(weightSession)!.decorations.size, 2);
  const neutralChrome = state.field(weightSession)!.decorations.iter(),
    roles: (string | number)[][] = [];
  for (; neutralChrome.value; neutralChrome.next()) {
    const widget = neutralChrome.value.spec.widget;
    assert.ok(widget instanceof EmphasisWidget);
    roles.push([neutralChrome.from, neutralChrome.to, widget.part, widget.feedback]);
  }
  assert.deepEqual(roles, [
    [1, 1, 'prefix', 2],
    [4, 4, 'suffix', 2],
  ]);
  assert.equal(state.selection.main.empty, true);
  state = prepareWeightChange(state, {
    kind: 'step',
    direction: 'decrease',
  })[1]!.state;
  assert.equal(readExactSource(state), '((cat:0.95):1.10)');
  assert.equal(state.field(weightSession)!.neutral, false);
});

test("native history restores the original directional selection and the command's resulting content caret on redo", () => {
  for (const [source, anchor, head, direction, changed, caret] of [
    ['cat\r\nnext', 0, 3, 'increase', '(cat:1.05)\r\nnext', 4],
    ['cat\r\nnext', 3, 0, 'decrease', '(cat:0.95)\r\nnext', 1],
    ['(cat:1.10)', 10, 10, 'increase', '(cat:1.15)', 4],
    ['((cat:1.05):1.10)', 2, 5, 'decrease', '(cat:1.10)', 4],
    ['🙂\r\n猫', 3, 4, 'increase', '🙂\r\n(猫:1.05)', 5],
  ] as const) {
    let state = prepareWeightChange(weightStateFor(source, anchor, head), {
      kind: 'step',
      direction,
    })[1]!.state;
    assert.equal(readExactSource(state), changed);
    assert.equal(state.selection.main.head, caret);
    undo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    });
    assert.equal(readExactSource(state), source);
    assert.deepEqual([state.selection.main.anchor, state.selection.main.head], [anchor, head]);
    assert.deepEqual([undoDepth(state), redoDepth(state)], [0, 1]);
    assert.equal(state.field(weightSession), null);
    redo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    });
    assert.equal(readExactSource(state), changed);
    assert.deepEqual([state.selection.main.anchor, state.selection.main.head], [caret, caret]);
    assert.deepEqual([undoDepth(state), redoDepth(state)], [1, 0]);
    assert.equal(state.field(weightSession), null);
  }
});

test('selection, unrelated source and explicit retirement reject the old session; stale publications cannot recreate it', () => {
  let state = weightStateFor('cat', 0, 3);
  const stale = state.field(editorFreshness);
  state = prepareWeightChange(state, {
    kind: 'step',
    direction: 'increase',
  })[1]!.state;
  const active = state;
  assert.equal(active.update({ selection: { anchor: 4 } }).state.field(weightSession), null);
  assert.equal(
    active.update({ changes: { from: 4, insert: 'x' } }).state.field(weightSession),
    null,
  );
  assert.equal(
    active.update({ effects: retireWeightSession.of(null) }).state.field(weightSession),
    null,
  );
  state = active.update({
    effects: [
      retireWeightSession.of(null),
      installWeightSession.of({
        owner: 'keyboard',
        freshness: stale,
        kind: 'emphasis',
        range: { from: 1, to: 4 },
        boundary: 'end',
        neutral: false,
      }),
    ],
  }).state;
  assert.equal(state.field(weightSession), null);
});

test('read-only preparation rejects mutation and permission replacement retires continuation', () => {
  const permission = new Compartment();
  let state = weightStateFor('cat', 0, 3);
  state = state.update({
    effects: StateEffect.appendConfig.of(permission.of(EditorState.readOnly.of(false))),
  }).state;
  state = prepareWeightChange(state, {
    kind: 'step',
    direction: 'increase',
  })[1]!.state;
  state = state.update({
    effects: permission.reconfigure(EditorState.readOnly.of(true)),
  }).state;
  assert.equal(state.field(weightSession), null);
  assert.deepEqual(prepareWeightChange(state, { kind: 'step', direction: 'increase' }), []);
  assert.deepEqual(
    prepareWeightChange(weightStateFor(''), {
      kind: 'step',
      direction: 'increase',
    }),
    [],
  );
});

test('caret proximity follows the pinned code-point midpoint rather than the browser UTF-16 midpoint', () => {
  const state = prepareWeightChange(weightStateFor('(ab🙂🙂:1.20)', 3), {
    kind: 'step',
    direction: 'increase',
  })[1]!.state;
  assert.equal(readExactSource(state), '(ab🙂🙂:1.25)');
  assert.equal(state.selection.main.head, 7);
  assert.equal(state.field(weightSession)!.boundary, 'end');
});

test('pointer steps preserve content-relative caret and map outside directional selection through native source changes', () => {
  for (const [source, from, to, anchor, head, expectedAnchor, expectedHead] of [
    ['(cat:1.05), dog', 1, 4, 13, 13, 6, 6],
    ['(cat:1.05), dog', 1, 4, 2, 2, 1, 1],
    ['(cat:1.05), dog', 1, 4, 4, 1, 3, 0],
    ['🙂\r\n(cat:1.05), dog', 4, 7, 16, 16, 9, 9],
  ] as const) {
    const initial = weightStateFor(source, anchor, head);
    let state = prepareWeightChange(
      initial,
      { kind: 'step', direction: 'decrease' },
      {
        owner: 'pointer',
        freshness: initial.field(editorFreshness),
        kind: 'emphasis',
        range: { from, to },
      },
    )[1]!.state;
    assert.equal(readExactSource(state), source.replace('(cat:1.05)', 'cat'));
    assert.deepEqual(
      [state.selection.main.anchor, state.selection.main.head],
      [expectedAnchor, expectedHead],
    );
    assert.equal(state.field(weightSession)!.owner, 'pointer');
    assert.equal(state.field(weightSession)!.neutral, true);
    assert.equal(undoDepth(state), 1);
    const stale = initial.field(editorFreshness);
    assert.deepEqual(
      prepareWeightChange(
        state,
        { kind: 'step', direction: 'increase' },
        {
          owner: 'pointer',
          freshness: stale,
          kind: 'emphasis',
          range: { from, to },
        },
      ),
      [],
    );
    undo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    });
    assert.equal(readExactSource(state), source);
    assert.deepEqual([state.selection.main.anchor, state.selection.main.head], [anchor, head]);
  }
});

test('wheel publication binds neutral continuation to its original opaque token rather than a later hover', () => {
  const token = Symbol();
  const initial = weightStateFor('(cat:1.05), dog', 13);
  const neutral = prepareWeightChange(
    initial,
    { kind: 'step', direction: 'decrease' },
    {
      owner: 'wheel',
      token,
      freshness: initial.field(editorFreshness),
      kind: 'emphasis',
      range: { from: 1, to: 4 },
    },
  )[1]!.state;
  assert.equal(neutral.field(weightSession)!.token, token);
  assert.equal(readExactSource(neutral), 'cat, dog');
  const request = {
    owner: 'wheel' as const,
    token,
    freshness: neutral.field(editorFreshness),
    kind: 'emphasis' as const,
    range: { from: 0, to: 3 },
  };
  assert.deepEqual(
    prepareWeightChange(
      neutral,
      { kind: 'step', direction: 'decrease' },
      { ...request, token: Symbol() },
    ),
    [],
  );
  const continued = prepareWeightChange(
    neutral,
    { kind: 'step', direction: 'decrease' },
    request,
  )[1]!.state;
  assert.equal(readExactSource(continued), '(cat:0.95), dog');
  assert.equal(continued.field(weightSession)!.token, token);
  assert.equal(continued.selection.main.head, 13);
});

test('CSV keyboard numeric wildcard steps stay in their admitted canonical group and current continuation', () => {
  let state = weightStateFor('h,z\r\n"{one|2}","{neighbor|7}"', 6, 6, 'wildcard-csv');
  const changed = prepareWeightChange(state, {
    kind: 'step',
    direction: 'increase',
  });
  assert.equal(changed.length, 2);
  state = changed.at(-1)!.state;
  assert.equal(readExactSource(state) === 'h,z\r\n"{one|3}","{neighbor|7}"', true);
  assert.equal(state.field(weightSession)!.kind, 'wildcard');
  assert.equal(
    state.field(semanticStructures).native!.scopes.scopes[0]!.wildcardGroups[0]!.numeric,
    true,
  );
  assert.equal(prepareNativeStructure(state), state.field(semanticStructures).native);
  const continued = prepareWeightChange(state, {
    kind: 'step',
    direction: 'increase',
  });
  state = continued.at(-1)!.state;
  assert.equal(readExactSource(state) === 'h,z\r\n"{one|4}","{neighbor|7}"', true);
  assert.equal(undoDepth(state), 2);
});
