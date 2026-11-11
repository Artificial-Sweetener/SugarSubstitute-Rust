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
/** Native source transactions independently qualify exact transfer, ingestion, empty no-ops and prepared text-menu authority. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  Compartment,
  EditorSelection,
  EditorState,
  StateEffect,
  Transaction,
} from '@codemirror/state';
import {
  clipboardCut,
  clipboardPaste,
  clipboardSelection,
} from '../../../../../../frontend/sugarprompt/editor/src/clipboard.ts';
import type { PreparedNativeIngestion } from '../../../../../../frontend/sugarprompt/editor/src/ingestion.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  currentTextMenu,
  prepareTextMenu,
} from '../../../../../../frontend/sugarprompt/editor/src/text-menu.ts';
import { projectionState } from './projection-state.ts';
import {
  authorshipExtension,
  authorship,
} from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';

/** Consume exactly the prepared actual transactions with their original continuous native state identities. */
function appliedClipboardPaste(prepared: PreparedNativeIngestion): EditorState {
  let state = prepared.startState;
  for (const transaction of prepared.transactions) {
    assert.equal(transaction.startState, state);
    state = transaction.state;
  }
  return state;
}

test('reverse source selection and exact private transfers preserve authored weights and mixed newline spellings', () => {
  const source = '🙂\r\n(cat: +1.200)\rtail\n';
  const baseline = projectionState(source);
  const state = baseline.update({
    selection: EditorSelection.single(baseline.doc.length, 0),
  }).state;
  const selected = clipboardSelection(state)!;
  assert.equal(selected.from, 0);
  assert.equal(selected.to, source.length);
  assert.equal(selected.text === source, true);
  const cut = clipboardCut(state)!;
  const empty = state.update(cut).state;
  assert.equal(readExactSource(empty), '');
  assert.equal(cut.userEvent, 'delete.cut');
  const paste = clipboardPaste(empty, { kind: 'exact', text: selected.text })!;
  const restored = appliedClipboardPaste(paste);
  assert.equal(readExactSource(restored) === source, true);
  assert.equal(restored.selection.main.head, restored.doc.length);
  assert.equal(paste.transactions.at(-1)!.annotation(Transaction.userEvent), 'input.paste');
});

test('plain ingestion and private exact transfer share one range owner while empty transfer preserves selection', () => {
  const baseline = projectionState('before keep after').update({
    effects: StateEffect.appendConfig.of(authorshipExtension),
  }).state;
  const state = baseline.update({
    selection: EditorSelection.single(11, 7),
  }).state;
  const plain = clipboardPaste(state, { kind: 'plain', text: '(cat)' })!;
  const exact = clipboardPaste(state, { kind: 'exact', text: '(cat)' })!;
  assert.equal(readExactSource(appliedClipboardPaste(plain)) === 'before (cat:1.10) after', true);
  assert.equal(readExactSource(appliedClipboardPaste(exact)) === 'before (cat) after', true);
  assert.equal(clipboardPaste(state, { kind: 'plain', text: '' }), null);
  assert.equal(clipboardPaste(state, { kind: 'exact', text: '' }), null);
  assert.equal(clipboardSelection(baseline), undefined);
  assert.equal(clipboardCut(baseline), null);
});

test('text menu consumes static availability, captures directional selection and retires on source or permission changes', () => {
  const permission = new Compartment();
  const baseline = projectionState('keep').update({
    effects: StateEffect.appendConfig.of(permission.of(EditorState.readOnly.of(false))),
  }).state;
  const state = baseline.update({
    selection: EditorSelection.single(4, 0),
  }).state;
  const menu = prepareTextMenu(state, { readable: true, writable: true });
  assert.equal(currentTextMenu(state, menu), true);
  assert.deepEqual(
    menu.items.filter((item) => item.kind === 'text').map((item) => item.action),
    ['cut', 'copy', 'paste', 'select-all'],
  );
  assert.equal(
    currentTextMenu(state.update({ selection: { anchor: 0, head: 4 } }).state, menu),
    false,
  );
  assert.equal(
    currentTextMenu(state.update({ changes: { from: 0, insert: 'X' } }).state, menu),
    false,
  );
  const readOnly = state.update({
    effects: permission.reconfigure(EditorState.readOnly.of(true)),
  }).state;
  assert.equal(currentTextMenu(readOnly, menu), false);
  const disabled = prepareTextMenu(readOnly, {
    readable: true,
    writable: true,
  });
  assert.deepEqual(
    disabled.items.filter((item) => item.kind === 'text').map((item) => item.action),
    ['copy', 'select-all'],
  );
  assert.equal(clipboardCut(readOnly), null);
  assert.equal(clipboardPaste(readOnly, { kind: 'exact', text: 'replacement' }), null);
});

test('plain CSV clipboard input uses genuine scoped normalization while private exact transfer preserves the exact storage payload', () => {
  const baseline = projectionState('header\n,""', new Compartment(), 'wildcard-csv').update({
    effects: StateEffect.appendConfig.of(authorshipExtension),
  }).state;
  const state = baseline.update({ selection: { anchor: 7 } }).state;
  const text = '(cat)\r\n"🙂"';
  const plain = clipboardPaste(state, { kind: 'plain', text })!,
    exact = clipboardPaste(state, { kind: 'exact', text })!;
  const plainState = appliedClipboardPaste(plain),
    exactState = appliedClipboardPaste(exact);
  assert.ok(plain.edit);
  assert.equal(plain.edit.postValues.scopes.scopes[0]!.mapping.text, '(cat:1.10)\r\n"🙂"');
  assert.equal(plain.edit.postValues.scopes.scopes[1]!.mapping.text, '');
  assert.equal(readExactSource(plainState), 'header\n"(cat:1.10)\r\n""🙂""",""');
  assert.equal(plainState.field(authorship).generated.size, 1);
  assert.equal(exact.edit, null);
  assert.equal(readExactSource(exactState), 'header\n(cat)\r\n"🙂",""');
  assert.equal(exactState.field(authorship).generated.size, 0);
  assert.equal(plain.transactions.at(-1)!.annotation(Transaction.userEvent), 'input.paste');
  assert.equal(exact.transactions[0]!.annotation(Transaction.userEvent), 'input.paste');
});
