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
/** One finite publication adapter consumes genuine ingestion transactions without reconstructing native state. */
import { Transaction, type EditorState } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import type { PreparedNativeIngestion } from './ingestion.js';
import { nativeCompositionStarted } from './input-settlement.js';
import { changesExactSource, lineEndings } from './exact-source.js';
import { editorFreshness } from './freshness.js';
import { semanticStructures, installSemanticStructure } from './structure.js';
import { currentDocumentValueEdit } from './document-value-edit-issuance.js';
import type { PreparedDocumentValueEdit } from './document-value-edits.js';
import { currentNativeIngestionPost } from './native-ingestion-post.js';

/** Recheck original issuance against the actual canonical command state before publication. */
function currentIngestionEdit(state: EditorState, edit: PreparedDocumentValueEdit): boolean {
  const semantic = state.field(semanticStructures).native;
  return (
    semantic !== null &&
    currentDocumentValueEdit(
      state,
      {
        values: semantic.nativeValues,
        semantics: semantic.documentSemantics,
        enabled: true,
        composing: false,
      },
      edit,
    ) === null
  );
}

/** Authenticate ordinary pending-rendering structure or an issued canonical post under the same source-neutral policy. */
function semanticLink(transaction: Transaction, edit?: PreparedDocumentValueEdit): boolean {
  const before = transaction.startState;
  const effect = transaction.effects[0];
  if (
    (transaction.effects.length !== 1 && !currentNativeIngestionPost(transaction, edit)) ||
    !effect?.is(installSemanticStructure) ||
    effect.value.purpose !== 'native' ||
    transaction.annotation(Transaction.addToHistory) !== false ||
    transaction.annotation(Transaction.userEvent) !== undefined ||
    transaction.scrollIntoView ||
    changesExactSource(transaction) ||
    transaction.state.doc !== before.doc ||
    transaction.state.field(lineEndings) !== before.field(lineEndings) ||
    transaction.state.selection !== before.selection ||
    transaction.state.field(editorFreshness) !== before.field(editorFreshness) ||
    transaction.state.field(semanticStructures).native !== effect.value.prepared
  )
    return false;
  return (
    !edit ||
    (effect.value.prepared.nativeValues === edit.postValues &&
      effect.value.prepared.documentSemantics === edit.semantics)
  );
}

/** Publish at most one genuine preparation, its issued command and its authenticated seeded semantic adoption; reentry retires remaining links. */
export function dispatchNativeIngestion(
  view: EditorView,
  prepared: PreparedNativeIngestion,
): boolean {
  const original = prepared.transactions,
    startState = prepared.startState;
  const length = original.length;
  if (length > 3) return false;
  const captured: Transaction[] = [];
  for (let index = 0; index < length; index++) {
    const transaction = original[index];
    if (!Object.hasOwn(original, index) || !(transaction instanceof Transaction)) return false;
    captured.push(transaction);
  }
  const transactions = Object.freeze(captured);
  /** Observer changes retire the remaining private chain; actual authenticated transaction objects are never reconstructed. */
  const chainCurrent = (): boolean =>
    prepared.transactions === original &&
    original.length === length &&
    transactions.every(
      (transaction, index) => Object.hasOwn(original, index) && original[index] === transaction,
    );
  const edit = prepared.edit;
  const command = edit?.transactions[0];
  const commandIndex = command ? transactions.indexOf(command) : transactions.length - 1;
  const metadataCount = edit && !command ? transactions.length : commandIndex;
  const postCount = command ? transactions.length - commandIndex - 1 : 0;
  if (
    metadataCount < 0 ||
    metadataCount > 1 ||
    postCount > 1 ||
    (!command && transactions.length > 2)
  )
    return false;
  let expected = startState;
  for (let index = 0; index < transactions.length; index++) {
    const transaction = transactions[index];
    if (
      !Object.hasOwn(transactions, index) ||
      !(transaction instanceof Transaction) ||
      transaction.startState !== expected
    )
      return false;
    if (
      (index < metadataCount || index > commandIndex) &&
      !semanticLink(transaction, index > commandIndex ? (edit ?? undefined) : undefined)
    )
      return false;
    expected = transaction.state;
  }
  if (edit) {
    if (edit.transactions.length > 1 || (command ? commandIndex < 0 : expected !== edit.startState))
      return false;
    if (!currentIngestionEdit(edit.startState, edit)) return false;
  }
  if (nativeCompositionStarted(view) || view.state.readOnly || view.state !== startState)
    return false;
  for (const [index, transaction] of transactions.entries()) {
    if (!chainCurrent()) return false;
    if (
      (index < metadataCount || index > commandIndex) &&
      (!semanticLink(transaction, index > commandIndex ? (edit ?? undefined) : undefined) ||
        (edit && !currentIngestionEdit(edit.startState, edit)))
    )
      return false;
    if (
      nativeCompositionStarted(view) ||
      view.state.readOnly ||
      view.state !== transaction.startState ||
      !chainCurrent()
    )
      return false;
    if (transaction === command && edit && !currentIngestionEdit(view.state, edit)) return false;
    view.dispatch(transaction);
  }
  return true;
}
