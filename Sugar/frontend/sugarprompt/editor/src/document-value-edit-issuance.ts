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
/** Complete genuine command issuance evidence has one weak owner and no source or selection authority. */
import { requireValue } from '../../core/src/required.js';
import {
  Transaction,
  type EditorState,
  type Text,
  type ChangeSet,
  type EditorSelection,
  type StateEffect,
} from '@codemirror/state';
import {
  selectionContents,
  selectionMatches,
  type NativeSelectionContents,
} from './native-selection.js';
import { isolateHistory } from '@codemirror/commands';
import type { PreparedDocumentSemantics } from '../../core/src/index.js';
import { lineEndings } from './exact-source.js';
import { currentNativeDocumentValue, type NativeDocumentValueContext } from './document-values.js';
import {
  captureDocumentValueEffect,
  documentValueEffectMatches,
  type CapturedDocumentValueEffect,
} from './document-value-edit-publication.js';
import {
  prepareDocumentValueEditInput,
  type DecodedDocumentValueEdit,
  type DocumentValueEditOutcome,
  type PreparedDocumentValueEdit,
  type DocumentValueEditRejection,
} from './document-value-edits.js';
import { prepareDocumentSemantics } from '../../core/src/index.js';
import { prepareNativeDocumentValues } from './document-values.js';
import { readExactSource } from './exact-source.js';
import { prepareDocumentValuePublication } from './document-value-edit-publication.js';
import { authorship, installAuthorship, normalizedAuthorship } from './authorship.js';
import { pasteImportContinuation, type PasteImportHistory } from './paste-import-history.js';
import { normalizationJoin } from './input-descriptor.js';
import { authoredImplicitDepth } from './normalization-events.js';
import { reorderedAuthorship } from './reorder-authorship.js';
/** Weak issuance evidence retains only the genuine result's captured state and input semantics, not another source/cache owner. */
interface DocumentValueEditIssuance {
  readonly postState: EditorState;
  readonly document: Text;
  readonly endings: readonly string[];
  readonly semantics: PreparedDocumentSemantics;
  readonly transaction: Transaction | null;
  readonly changes: ChangeSet | null;
  readonly changeContents: string;
  readonly selection: EditorSelection | undefined;
  readonly selectionContents: NativeSelectionContents;
  readonly effects: readonly StateEffect<unknown>[];
  readonly effectList: readonly StateEffect<unknown>[];
  readonly effectPayloads: readonly unknown[];
  readonly effectValues: readonly CapturedDocumentValueEffect[];
  readonly scrollIntoView: boolean;
  readonly history: boolean | undefined;
  readonly historyIsolation: 'before' | 'after' | 'full' | undefined;
  readonly userEvent: string | undefined;
  readonly time: number | undefined;
  readonly pasteHistory: PasteImportHistory | undefined;
  readonly normalizationJoin: boolean | undefined;
  readonly authoredImplicitDepth: number | undefined;
}
/** Complete frozen results are the keys; copied fields or markers never confer issuance. */
const issuedEdits = new WeakMap<PreparedDocumentValueEdit, DocumentValueEditIssuance>();
/** Native exact-source effects carry a flat list of newline spellings; retain their values rather than mutable array identity. */
function effectContentsMatch(value: unknown, captured: unknown): boolean {
  if (!Array.isArray(captured)) return Object.is(value, captured);
  if (!Array.isArray(value) || value.length !== captured.length) return false;
  for (let index = 0; index < captured.length; index++)
    if (!Object.hasOwn(value, index) || !Object.is(value[index], captured[index])) return false;
  return true;
}
/** Admit the actual producer result on its captured post-state/profile; publication owners separately verify their current slot membership. */
export function isIssuedDocumentValueEdit(
  state: EditorState,
  profileKey: string,
  edit: PreparedDocumentValueEdit,
  semantics: PreparedDocumentSemantics,
): boolean {
  const issuance = issuedEdits.get(edit);
  const matches =
    issuance !== undefined &&
    issuance.postState === state &&
    edit.postValues.scopes.profileKey === profileKey &&
    (edit.transactions[0] ?? null) === issuance.transaction &&
    (issuance.transaction === null ||
      (issuance.transaction.startState === edit.startState &&
        issuance.transaction.state === issuance.postState &&
        issuance.transaction.changes === issuance.changes &&
        JSON.stringify(issuance.transaction.changes.toJSON()) === issuance.changeContents &&
        issuance.transaction.selection === issuance.selection &&
        selectionMatches(issuance.transaction.newSelection, issuance.selectionContents) &&
        issuance.transaction.effects === issuance.effectList &&
        issuance.transaction.effects.length === issuance.effects.length &&
        issuance.effects.every(
          (effect, index) =>
            Object.hasOwn(
              requireValue(issuance.transaction, 'document-value-edit-issuance').effects,
              index,
            ) &&
            effect ===
              requireValue(issuance.transaction, 'document-value-edit-issuance').effects[index] &&
            effect.value === issuance.effectPayloads[index] &&
            documentValueEffectMatches(
              effect,
              requireValue(issuance.effectValues[index], 'document-value-edit-issuance'),
            ),
        ) &&
        issuance.transaction.scrollIntoView === issuance.scrollIntoView &&
        issuance.transaction.annotation(Transaction.addToHistory) === issuance.history &&
        issuance.transaction.annotation(isolateHistory) === issuance.historyIsolation &&
        issuance.transaction.annotation(Transaction.userEvent) === issuance.userEvent &&
        issuance.transaction.annotation(pasteImportContinuation) === issuance.pasteHistory &&
        Object.is(issuance.transaction.annotation(Transaction.time), issuance.time) &&
        issuance.transaction.annotation(normalizationJoin) === issuance.normalizationJoin &&
        issuance.transaction.annotation(authoredImplicitDepth) ===
          issuance.authoredImplicitDepth)) &&
    selectionMatches(issuance.postState.selection, issuance.selectionContents) &&
    issuance.postState.doc === issuance.document &&
    effectContentsMatch(issuance.postState.field(lineEndings), issuance.endings);
  if (
    !matches &&
    issuance &&
    state === issuance.postState &&
    profileKey === edit.postValues.scopes.profileKey
  )
    issuedEdits.delete(edit);
  return matches && semantics === issuance?.semantics;
}
/** Register one complete actual producer result after construction, retaining every native public payload. */
function registerDocumentValueEdit(
  edit: PreparedDocumentValueEdit,
  semantics: PreparedDocumentSemantics,
): void {
  const transaction = edit.transactions[0] ?? null;
  const postState = transaction?.state ?? edit.startState;
  issuedEdits.set(edit, {
    postState,
    document: postState.doc,
    endings: Object.freeze([...postState.field(lineEndings)]),
    semantics,
    transaction,
    changes: transaction?.changes ?? null,
    changeContents: JSON.stringify(transaction?.changes.toJSON() ?? null),
    selection: transaction?.selection,
    selectionContents: selectionContents(postState.selection),
    effects: Object.freeze([...(transaction?.effects ?? [])]),
    effectList: transaction?.effects ?? Object.freeze([]),
    effectPayloads: Object.freeze(
      (transaction?.effects ?? []).map((effect): unknown => effect.value),
    ),
    effectValues: Object.freeze((transaction?.effects ?? []).map(captureDocumentValueEffect)),
    scrollIntoView: transaction?.scrollIntoView ?? false,
    history: transaction?.annotation(Transaction.addToHistory),
    historyIsolation: transaction?.annotation(isolateHistory),
    userEvent: transaction?.annotation(Transaction.userEvent),
    time: transaction?.annotation(Transaction.time),
    pasteHistory: transaction?.annotation(pasteImportContinuation),
    normalizationJoin: transaction?.annotation(normalizationJoin),
    authoredImplicitDepth: transaction?.annotation(authoredImplicitDepth),
  });
}
/** Publication fence must run against the current state/slot after any caller callback or release; it never refreshes captured authority. */
export function currentDocumentValueEdit(
  state: EditorState,
  context: NativeDocumentValueContext,
  edit: PreparedDocumentValueEdit,
): DocumentValueEditRejection | null {
  const issuance = issuedEdits.get(edit);
  if (
    !issuance ||
    !isIssuedDocumentValueEdit(
      issuance.postState,
      edit.postValues.scopes.profileKey,
      edit,
      issuance.semantics,
    )
  )
    return 'foreign';
  const outcome = currentNativeDocumentValue(state, context, edit.target);
  if (outcome.status === 'rejected') return outcome.reason;
  return state === edit.startState && context.semantics === issuance.semantics
    ? null
    : 'stale-context';
}
/** Validate the genuine target and construct/capture native output together; assembled edit DTOs never enter issuance. */
export function prepareDocumentValueEdit(
  state: EditorState,
  intent: DecodedDocumentValueEdit,
): DocumentValueEditOutcome {
  const prepared = prepareDocumentValueEditInput(state, intent);
  if (prepared.status === 'rejected') return prepared;
  const {
    target,
    replacement,
    preparedValues,
    exact,
    encoded,
    changes,
    selection,
    retainedSelection,
    scopes,
  } = prepared;
  const changed = preparedValues.source !== target.values.scopes.values.source;
  const selectedContents = selectionContents(retainedSelection ? state.selection : selection);
  const provenance = intent.publication?.normalization;
  const reorder = intent.publication?.reorder;
  if (
    intent.publication?.purpose === 'reorder' &&
    (changed || intent.publication.authorship) &&
    !reorder
  )
    return { status: 'rejected', reason: 'unsafe-origin' };
  if (
    reorder &&
    (provenance || intent.publication?.authorship || intent.publication?.purpose !== 'reorder')
  )
    throw new Error('decoded-edit-duplicate-authorship');
  if (provenance && intent.publication?.authorship)
    throw new Error('decoded-edit-duplicate-authorship');
  const normalized = provenance
    ? normalizedAuthorship(state, target, provenance, scopes, changes, encoded)
    : state.field(authorship, false);
  const reordered = reorder
    ? reorderedAuthorship(state, target, reorder, scopes, changes, encoded, {
        range: intent.range,
        text: intent.text,
      })
    : null;
  if (reordered?.status === 'rejected') return reordered;
  const authored =
    provenance && normalized !== state.field(authorship)
      ? installAuthorship.of(requireValue(normalized, 'document-value-edit-issuance'))
      : reordered?.status === 'ready'
        ? installAuthorship.of(reordered.value)
        : intent.publication?.authorship;
  const publication = prepareDocumentValuePublication(
    intent.publication
      ? { ...intent.publication, ...(authored ? { authorship: authored } : {}) }
      : undefined,
    state,
  );
  const authoredPaste = intent.publication?.purpose === 'paste' && publication.effects.length > 0;
  if (exact) delete exact.userEvent;
  const { userEvent, ...nativePublication } = publication;
  const event = userEvent === undefined ? {} : { userEvent };
  const transaction = changed
    ? state.update({
        ...exact,
        changes,
        selection,
        ...nativePublication,
        ...event,
        effects: [...(exact?.effects ?? []), ...publication.effects],
      })
    : retainedSelection && !publication.effects.length && !publication.annotations?.length
      ? null
      : state.update({
          ...(retainedSelection && intent.publication?.purpose === 'typed'
            ? {}
            : { selection: retainedSelection ? state.selection : selection }),
          ...nativePublication,
          ...event,
          annotations:
            intent.publication?.purpose === 'typed'
              ? (publication.annotations ?? [])
              : intent.publication?.pasteHistory
                ? [...(publication.annotations ?? []), Transaction.addToHistory.of(false)]
                : authoredPaste
                  ? isolateHistory.of('full')
                  : [...(publication.annotations ?? []), Transaction.addToHistory.of(false)],
        });
  const postState = transaction?.state ?? state;
  if (
    readExactSource(postState) !== preparedValues.source ||
    !selectionMatches(postState.selection, selectedContents)
  )
    throw new Error('decoded-edit-actual-result-mismatch');
  const postValues = prepareNativeDocumentValues(
    postState,
    scopes,
    target.values.syntax,
    target.values,
  );
  const semantics = prepareDocumentSemantics(
    preparedValues.source,
    preparedValues.kind,
    intent.context.semantics,
    preparedValues,
  );
  const edit: PreparedDocumentValueEdit = Object.freeze({
    startState: state,
    target,
    replacement,
    currentValues: target.values,
    preparedValues,
    postValues,
    semantics,
    transactions: Object.freeze(transaction ? [transaction] : []),
  });
  registerDocumentValueEdit(edit, intent.context.semantics);
  return { status: changed ? 'ready' : 'unchanged', edit };
}
