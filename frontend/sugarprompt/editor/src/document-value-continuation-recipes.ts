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
/** Original-native command recipes delegate authority to the actual canonical issuer. */
import { requireValue } from '../../core/src/required.js';
import { Transaction, type EditorState } from '@codemirror/state';
import { isolateHistory } from '@codemirror/commands';
import { currentDocumentValueEdit } from './document-value-edit-issuance.js';
import { selectionContents, selectionMatches } from './native-selection.js';
import type { PreparedDocumentValueEdit } from './document-value-edits.js';
import { normalizationJoin } from './input-descriptor.js';
import { authoredImplicitDepth } from './normalization-events.js';
import type { NativeDocumentValueContext } from './document-values.js';
import { lineEndings } from './exact-source.js';
/** Original transaction authority is never reconstructed from DTO output. */
export interface Recipe {
  readonly edit: PreparedDocumentValueEdit;
  readonly transaction: Transaction | null;
  readonly context: NativeDocumentValueContext;
}
/** Revalidate original issuance at the actual origin; issuer mutation refusal remains permanent. */
export function authentic(recipe: Recipe): boolean {
  const { edit, transaction, context } = recipe;
  return (
    currentDocumentValueEdit(edit.startState, context, edit) === null &&
    edit.transactions.length === (transaction ? 1 : 0) &&
    (edit.transactions[0] ?? null) === transaction
  );
}
/** Admit one current issued edit with at most one native transaction. */
export function recipeFor(
  state: EditorState,
  context: NativeDocumentValueContext,
  edit: PreparedDocumentValueEdit,
): Recipe | null {
  if (currentDocumentValueEdit(state, context, edit) !== null || edit.transactions.length > 1)
    return null;
  const recipe = { edit, transaction: edit.transactions[0] ?? null, context };
  return authentic(recipe) ? recipe : null;
} /** Compile the original native publication at the exact observed tip and verify its complete actual output. */
export function compileNativeRecipe(
  state: EditorState,
  origin: EditorState,
  recipe: Recipe,
): Transaction {
  const original = recipe.transaction;
  if (!original) {
    if (
      !state.doc.eq(origin.doc) ||
      !selectionMatches(state.selection, selectionContents(origin.selection))
    )
      throw new Error('native-continuation-noop-mismatch');
    return state.update({ annotations: Transaction.addToHistory.of(false) });
  }
  if (state === origin) return original;
  const transaction = state.update({
    changes: original.changes,

    selection: original.newSelection,
    effects: original.effects,
    annotations: [
      original.annotation(Transaction.addToHistory) === undefined
        ? []
        : Transaction.addToHistory.of(
            requireValue(
              original.annotation(Transaction.addToHistory),
              'document-value-continuation-recipes',
            ),
          ),
      original.annotation(isolateHistory) === undefined
        ? []
        : isolateHistory.of(
            requireValue(
              original.annotation(isolateHistory),
              'document-value-continuation-recipes',
            ),
          ),
      Transaction.time.of(
        requireValue(original.annotation(Transaction.time), 'document-value-continuation-recipes'),
      ),
      original.annotation(normalizationJoin) === undefined
        ? []
        : normalizationJoin.of(
            requireValue(
              original.annotation(normalizationJoin),
              'document-value-continuation-recipes',
            ),
          ),
      original.annotation(authoredImplicitDepth) === undefined
        ? []
        : authoredImplicitDepth.of(
            requireValue(
              original.annotation(authoredImplicitDepth),
              'document-value-continuation-recipes',
            ),
          ),
    ].flat(),
    ...(original.annotation(Transaction.userEvent) === undefined
      ? {}
      : {
          userEvent: requireValue(
            original.annotation(Transaction.userEvent),
            'document-value-continuation-recipes',
          ),
        }),
    scrollIntoView: original.scrollIntoView,
  });
  if (
    transaction.scrollIntoView !== original.scrollIntoView ||
    transaction.annotation(Transaction.addToHistory) !==
      original.annotation(Transaction.addToHistory) ||
    transaction.annotation(isolateHistory) !== original.annotation(isolateHistory) ||
    transaction.annotation(Transaction.userEvent) !== original.annotation(Transaction.userEvent) ||
    !Object.is(transaction.annotation(Transaction.time), original.annotation(Transaction.time)) ||
    transaction.annotation(normalizationJoin) !== original.annotation(normalizationJoin) ||
    transaction.annotation(authoredImplicitDepth) !== original.annotation(authoredImplicitDepth) ||
    !transaction.newDoc.eq(original.newDoc) ||
    !selectionMatches(transaction.newSelection, selectionContents(original.newSelection)) ||
    transaction.effects.length !== original.effects.length ||
    transaction.effects.some((effect, index) => effect !== original.effects[index]) ||
    transaction.state.field(lineEndings).length !== original.state.field(lineEndings).length ||
    transaction.state
      .field(lineEndings)
      .some((ending, index) => ending !== original.state.field(lineEndings)[index])
  )
    throw new Error('native-continuation-output-mismatch');
  return transaction;
}
