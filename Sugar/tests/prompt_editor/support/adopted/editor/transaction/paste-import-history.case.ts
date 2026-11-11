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
/** Actual native transaction identity and finite metadata continuation characterize the dedicated paste history capability. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, StateEffect, Text, Transaction } from '@codemirror/state';
import { isolateHistory } from '@codemirror/commands';
import { structureState } from './document-structure-fixture.ts';
import { authorshipExtension } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { enteredTextInsertion } from '../../../../../../frontend/sugarprompt/editor/src/ingestion.ts';
import {
  preparePasteImportHistory,
  capturePasteImportHistory,
  acceptPasteImportHistory,
  followPasteImportHistory,
  currentPasteImportHistory,
  retirePasteImportHistory,
  pasteImportAnnotations,
  pasteImportContinuation,
  joinsPasteImportHistory,
  pasteImportUserEvent,
} from '../../../../../../frontend/sugarprompt/editor/src/paste-import-history.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
/** Prepare one genuine raw native command through its sole ingestion owner. */
function accepted() {
  const state = structureState('', 'prompt', [authorshipExtension, richRendering.of(false)]).state,
    handle = preparePasteImportHistory(state);
  const plan = enteredTextInsertion(
    state,
    0,
    0,
    'https://danbooru.donmai.us/posts/123',
    undefined,
    { handle, phase: 'initial' },
  );
  const transaction = plan.transactions[0]!;
  assert.equal(capturePasteImportHistory(handle, transaction), true);
  assert.equal(acceptPasteImportHistory(handle, transaction, transaction.state), true);
  return { state: transaction.state, handle, transaction };
}
test('actual two neutral native links preserve the exact accepted history tip and disposal releases it', () => {
  const f = accepted();
  const first = f.state.update({
    annotations: Transaction.addToHistory.of(false),
  });
  assert.equal(followPasteImportHistory(f.handle, first), true);
  const second = first.state.update({
    annotations: Transaction.addToHistory.of(false),
  });
  assert.equal(followPasteImportHistory(f.handle, second), true);
  assert.equal(currentPasteImportHistory(f.handle, second.state), true);
  assert.equal(currentPasteImportHistory(f.handle, f.state), false);
  retirePasteImportHistory(f.handle);
  assert.equal(currentPasteImportHistory(f.handle, second.state), false);
});
test('branched, navigation, context and source links permanently retire the native receipt', () => {
  for (const mode of ['branch', 'navigation', 'context', 'source']) {
    const f = accepted();
    const transaction =
      mode === 'branch'
        ? f.transaction.startState.update({
            annotations: Transaction.addToHistory.of(false),
          })
        : mode === 'navigation'
          ? f.state.update({
              selection: f.state.selection,
              annotations: Transaction.addToHistory.of(false),
            })
          : mode === 'context'
            ? f.state.update({
                effects: invalidateCommandContext.of(null),
                annotations: Transaction.addToHistory.of(false),
              })
            : f.state.update({ changes: { from: 0, insert: 'x' } });
    assert.equal(followPasteImportHistory(f.handle, transaction), false);
    assert.equal(currentPasteImportHistory(f.handle, f.state), false);
    retirePasteImportHistory(f.handle);
  }
});
test('foreign actual command cannot replace the already captured native initial identity', () => {
  const f = accepted();
  const foreign = f.transaction.startState.update({
    changes: { from: 0, insert: 'different' },
    userEvent: 'input.paste',
  });
  assert.equal(acceptPasteImportHistory(f.handle, foreign, foreign.state), false);
  assert.equal(currentPasteImportHistory(f.handle, f.state), true);
  retirePasteImportHistory(f.handle);
});

test('actual accepted command payload substitution permanently retires history even after restored bytes', () => {
  for (const mode of ['changes', 'selection', 'effects']) {
    const f = accepted();
    const transaction = f.transaction;
    let restore: () => void;
    if (mode === 'changes') {
      const original = transaction.changes;
      const foreign = transaction.startState.update({
        changes: { from: 0, insert: 'foreign' },
      });
      assert.equal(Reflect.set(transaction, 'changes', foreign.changes), true);
      restore = () => {
        assert.equal(Reflect.set(transaction, 'changes', original), true);
      };
    } else if (mode === 'selection') {
      const range = transaction.newSelection.main;
      const original = range.goalColumn;
      assert.equal(Reflect.set(range, 'goalColumn', NaN), true);
      restore = () => {
        assert.equal(Reflect.set(range, 'goalColumn', original), true);
      };
    } else {
      const original = transaction.effects;
      assert.equal(
        Reflect.set(transaction, 'effects', [StateEffect.define<null>().of(null)]),
        true,
      );
      restore = () => {
        assert.equal(Reflect.set(transaction, 'effects', original), true);
      };
    }
    assert.equal(currentPasteImportHistory(f.handle, f.state), false);
    restore();
    assert.equal(currentPasteImportHistory(f.handle, f.state), false);
    retirePasteImportHistory(f.handle);
    const following = accepted();
    assert.equal(currentPasteImportHistory(following.handle, following.state), true);
    retirePasteImportHistory(following.handle);
  }
});

test('complete initial assoc and bidi observations refuse positional-equal foreign starts without retiring a genuine original', () => {
  for (const flag of ['assoc', 'bidi'] as const) {
    const origin = structureState('', 'prompt', [
      authorshipExtension,
      richRendering.of(false),
    ]).state;
    const handle = preparePasteImportHistory(origin);
    const next = origin.update({
      selection: EditorSelection.create([
        EditorSelection.cursor(0, flag === 'assoc' ? 1 : 0, flag === 'bidi' ? 1 : undefined),
      ]),
      annotations: Transaction.addToHistory.of(false),
    }).state;
    assert.equal(next.selection.eq(origin.selection), true);
    assert.throws(
      () => pasteImportAnnotations(next, { handle, phase: 'initial' }),
      /history-initial/u,
    );
    const foreign = next.update({
      changes: { from: 0, insert: 'literal' },
      userEvent: 'input.paste',
      annotations: [isolateHistory.of('before'), pasteImportContinuation.of(handle)],
    });
    assert.equal(capturePasteImportHistory(handle, foreign), false);
    const genuine = enteredTextInsertion(origin, 0, 0, 'literal', undefined, {
      handle,
      phase: 'initial',
    }).transactions.at(-1)!;
    assert.equal(capturePasteImportHistory(handle, genuine), true);
    assert.equal(acceptPasteImportHistory(handle, genuine, genuine.state), true);
    assert.equal(currentPasteImportHistory(handle, genuine.state), true);
    retirePasteImportHistory(handle);
  }
});

test('complete tuple-preserving initial metadata preparation remains genuine in raw and rich modes', () => {
  for (const rich of [false, true]) {
    const baseline = structureState('', 'prompt', [
      authorshipExtension,
      richRendering.of(rich),
    ]).state;
    const origin = baseline.update({
      selection: EditorSelection.create([EditorSelection.cursor(0, -1, 2)]),
      annotations: Transaction.addToHistory.of(false),
    }).state;
    const handle = preparePasteImportHistory(origin);
    const neutral = origin.update({
      annotations: Transaction.addToHistory.of(false),
    }).state;
    assert.equal(neutral.doc, origin.doc);
    const genuine = enteredTextInsertion(neutral, 0, 0, 'literal', undefined, {
      handle,
      phase: 'initial',
    }).transactions.at(-1)!;
    assert.equal(capturePasteImportHistory(handle, genuine), true);
    assert.equal(acceptPasteImportHistory(handle, genuine, genuine.state), true);
    assert.equal(currentPasteImportHistory(handle, genuine.state), true);
    retirePasteImportHistory(handle);
  }
});

test('retained original native Text and complete start selection substitution permanently retire initial admission', () => {
  for (const mode of ['document', 'assoc', 'bidi'] as const) {
    const origin = structureState('', 'prompt', [
      authorshipExtension,
      richRendering.of(false),
    ]).state;
    const handle = preparePasteImportHistory(origin);
    const oldDocument = origin.doc,
      oldSelection = origin.selection;
    if (mode === 'document') assert.equal(Reflect.set(origin, 'doc', Text.of(['x'])), true);
    else
      assert.equal(
        Reflect.set(
          origin,
          'selection',
          EditorSelection.create([
            EditorSelection.cursor(0, mode === 'assoc' ? 1 : 0, mode === 'bidi' ? 1 : undefined),
          ]),
        ),
        true,
      );
    assert.throws(
      () => pasteImportAnnotations(origin, { handle, phase: 'initial' }),
      /history-retired/u,
    );
    assert.equal(Reflect.set(origin, 'doc', oldDocument), true);
    assert.equal(Reflect.set(origin, 'selection', oldSelection), true);
    assert.throws(
      () => pasteImportAnnotations(origin, { handle, phase: 'initial' }),
      /history-retired/u,
    );
    retirePasteImportHistory(handle);
  }
});

test('accepted and neutral native Text substitution retires every history entry point before joining or publication', () => {
  for (const boundary of ['current', 'annotations', 'follow', 'join', 'event'] as const)
    for (const neutral of [false, true]) {
      const f = accepted();
      const metadata = f.state.update({
        annotations: Transaction.addToHistory.of(false),
      });
      if (neutral) assert.equal(followPasteImportHistory(f.handle, metadata), true);
      const tip = neutral ? metadata.state : f.state;
      const original = tip.doc;
      const candidate = tip.update({
        annotations: Transaction.addToHistory.of(false),
      });
      const replacement = tip.update({
        changes: { from: 0, to: tip.doc.length, insert: 'tags' },
        annotations: [
          isolateHistory.of('after'),
          pasteImportContinuation.of(f.handle),
          Transaction.time.of(f.transaction.annotation(Transaction.time)!),
        ],
      });
      assert.equal(Reflect.set(tip, 'doc', Text.of(['x' + tip.doc.toString().slice(1)])), true);
      if (boundary === 'current') assert.equal(currentPasteImportHistory(f.handle, tip), false);
      else if (boundary === 'annotations')
        assert.throws(
          () =>
            pasteImportAnnotations(tip, {
              handle: f.handle,
              phase: 'replacement',
            }),
          /history-retired/u,
        );
      else if (boundary === 'follow')
        assert.equal(followPasteImportHistory(f.handle, candidate), false);
      else if (boundary === 'join') assert.equal(joinsPasteImportHistory(replacement), false);
      else
        assert.throws(
          () => pasteImportUserEvent({ handle: f.handle, phase: 'replacement' }),
          /history-retired/u,
        );
      assert.equal(Reflect.set(tip, 'doc', original), true);
      assert.equal(currentPasteImportHistory(f.handle, tip), false);
      retirePasteImportHistory(f.handle);
    }
});

test('neutral successor document substitution refuses before advancement and a fresh genuine receipt remains usable', () => {
  const f = accepted();
  const next = f.state.update({
    annotations: Transaction.addToHistory.of(false),
  });
  const original = next.state.doc;
  assert.equal(Reflect.set(next.state, 'doc', Text.of(['foreign'])), true);
  assert.equal(followPasteImportHistory(f.handle, next), false);
  assert.equal(Reflect.set(next.state, 'doc', original), true);
  assert.equal(currentPasteImportHistory(f.handle, f.state), false);
  retirePasteImportHistory(f.handle);
  const fresh = accepted();
  assert.equal(currentPasteImportHistory(fresh.handle, fresh.state), true);
  retirePasteImportHistory(fresh.handle);
});

test('captured post document and metadata-start tuple mutations retire before acceptance and remain retired after restoration', () => {
  for (const mode of ['post-document', 'start-assoc', 'start-bidi'] as const) {
    const origin = structureState('', 'prompt', [
      authorshipExtension,
      richRendering.of(false),
    ]).state;
    const handle = preparePasteImportHistory(origin);
    const neutral = origin.update({
      annotations: Transaction.addToHistory.of(false),
    }).state;
    const transaction = enteredTextInsertion(neutral, 0, 0, 'literal', undefined, {
      handle,
      phase: 'initial',
    }).transactions.at(-1)!;
    assert.equal(capturePasteImportHistory(handle, transaction), true);
    const originalDocument = transaction.state.doc,
      originalSelection = transaction.startState.selection;
    if (mode === 'post-document')
      assert.equal(Reflect.set(transaction.state, 'doc', Text.of(['changed'])), true);
    else
      assert.equal(
        Reflect.set(
          transaction.startState,
          'selection',
          EditorSelection.create([
            EditorSelection.cursor(
              0,
              mode === 'start-assoc' ? 1 : 0,
              mode === 'start-bidi' ? 1 : undefined,
            ),
          ]),
        ),
        true,
      );
    assert.equal(acceptPasteImportHistory(handle, transaction, transaction.state), false);
    assert.equal(Reflect.set(transaction.state, 'doc', originalDocument), true);
    assert.equal(Reflect.set(transaction.startState, 'selection', originalSelection), true);
    assert.equal(acceptPasteImportHistory(handle, transaction, transaction.state), false);
    retirePasteImportHistory(handle);
  }
});
