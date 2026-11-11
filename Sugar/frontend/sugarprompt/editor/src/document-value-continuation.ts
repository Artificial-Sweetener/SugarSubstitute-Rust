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
/** Metadata-only command receipts compile original issued output without granting stale edits current authority. */
import { Transaction, type EditorState } from '@codemirror/state';
import { type PreparedDocumentValueEdit } from './document-value-edits.js';
import type { NativeDocumentValueContext } from './document-values.js';
import { changesExactSource, lineEndings } from './exact-source.js';
import { editorFreshness } from './freshness.js';
import {
  authentic,
  recipeFor,
  compileNativeRecipe,
  type Recipe,
} from './document-value-continuation-recipes.js';
import {
  selectionContents,
  selectionMatches,
  type NativeSelectionContents,
} from './native-selection.js';
import { installAuthorship } from './authorship.js';
/** Opaque evidence has no public source, selection or issuer capability. */
export interface NativeEditContinuation {
  readonly identity: unique symbol;
}
/** Expected freshness and authority failures carry only a stable source-free code. */
interface Refused {
  readonly status: 'rejected';
  readonly reason: 'continuation-refused';
}
/** Native observation failure releases the command's executable receipt. */
interface Retired {
  readonly status: 'retired';
}
/** A successful operation exposes only its requested finite result. */
interface Ready<T> {
  readonly status: 'ready';
  readonly value: T;
}
const refused: Refused = Object.freeze({
  status: 'rejected',
  reason: 'continuation-refused',
});
const retired: Retired = Object.freeze({ status: 'retired' });
/** A persistent tip retains one native description, not a transaction history. */
interface Receipt {
  readonly origin: EditorState;
  readonly tip: EditorState;
  readonly selection: NativeSelectionContents;
  readonly endings: readonly string[];
  readonly recipe: Recipe;
}
const receipts = new WeakMap<NativeEditContinuation, Receipt>();
const mutatedTips = new WeakSet<Receipt>();
/** Each branch gets a distinct private receipt; speculative native updates cannot advance another branch. */
function issue(receipt: Receipt): NativeEditContinuation {
  const handle = Object.freeze({}) as NativeEditContinuation;
  receipts.set(handle, receipt);
  return handle;
}
/** Begin a metadata-only command lifetime from an actual current issued edit. */
export function prepareNativeEditContinuation(
  state: EditorState,
  context: NativeDocumentValueContext,
  edit: PreparedDocumentValueEdit,
): Ready<NativeEditContinuation> | Refused {
  const recipe = recipeFor(state, context, edit);
  return recipe
    ? {
        status: 'ready',
        value: issue({
          origin: state,
          tip: state,
          selection: selectionContents(state.selection),
          endings: [...state.field(lineEndings)],
          recipe,
        }),
      }
    : refused;
}
/** Source-free currentness checks never parse or substitute stale canonical values into the tip. */
function valid(receipt: Receipt, state: EditorState): boolean {
  if (receipt.tip !== state) return false;
  const intact =
    selectionMatches(state.selection, receipt.selection) &&
    state.field(lineEndings).length === receipt.endings.length &&
    receipt.endings.every(
      (ending, index) =>
        Object.hasOwn(state.field(lineEndings), index) &&
        state.field(lineEndings)[index] === ending,
    );
  if (!intact) mutatedTips.add(receipt);
  return intact && !mutatedTips.has(receipt) && !state.readOnly && authentic(receipt.recipe);
}
/** Metadata-only transitions advance the receipt; exact-source, context or authorship changes retire it. */
export function followNativeEditContinuation(
  handle: NativeEditContinuation,
  transaction: Transaction,
): Ready<NativeEditContinuation> | Retired {
  const previous = receipts.get(handle);
  if (
    !(transaction instanceof Transaction) ||
    !previous ||
    !valid(previous, transaction.startState)
  )
    return retired;
  const state = transaction.state,
    before = transaction.startState.field(editorFreshness),
    after = state.field(editorFreshness);
  if (
    state.readOnly ||
    transaction.isUserEvent('input.type.compose') ||
    before.context !== after.context ||
    transaction.effects.some((effect) => effect.is(installAuthorship)) ||
    changesExactSource(transaction) ||
    before.source !== after.source ||
    state.doc !== transaction.startState.doc ||
    !selectionMatches(state.selection, previous.selection) ||
    state.field(lineEndings) !== transaction.startState.field(lineEndings)
  )
    return retired;
  return {
    status: 'ready',
    value: issue({
      ...previous,
      tip: state,
      selection: selectionContents(state.selection),
      endings: [...state.field(lineEndings)],
    }),
  };
}
/** Current menu actions compile only their original genuinely issued native recipe. */
export function nativeEditContinuationTransaction(
  state: EditorState,
  handle: NativeEditContinuation,
): Ready<Transaction> | Refused {
  const receipt = receipts.get(handle);
  return receipt && valid(receipt, state)
    ? { status: 'ready', value: compileNativeRecipe(state, receipt.origin, receipt.recipe) }
    : refused;
}
