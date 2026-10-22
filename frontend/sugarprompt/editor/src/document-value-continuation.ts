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
/** Finite native transition evidence compiles original issued output without granting stale edits current authority. */
import { requireValue } from '../../core/src/required.js';
import { Transaction, EditorSelection, type EditorState, type ChangeDesc } from '@codemirror/state';
import { type PreparedDocumentValueEdit } from './document-value-edits.js';
import type { NativeDocumentValueContext } from './document-values.js';
import type {
  PreparedCompletionScope,
  PreparedCompletionResult,
  PreparedCompletionSplice,
  CompletionActivation,
} from './prompt-completion-model.js';
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
import {
  completionResultMatches,
  type CompletionEvidence,
  type Candidate,
} from './document-value-completion-evidence.js';
import { installAuthorship } from './authorship.js';
import {
  stableCompletionEdit,
  hasCompletionPrefix,
} from '../../core/src/completion/continuation.js';
/** Opaque evidence has no public source, selection or issuer capability. */
export interface NativeEditContinuation {
  readonly identity: unique symbol;
}
/** Completion evidence shares the native recipe owner. */
export type NativeCompletionContinuation = NativeEditContinuation;
/** Expected freshness and authority failures carry only a stable source-free code. */
interface Refused {
  readonly status: 'rejected';
  readonly reason: 'continuation-refused';
}
/** Native observation failure releases the session's executable receipt. */
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
interface Receipt extends CompletionEvidence {
  readonly origin: EditorState;
  readonly tip: EditorState;
  readonly selection: NativeSelectionContents;
  readonly endings: readonly string[];
  readonly recipes: readonly Recipe[];
  readonly lineage: ChangeDesc;
  readonly caret: number;
  readonly queryTo: number;
  readonly changed: boolean;
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
  const recipe = recipeFor(state, context, edit, null);
  return recipe
    ? {
        status: 'ready',
        value: issue({
          origin: state,
          tip: state,
          selection: selectionContents(state.selection),
          endings: [...state.field(lineEndings)],
          recipes: [recipe],
          lineage: state.changes([]).desc,
          result: null,
          candidates: [],
          queryBounds: null,
          kind: null,
          query: null,
          caret: state.selection.main.head,
          queryTo: 0,
          changed: false,
        }),
      }
    : refused;
}
/** Admit every genuine activation/retained/right variant before observing input. */
export function prepareNativeCompletionContinuation(
  state: EditorState,
  scope: PreparedCompletionScope,
  result: PreparedCompletionResult,
): Ready<NativeCompletionContinuation> | Refused {
  if (
    result.query.scope !== scope.target.scope ||
    !result.options.length ||
    !state.selection.main.empty
  )
    return refused;
  const recipes: Recipe[] = [],
    candidates: Candidate[] = [];
  for (const option of result.options) {
    if (
      option.freshness !== state.field(editorFreshness) ||
      option.caret !== state.selection.main.head
    )
      return refused;
    for (const activation of ['keyboard', 'pointer'] as const) {
      const acceptance = option.acceptance[activation];
      for (const [edit, splice] of [
        [acceptance.edit, null],
        ...(acceptance.splice ? [[acceptance.splice.edit, acceptance.splice]] : []),
        ...(acceptance.right ? [[acceptance.right.splice.edit, acceptance.right.splice]] : []),
      ] as readonly (readonly [PreparedDocumentValueEdit, PreparedCompletionSplice | null])[]) {
        const recipe = recipeFor(state, scope.context, edit, splice);
        if (!recipe || edit.target.scope !== scope.target.scope) return refused;
        recipes.push(recipe);
      }
    }
    candidates.push({
      option,
      kind: option.kind,
      caret: option.caret,
      freshness: option.freshness,
      keyboard: option.acceptance.keyboard,
      pointer: option.acceptance.pointer,
      edits: [option.acceptance.keyboard, option.acceptance.pointer].map((acceptance) => ({
        acceptance,
        edit: acceptance.edit,
        splice: acceptance.splice,
        right: acceptance.right,
        rightSplice: acceptance.right?.splice ?? null,
        rightMatch: acceptance.right?.match ?? null,
        maximum: acceptance.right?.match.maximumSourcePrefixLength ?? null,
        consumes: acceptance.right?.match.consumesSourcePrefix ?? null,
      })),
    });
  }
  const query = result.query;
  if (
    ![query.from, query.contentFrom, query.to].every(Number.isSafeInteger) ||
    query.from < 0 ||
    query.contentFrom < query.from ||
    query.to < query.contentFrom ||
    query.to > state.doc.length
  )
    return refused;
  const mapping = scope.target.entry.semanticMapping;
  if (
    !mapping ||
    [query.from, query.contentFrom, query.to].some(
      (position) =>
        position < mapping.logicalRange.from ||
        position > mapping.logicalRange.to ||
        mapping.logicalToDecoded[position - mapping.logicalRange.from] == null,
    )
  )
    return refused;
  return {
    status: 'ready',
    value: issue({
      origin: state,
      tip: state,
      selection: selectionContents(state.selection),
      endings: [...state.field(lineEndings)],
      recipes,
      candidates,
      queryBounds: [query.from, query.contentFrom, query.to, result.from, result.to],
      kind: query.kind,
      lineage: state.changes([]).desc,
      result,
      query,
      caret: state.selection.main.head,
      queryTo: query.to,
      changed: false,
    }),
  };
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
  return (
    intact &&
    !mutatedTips.has(receipt) &&
    completionResultMatches(receipt) &&
    !state.readOnly &&
    receipt.recipes.every(authentic)
  );
}
/** Compatible deletion may reset the main cursor exactly as the native delete command; secondary ranges and all other transitions retain complete mapped intent. */
function nativeInputSelection(transaction: Transaction, caret: number): boolean {
  const mapped = transaction.startState.selection.map(transaction.changes, 1);
  if (selectionMatches(transaction.state.selection, selectionContents(mapped))) return true;
  const backward = transaction.isUserEvent('delete.backward'),
    forward = transaction.isUserEvent('delete.forward');
  if (!backward && !forward) return false;
  let adjacent = true,
    count = 0;
  transaction.changes.iterChanges((from, to, _a, _b, inserted) => {
    count++;
    if (
      from >= to ||
      inserted.length !== 0 ||
      (backward
        ? to !== transaction.startState.selection.main.head
        : from !== transaction.startState.selection.main.head)
    )
      adjacent = false;
  });
  if (!adjacent || count !== 1 || !transaction.startState.selection.main.empty) return false;
  return [0, backward ? -1 : 1].some((assoc) => {
    const ranges = [...mapped.ranges];
    ranges[mapped.mainIndex] = EditorSelection.cursor(caret, assoc);
    return selectionMatches(
      transaction.state.selection,
      selectionContents(EditorSelection.create(ranges, mapped.mainIndex)),
    );
  });
}
/** Observe one actual transition; metadata and compatible prompt inputs share exact continuity fences. */
function follow(
  handle: NativeEditContinuation,
  transaction: Transaction,
  allowInput: boolean,
):
  | Ready<{
      readonly continuation: NativeEditContinuation;
      readonly phase: 'ready' | 'refreshing';
      readonly caret: number;
      readonly queryTo: number;
    }>
  | Retired {
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
    transaction.effects.some((effect) => effect.is(installAuthorship))
  )
    return retired;
  const sourceChanged = changesExactSource(transaction);
  let caret = previous.caret,
    queryTo = previous.queryTo,
    recipes = previous.recipes;
  if (!sourceChanged) {
    if (
      before.source !== after.source ||
      state.doc !== transaction.startState.doc ||
      !selectionMatches(state.selection, previous.selection) ||
      state.field(lineEndings) !== transaction.startState.field(lineEndings)
    )
      return retired;
  } else {
    const query = previous.query;
    if (
      !allowInput ||
      !query ||
      query.scope.container.location.kind !== 'prompt' ||
      !transaction.docChanged ||
      transaction.isUserEvent('input.type.compose') ||
      (!transaction.isUserEvent('input.type') && !transaction.isUserEvent('delete')) ||
      !state.selection.main.empty
    )
      return retired;
    let count = 0,
      stable = true;
    transaction.changes.iterChanges((from, to, _a, _b, inserted) => {
      count++;
      if (
        from < query.contentFrom ||
        to > previous.queryTo ||
        (query.kind === 'tag' && to > previous.caret) ||
        !stableCompletionEdit(
          query.kind,
          transaction.startState.doc.sliceString(Math.max(0, from - 3), from),
          transaction.startState.doc.sliceString(from, to),
          inserted.toString(),
          transaction.startState.doc.sliceString(
            to,
            Math.min(transaction.startState.doc.length, to + 3),
          ),
        )
      )
        stable = false;
      recipes = recipes.filter(
        (recipe) =>
          from >= previous.lineage.mapPos(recipe.from, -1) &&
          to <= previous.lineage.mapPos(recipe.to, 1),
      );
    });
    caret = transaction.changes.mapPos(caret, 1);
    queryTo = transaction.changes.mapPos(queryTo, 1);
    if (
      count !== 1 ||
      !stable ||
      !recipes.length ||
      state.selection.main.head !== caret ||
      caret < query.contentFrom ||
      caret > queryTo ||
      (query.kind === 'tag' &&
        (/^[ \t]/u.test(state.doc.sliceString(query.contentFrom, query.contentFrom + 1)) ||
          !hasCompletionPrefix(
            state.doc.sliceString(query.contentFrom, Math.min(caret, query.contentFrom + 4)),
          )))
    )
      return retired;
    if (!nativeInputSelection(transaction, caret)) return retired;
  }
  const receipt: Receipt = {
    ...previous,
    tip: state,
    selection: selectionContents(state.selection),
    endings: [...state.field(lineEndings)],
    recipes,
    lineage: previous.lineage.composeDesc(transaction.changes.desc),
    caret,
    queryTo,
    changed: previous.changed || sourceChanged,
  };
  return {
    status: 'ready',
    value: {
      continuation: issue(receipt),
      phase: receipt.changed ? 'refreshing' : 'ready',
      caret,
      queryTo,
    },
  };
}
/** Metadata-only actions retire immediately on any exact-source mutation. */
export function followNativeEditContinuation(
  handle: NativeEditContinuation,
  transaction: Transaction,
): Ready<NativeEditContinuation> | Retired {
  const outcome = follow(handle, transaction, false);
  return outcome.status === 'ready'
    ? { status: 'ready', value: outcome.value.continuation }
    : outcome;
}
/** Completion follows only syntax-compatible actual native consumed-prefix edits. */
export function followNativeCompletionContinuation(
  handle: NativeCompletionContinuation,
  transaction: Transaction,
) {
  return follow(handle, transaction, true);
}
/** Compile only an authenticated selected recipe at the exact persistent native tip. */
function compile(
  state: EditorState,
  receipt: Receipt,
  recipe: Recipe,
): Ready<Transaction> | Refused {
  return valid(receipt, state) && receipt.recipes.includes(recipe) && authentic(recipe)
    ? {
        status: 'ready',
        value: compileNativeRecipe(state, receipt.origin, receipt.lineage, receipt.changed, recipe),
      }
    : refused;
}
/** Current menu actions compile only their original genuinely issued native recipe. */
export function nativeEditContinuationTransaction(
  state: EditorState,
  handle: NativeEditContinuation,
): Ready<Transaction> | Refused {
  const receipt = receipts.get(handle);
  return receipt && !receipt.result
    ? compile(state, receipt, requireValue(receipt.recipes[0], 'document-value-continuation'))
    : refused;
}
/** Result/activation membership chooses an authentic retained or right-consuming variant, never detached output. */
export function completionContinuationTransaction(
  state: EditorState,
  handle: NativeCompletionContinuation,
  result: PreparedCompletionResult,
  index: number,
  activation: CompletionActivation,
): Ready<Transaction> | Refused {
  const receipt = receipts.get(handle),
    option = Number.isSafeInteger(index) ? result.options[index] : undefined;
  if (
    !receipt ||
    !completionResultMatches(receipt) ||
    receipt.result !== result ||
    !option ||
    result.query !== receipt.query
  )
    return refused;
  const acceptance = option.acceptance[activation];
  let splice = receipt.changed ? acceptance.splice : null;
  const right = acceptance.right;
  if (
    receipt.changed &&
    right &&
    receipt.caret - result.query.contentFrom <= right.match.maximumSourcePrefixLength &&
    right.match.consumesSourcePrefix(state.doc.sliceString(result.query.contentFrom, receipt.caret))
  )
    splice = right.splice;
  if (receipt.changed && !splice) return refused;
  const edit = splice?.edit ?? acceptance.edit;
  const recipe = receipt.recipes.find((recipe) => recipe.edit === edit && recipe.splice === splice);
  return recipe ? compile(state, receipt, recipe) : refused;
}
