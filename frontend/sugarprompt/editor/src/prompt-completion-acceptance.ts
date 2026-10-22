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
/** Native prompt completion consumes an already prepared transaction synchronously, preserving ordering with following input. */
import { requireValue } from '../../core/src/required.js';
import { captureFocusHandoff, currentFocusHandoff } from './focus-handoff.js';
import type { EditorView } from '@codemirror/view';
import type { EditorState, Transaction } from '@codemirror/state';
import { editorFreshness } from './freshness.js';
import type {
  PreparedCompletion,
  PreparedCompletionResult,
  CompletionActivation,
} from './prompt-completion-model.js';
import { currentDocumentValueEdit } from './document-value-edit-issuance.js';
import { semanticStructures } from './structure.js';
import { completionSession, completionContinuation } from './completion-session.js';
import { completionContinuationTransaction } from './document-value-continuation.js';
/** A native session alone can retarget prepared output after fully consumed content edits; arbitrary stale proposals still reject. */
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
    !option ||
    state.readOnly ||
    session.freshness !== state.field(editorFreshness) ||
    !state.selection.main.empty ||
    session.caret !== state.selection.main.head
  )
    return null;
  const continuation = completionContinuation(state);
  if (!continuation) return null;
  const outcome = completionContinuationTransaction(state, continuation, result, index, activation);
  return outcome.status === 'ready' ? outcome.value : null;
}
/** Accept a visible native option before following physical input, without parsing or ranking in the key/pointer callback. */
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
/** Validate prepared source/context/caret against current native state and build one history-isolated transaction without semantic work. */
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
  const session = state.field(completionSession, false);
  if (session?.phase === 'ready' && completionContinuation(state)) {
    const index = session.result.options.indexOf(prepared);
    if (index >= 0) return completionOptionTransaction(state, session.result, index, activation);
  }
  const semantic = state.field(semanticStructures).native;
  if (!semantic) return null;
  const edit = prepared.acceptance[activation].edit;
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
    ? requireValue(edit.transactions[0], 'prompt-completion-acceptance')
    : null;
}
