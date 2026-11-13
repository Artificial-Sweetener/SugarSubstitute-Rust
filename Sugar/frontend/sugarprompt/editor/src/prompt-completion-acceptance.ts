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
/**
 * Accepting a suggestion builds its edit at that moment, the standard autocomplete pattern: the
 * list only ranks labels while the user types, and the accepted row's canonical edit is issued
 * and validated against the current document in the key or pointer callback, then applied as
 * one transaction.
 */
import type { EditorView } from '@codemirror/view';
import type { EditorState, Transaction } from '@codemirror/state';
import { captureFocusHandoff, currentFocusHandoff } from './focus-handoff.js';
import { editorFreshness } from './freshness.js';
import type {
  PreparedCompletion,
  PreparedCompletionResult,
  CompletionActivation,
} from './prompt-completion-model.js';
import { currentDocumentValueEdit } from './document-value-edit-issuance.js';
import { semanticStructures } from './structure.js';
import { completionSession } from './completion-session.js';
/** Accept one row of the visible session only while its source, context and caret are still current. */
export function completionOptionTransaction(
  state: EditorState,
  result: PreparedCompletionResult,
  index: number,
  activation: CompletionActivation,
): Transaction | null {
  const session = state.field(completionSession),
    option = Number.isSafeInteger(index) ? result.options[index] : undefined;
  if (
    !session ||
    session.result !== result ||
    session.phase !== 'ready' ||
    !option ||
    session.freshness !== state.field(editorFreshness) ||
    session.caret !== state.selection.main.head
  )
    return null;
  return completionAcceptanceTransaction(state, option, activation);
}
/** Accept a visible native option before following physical input. */
export function acceptCompletionOption(
  view: EditorView,
  result: PreparedCompletionResult,
  index: number,
  activation: CompletionActivation,
): boolean {
  if (!view.hasFocus || view.compositionStarted || view.composing) return false;
  const transaction = completionOptionTransaction(view.state, result, index, activation);
  if (!transaction) return false;
  const handoff = captureFocusHandoff(view);
  if (
    view.state !== transaction.startState ||
    !currentFocusHandoff(view, handoff) ||
    !view.hasFocus ||
    view.compositionStarted ||
    view.composing
  ) {
    handoff.cancel();
    return false;
  }
  view.dispatch(transaction);
  void handoff.restore();
  return true;
}
/** Issue one option's edit for the current state and admit it only as a current canonical edit. */
export function completionAcceptanceTransaction(
  state: EditorState,
  prepared: PreparedCompletion,
  activation: CompletionActivation,
): Transaction | null {
  if (
    state.readOnly ||
    state.field(editorFreshness) !== prepared.freshness ||
    !state.selection.main.empty ||
    state.selection.main.head !== prepared.caret
  )
    return null;
  const semantic = state.field(semanticStructures, false)?.native;
  if (!semantic) return null;
  const edit = prepared.prepareEdit(state, activation);
  const transaction = edit?.transactions[0];
  if (!edit || edit.transactions.length !== 1 || transaction?.startState !== state) return null;
  return currentDocumentValueEdit(
    state,
    {
      values: semantic.nativeValues,
      semantics: semantic.documentSemantics,
      enabled: true,
      composing: false,
    },
    edit,
  ) === null
    ? transaction
    : null;
}
