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
/** Issued atomic rich posts extend the genuine native adapter without admitting unsigned metadata or changing pending input paint. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, StateEffect, Transaction, type EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { undoDepth } from '@codemirror/commands';
import { projectionState } from './projection-state.ts';
import { context, editAt } from './document-structure-fixture.ts';
import { nativeDocumentValueAt } from '../../../../../../frontend/sugarprompt/editor/src/document-values.ts';
import { prepareDocumentValueEdit } from '../../../../../../frontend/sugarprompt/editor/src/document-value-edit-issuance.ts';
import { authorshipExtension } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import {
  initializeNativeStructure,
  semanticStructures,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { enteredTextInsertion } from '../../../../../../frontend/sugarprompt/editor/src/ingestion.ts';
import { dispatchNativeIngestion } from '../../../../../../frontend/sugarprompt/editor/src/native-ingestion.ts';
import {
  currentNativeIngestionPost,
  prepareNativeIngestionPost,
} from '../../../../../../frontend/sugarprompt/editor/src/native-ingestion-post.ts';
import {
  currentSourceDisplayProjection,
  richProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** Prepare an actual decoded command whose native caret exposes the token interior before its post is issued. */
function interiorEdit(state: EditorState) {
  const current = context(state.field(semanticStructures).native!),
    target = nativeDocumentValueAt(state, current, { from: 0, to: 0 });
  assert.equal(target.status, 'ready');
  if (target.status !== 'ready') throw new Error('fixture-interior-target');
  const result = prepareDocumentValueEdit(state, {
    context: current,
    target: target.target,
    range: { from: 0, to: 0 },
    text: '<lora:real:0.70>',
    selection: {
      ranges: [{ kind: 'decoded', anchor: 1, head: 1 }],
      mainIndex: 0,
    },
  });
  if (result.status === 'rejected') throw new Error('fixture-interior-edit');
  assert.equal(result.status, 'ready');
  return result.edit;
}

/** Actual CodeMirror states/transactions own source and history; the nominal view transports only synchronous dispatch and reentry. */
function postFixture(raw = false, interior = false) {
  const presentation = new Compartment();
  let state = initializeNativeStructure(
    projectionState('', presentation).update({
      effects: StateEffect.appendConfig.of(authorshipExtension),
    }).state,
  );
  if (raw)
    state = state.update({
      effects: presentation.reconfigure(richRendering.of(false)),
    }).state;
  const rawEdit = interior
      ? interiorEdit(state)
      : raw
        ? editAt(state, state.field(semanticStructures).native!, 0, '<lora:real:0.70>')
        : null,
    plan = rawEdit
      ? { startState: state, transactions: rawEdit.transactions, edit: rawEdit }
      : enteredTextInsertion(state, 0, 0, '<lora:real:0.70>'),
    edit = plan.edit;
  assert.ok(edit);
  const command = edit.transactions[0];
  assert.ok(command);
  const post = prepareNativeIngestionPost(command.state, edit),
    dispatched: Transaction[] = [];
  let observer: (() => void) | null = null;
  const view = Object.create(EditorView.prototype) as EditorView;
  Object.defineProperties(view, {
    state: { get: () => state },
    compositionStarted: { value: false, writable: true },
    dispatch: {
      /** Preserve the actual command/state identities at each native publication frontier. */
      value(transaction: Transaction) {
        assert.equal(transaction.startState, state);
        state = transaction.state;
        dispatched.push(transaction);
        observer?.();
      },
    },
  });
  return {
    view,
    plan,
    post,
    edit,
    dispatched,
    /** The finite external observer may mutate an actual source state, never a shadow document. */
    observe(callback: () => void): void {
      observer = callback;
    },
    /** An independent native transaction invalidates the remaining captured chain. */
    change(transaction: Transaction): void {
      assert.equal(transaction.startState, state);
      state = transaction.state;
    },
    /** Publish the exact prepared source command followed by the chosen real post. */
    publish(next = post): boolean {
      return dispatchNativeIngestion(view, {
        ...plan,
        transactions: [...plan.transactions, next],
      });
    },
  };
}

test('an issued canonical post preserves exact command/effect identities and current rich source paint', () => {
  const f = postFixture();
  assert.equal(f.post.effects.length, 2);
  assert.equal(currentNativeIngestionPost(f.post, f.edit), true);
  assert.equal(f.publish(), true);
  assert.deepEqual(f.dispatched, [...f.plan.transactions, f.post]);
  assert.equal(readExactSource(f.view.state), '<lora:real:0.70>');
  assert.ok(currentSourceDisplayProjection(f.view.state));
  assert.equal(undoDepth(f.view.state), 1);
});

test('raw presentation with the real rich field keeps the supported sole-semantic post', () => {
  const f = postFixture(true);
  assert.ok(f.view.state.field(richProjection));
  assert.equal(f.post.effects.length, 1);
  assert.equal(f.publish(), true);
  assert.equal(f.view.state.field(richProjection).decorations.size, 0);
  assert.equal(readExactSource(f.view.state), '<lora:real:0.70>');
  assert.equal(undoDepth(f.view.state), 1);
});

test('ordinary ingestion with a rich field deliberately retains pending rendering', () => {
  const f = postFixture();
  assert.equal(dispatchNativeIngestion(f.view, f.plan), true);
  assert.equal(f.view.state.field(richProjection).sourceIdentity, null);
  assert.equal(undoDepth(f.view.state), 1);
});

for (const variation of ['copied', 'reversed', 'extra', 'selection'] as const)
  test(`unsigned ${variation} post refuses the whole chain before its source prefix`, () => {
    const f = postFixture(),
      original = f.view.state,
      arbitrary = StateEffect.define<null>(),
      effects =
        variation === 'reversed'
          ? [...f.post.effects].reverse()
          : variation === 'extra'
            ? [...f.post.effects, arbitrary.of(null)]
            : f.post.effects,
      post = f.post.startState.update({
        effects,
        ...(variation === 'selection' ? { selection: { anchor: 0 } } : {}),
        annotations: Transaction.addToHistory.of(false),
      });
    assert.equal(f.publish(post), false);
    assert.equal(f.dispatched.length, 0);
    assert.equal(f.view.state, original);
    assert.equal(undoDepth(f.view.state), 0);
  });

test('an issued post cannot transfer to a different edit', () => {
  const first = postFixture(),
    second = postFixture();
  assert.equal(currentNativeIngestionPost(first.post, second.edit), false);
});

test('mutating an issued canonical artifact cannot authorize its source prefix', () => {
  const f = postFixture(),
    value = f.post.effects[1]!.value as { all: unknown },
    original = value.all;
  try {
    Reflect.set(value, 'all', f.post.startState.field(richProjection).all);
    assert.equal(f.publish(), false);
    assert.equal(f.dispatched.length, 0);
    assert.equal(undoDepth(f.view.state), 0);
  } finally {
    Reflect.set(value, 'all', original);
  }
  const exposure = postFixture(false, true),
    exposed = exposure.post,
    payload = exposed.effects[1]!.value as { all: unknown },
    originalAll = payload.all,
    painted = exposed.state.field(richProjection);
  assert.notEqual(painted, payload);
  assert.equal(painted.expanded.length, 1);
  assert.equal(currentNativeIngestionPost(exposed, exposure.edit), true);
  try {
    Reflect.set(payload, 'all', exposed.startState.field(richProjection).all);
    assert.equal(painted.all, originalAll);
    assert.equal(currentNativeIngestionPost(exposed, exposure.edit), false);
    assert.equal(exposure.publish(), false);
    assert.equal(exposure.dispatched.length, 0);
    assert.equal(undoDepth(exposure.view.state), 0);
  } finally {
    Reflect.set(payload, 'all', originalAll);
  }
});

test('native command reentry retires an otherwise issued canonical post', () => {
  const f = postFixture();
  f.observe(() => {
    if (f.dispatched.length === f.plan.transactions.length)
      f.change(f.view.state.update({ selection: { anchor: 0 } }));
  });
  assert.equal(f.publish(), false);
  assert.equal(f.dispatched.length, f.plan.transactions.length);
  assert.equal(f.view.state.field(richProjection).sourceIdentity, null);
  assert.equal(undoDepth(f.view.state), 1);
});
