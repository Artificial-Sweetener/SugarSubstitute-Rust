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
/** Exact numeric draft acceptance owns its keyed phase, source/caret settlement and cancellation frontier. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, StateEffect, Transaction } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  editorFreshness,
  invalidateCommandContext,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  exactWeightDraft,
  setExactWeightDraft,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-weight-draft.ts';
import { control, disposeQueueFixture } from './weight-command-queue-fixture.ts';
/** Open only genuine native draft metadata; text remains absent from this state owner. */
function openDraft(
  native: ReturnType<typeof control>,
  kind: 'emphasis' | 'lora',
  from: number,
  to: number,
) {
  const key = Symbol('owned draft'),
    request = {
      owner: 'exact',
      completion: 'keyboard',
      neutral: false,
      kind,
      range: { from, to },
      freshness: native.view.state.field(editorFreshness),
    } as const;
  native.view.dispatch({
    effects: setExactWeightDraft.of({ ...request, key, phase: 'editing' }),
    annotations: Transaction.addToHistory.of(false),
  });
  return { key, request };
}

for (const specimen of [
  {
    kind: 'emphasis',
    source: '(cat:1.2), tail',
    from: 1,
    to: 4,
    expected: '(cat:111.22), tail',
    head: 12,
  },
  {
    kind: 'lora',
    source: '<lora:missing:1.2>, tail',
    from: 0,
    to: 18,
    expected: '<lora:missing:111.22>, tail',
    head: 21,
  },
] as const)
  test(`${specimen.kind} exact acceptance publishes before immediate ordinary typing without geometry reads`, async () => {
    const native = control(specimen.source);
    try {
      const { key, request } = openDraft(native, specimen.kind, specimen.from, specimen.to);
      assert.equal(
        native.owner.exact({ kind: 'set', weight: '111.22' }, request, native.handoff, key),
        true,
      );
      assert.equal(readExactSource(native.view.state), specimen.expected);
      assert.equal(native.view.state.selection.main.head, specimen.head);
      assert.equal(native.view.state.field(exactWeightDraft), null);
      assert.equal(undoDepth(native.view.state), 1);
      assert.equal(native.snapshotReads(), 0);
      assert.equal(native.restored(), 1);
      native.view.dispatch(
        native.view.state.update(native.view.state.replaceSelection('x'), {
          userEvent: 'input.type',
        }),
      );
      assert.equal(
        readExactSource(native.view.state),
        specimen.expected.slice(0, specimen.head) + 'x' + specimen.expected.slice(specimen.head),
      );
      assert.equal(undoDepth(native.view.state), 2);
      await native.flush();
      assert.equal(undoDepth(native.view.state), 2);
    } finally {
      native.dispose();
    }
  });

test('outside acceptance retains the same current native cell until measured source publication', async () => {
  const native = control('(cat:1.2), tail');
  try {
    const { key, request } = openDraft(native, 'emphasis', 1, 4);
    assert.equal(
      native.owner.exact(
        { kind: 'set', weight: '111.22' },
        { ...request, completion: 'outside' },
        native.handoff,
        key,
      ),
      true,
    );
    assert.equal(readExactSource(native.view.state), '(cat:1.2), tail');
    assert.equal(native.view.state.field(exactWeightDraft)?.key, key);
    assert.equal(native.view.state.field(exactWeightDraft)?.phase, 'awaiting-commit');
    assert.equal(undoDepth(native.view.state), 0);
    await native.flush();
    assert.equal(readExactSource(native.view.state), '(cat:111.22), tail');
    assert.equal(native.view.state.field(exactWeightDraft), null);
    assert.equal(native.restored(), 0);
  } finally {
    native.dispose();
  }
});

test('phase publication reentry retires exact acceptance without stealing newer host focus', () => {
  const native = control('(cat:1.2), tail');
  try {
    const { key, request } = openDraft(native, 'emphasis', 1, 4);
    native.reenter(() => native.supersede());
    assert.equal(
      native.owner.exact({ kind: 'set', weight: '111.22' }, request, native.handoff, key),
      true,
    );
    assert.equal(readExactSource(native.view.state), '(cat:1.2), tail');
    assert.equal(native.view.state.field(exactWeightDraft), null);
    assert.equal(native.restored(), 0);
    assert.equal(undoDepth(native.view.state), 0);
  } finally {
    native.dispose();
  }
});

test('reentrant replacement draft is neither committed through the old key nor cleared by old settlement', () => {
  const native = control('(cat:1.2), tail');
  try {
    const { key, request } = openDraft(native, 'emphasis', 1, 4),
      replacement = Symbol('replacement');
    native.reenter(() =>
      native.view.dispatch({
        effects: setExactWeightDraft.of({
          ...request,
          key: replacement,
          phase: 'editing',
        }),
        annotations: Transaction.addToHistory.of(false),
      }),
    );
    native.owner.exact({ kind: 'set', weight: '111.22' }, request, native.handoff, key);
    assert.equal(readExactSource(native.view.state), '(cat:1.2), tail');
    assert.equal(native.view.state.field(exactWeightDraft)?.key, replacement);
    assert.equal(native.restored(), 0);
  } finally {
    native.dispose();
  }
});

test('unchanged keyboard acceptance retires its cell exactly once without source history', () => {
  const native = control('(cat:1.20), tail');
  try {
    native.view.dispatch({
      selection: { anchor: 10 },
      annotations: Transaction.addToHistory.of(false),
    });
    const { key, request } = openDraft(native, 'emphasis', 1, 4);
    assert.equal(
      native.owner.exact({ kind: 'set', weight: '1.20' }, request, native.handoff, key),
      true,
    );
    assert.equal(native.view.state.field(exactWeightDraft), null);
    assert.equal(undoDepth(native.view.state), 0);
    assert.equal(native.restored(), 1);
    assert.equal(
      native.owner.exact({ kind: 'set', weight: '1.20' }, request, native.handoff, key),
      false,
    );
    assert.equal(native.restored(), 1);
  } finally {
    native.dispose();
  }
});

for (const mode of ['source', 'permissions', 'context', 'dispose'] as const)
  test(`outside exact ${mode} supersession cancels publication and settles only its captured draft`, async () => {
    const native = control('(cat:1.20), tail');
    try {
      const { key, request } = openDraft(native, 'emphasis', 1, 4);
      native.owner.exact(
        { kind: 'set', weight: '111.22' },
        { ...request, completion: 'outside' },
        native.handoff,
        key,
      );
      const startState = native.view.state;
      if (mode === 'source')
        native.view.dispatch(
          startState.update({
            changes: {
              from: 0,
              to: startState.doc.length,
              insert: 'replacement',
            },
          }),
        );
      else if (mode === 'permissions')
        native.view.dispatch({
          effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)),
        });
      else if (mode === 'context')
        native.view.dispatch({ effects: invalidateCommandContext.of(null) });
      if (mode === 'dispose') native.owner.destroy();
      else native.owner.update({ startState, state: native.view.state });
      await native.flush();
      assert.equal(
        readExactSource(native.view.state),
        mode === 'source' ? 'replacement' : '(cat:1.20), tail',
      );
      assert.equal(native.restored(), 0);
      if (mode !== 'dispose') assert.equal(native.view.state.field(exactWeightDraft), null);
    } finally {
      native.dispose();
    }
  });

test('deferred outside cancellation cannot clear a newer draft admitted before its cleanup microtask', async () => {
  const native = control('(cat:1.20), tail');
  try {
    const { key, request } = openDraft(native, 'emphasis', 1, 4);
    native.owner.exact(
      { kind: 'set', weight: '111.22' },
      { ...request, completion: 'outside' },
      native.handoff,
      key,
    );
    const startState = native.view.state;
    native.view.dispatch({ selection: { anchor: 0 } });
    native.owner.update({ startState, state: native.view.state });
    const replacement = Symbol('newer editing cell');
    native.view.dispatch({
      effects: setExactWeightDraft.of({
        ...request,
        key: replacement,
        phase: 'editing',
      }),
      annotations: Transaction.addToHistory.of(false),
    });
    await native.flush();
    assert.equal(native.view.state.field(exactWeightDraft)?.key, replacement);
    assert.equal(readExactSource(native.view.state), '(cat:1.20), tail');
    assert.equal(native.restored(), 0);
  } finally {
    native.dispose();
  }
});

for (const boundary of [1, 2])
  test(`exact publication callback failure at boundary ${boundary} remains observable and prevents future work`, async () => {
    const native = control('(cat:1.20), tail'),
      failure = new Error('controlled publication failure');
    try {
      const { key, request } = openDraft(native, 'emphasis', 1, 4);
      native.reenter(() => {
        throw failure;
      }, boundary);
      assert.throws(
        () => native.owner.exact({ kind: 'set', weight: '111.22' }, request, native.handoff, key),
        (cause) => cause === failure,
      );
      await native.flush();
      assert.equal(native.view.state.field(exactWeightDraft), null);
      assert.equal(native.restored(), 0);
      assert.equal(
        readExactSource(native.view.state),
        boundary === 1 ? '(cat:1.20), tail' : '(cat:111.22), tail',
      );
    } finally {
      native.dispose();
    }
  });

for (const completion of ['keyboard', 'outside'] as const)
  test(
    'LoRA ' +
      completion +
      ' acceptance distinguishes source publication before later permission retirement',
    async () => {
      const native = control('<lora:real:1.20>');
      try {
        const { key, request } = openDraft(native, 'lora', 0, 16);
        assert.equal(
          native.owner.exact(
            { kind: 'set', weight: '2.00' },
            { ...request, completion },
            native.handoff,
            key,
          ),
          true,
        );
        assert.equal(
          readExactSource(native.view.state),
          completion === 'keyboard' ? '<lora:real:2.00>' : '<lora:real:1.20>',
        );
        assert.equal(undoDepth(native.view.state), completion === 'keyboard' ? 1 : 0);
        assert.equal(native.snapshotReads(), 0);
        assert.equal(
          native.view.state.field(exactWeightDraft)?.phase ?? null,
          completion === 'keyboard' ? null : 'awaiting-commit',
        );
        const startState = native.view.state;
        native.view.dispatch({
          effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)),
          annotations: Transaction.addToHistory.of(false),
        });
        native.owner.update({ startState, state: native.view.state });
        await native.flush();
        assert.equal(native.view.state.readOnly, true);
        assert.equal(
          readExactSource(native.view.state),
          completion === 'keyboard' ? '<lora:real:2.00>' : '<lora:real:1.20>',
        );
        assert.equal(undoDepth(native.view.state), completion === 'keyboard' ? 1 : 0);
        assert.equal(native.view.state.field(exactWeightDraft), null);
        assert.equal(native.snapshotReads(), 0);
      } finally {
        native.dispose();
      }
    },
  );

test.after(disposeQueueFixture);
