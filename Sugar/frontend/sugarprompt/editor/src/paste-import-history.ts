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
/** Accepted paste continuation contributes one bounded capability to native CodeMirror history. */
import { requireValue } from '../../core/src/required.js';
import {
  Annotation,
  Transaction,
  type EditorState,
  type EditorSelection,
  type ChangeSet,
  type Text,
  type StateEffect,
} from '@codemirror/state';
import { isolateHistory } from '@codemirror/commands';
import {
  selectionContents,
  selectionMatches,
  type NativeSelectionContents,
} from './native-selection.js';
import { changesExactSource, lineEndings } from './exact-source.js';
import { authorship } from './authorship.js';
import { editorFreshness } from './freshness.js';
/** Opaque handle retains no source or history stack; private membership carries actual native identity. */
export interface PasteImportHistory {
  readonly identity: symbol;
}
/** Publication distinguishes the accepted literal paste from its sole current replacement. */
export interface PasteImportHistoryIntent {
  readonly handle: PasteImportHistory;
  readonly phase: 'initial' | 'replacement';
}
/** One finite recipe retains the actual accepted native command, original time and current neutral continuation tip. */
interface Receipt {
  readonly origin: EditorState;
  readonly selection: NativeSelectionContents;
  readonly originDocument: Text;
  readonly originEndings: readonly string[];
  readonly originEndingContents: readonly string[];
  outputDocument: Text | null;
  outputEndings: readonly string[] | null;
  endings: readonly string[] | null;
  changes: ChangeSet | null;
  changeContents: string;
  outputSelection: NativeSelectionContents | null;
  effects: readonly StateEffect<unknown>[];
  effectValues: readonly unknown[];
  current: EditorState;
  initial: Transaction | null;
  time: number | null;
  accepted: boolean;
  retired: boolean;
}
const receipts = new WeakMap<PasteImportHistory, Receipt>();
/** Native history and issuer compare this exact immutable capability identity. */
export const pasteImportContinuation = Annotation.define<PasteImportHistory>();
/** Initialize one speculative paste recipe; acceptance still requires the actual dispatched transaction. */
export function preparePasteImportHistory(state: EditorState): PasteImportHistory {
  const handle = Object.freeze({ identity: Symbol() });
  receipts.set(handle, {
    origin: state,
    selection: selectionContents(state.selection),
    originDocument: state.doc,
    originEndings: state.field(lineEndings),
    originEndingContents: Object.freeze(Array.from(state.field(lineEndings))),
    outputDocument: null,
    outputEndings: null,
    endings: null,
    changes: null,
    changeContents: '',
    outputSelection: null,
    effects: [],
    effectValues: [],
    current: state,
    initial: null,
    time: null,
    accepted: false,
    retired: false,
  });
  return handle;
}
/** Resolve native history metadata before transaction construction; timestamps come only from the actual accepted paste. */
export function pasteImportAnnotations(
  state: EditorState,
  intent: PasteImportHistoryIntent,
): readonly Annotation<unknown>[] {
  const receipt = receipts.get(intent.handle);
  if (!receipt || !historyReceiptCurrent(intent.handle) || state.readOnly)
    throw new Error('paste-import-history-retired');
  if (intent.phase === 'initial') {
    if (
      receipt.initial ||
      state.field(editorFreshness) !== receipt.origin.field(editorFreshness) ||
      state.doc !== receipt.originDocument ||
      state.field(lineEndings) !== receipt.originEndings ||
      !selectionMatches(state.selection, receipt.selection)
    )
      throw new Error('paste-import-history-initial');
    return [isolateHistory.of('before'), pasteImportContinuation.of(intent.handle)];
  }
  if (!receipt.initial || receipt.current !== state || !historyReceiptCurrent(intent.handle))
    throw new Error('paste-import-history-current');
  return [
    isolateHistory.of(changesExactSource(receipt.initial) ? 'after' : 'full'),
    pasteImportContinuation.of(intent.handle),
    Transaction.time.of(requireValue(receipt.time, 'paste-import-history')),
  ];
}
/** Capture the exact already-prepared native command before synchronous dispatch observers may run. */
export function capturePasteImportHistory(
  handle: PasteImportHistory,
  transaction: Transaction,
): boolean {
  const receipt = receipts.get(handle);
  if (
    !receipt ||
    !historyReceiptCurrent(handle) ||
    receipt.initial ||
    transaction.annotation(pasteImportContinuation) !== handle ||
    transaction.annotation(isolateHistory) !== 'before' ||
    transaction.annotation(Transaction.userEvent) !== 'input.paste' ||
    transaction.startState.field(editorFreshness) !== receipt.origin.field(editorFreshness) ||
    transaction.startState.doc !== receipt.originDocument ||
    transaction.startState.field(lineEndings) !== receipt.originEndings ||
    !selectionMatches(transaction.startState.selection, receipt.selection)
  )
    return false;
  receipt.initial = transaction;
  receipt.outputDocument = transaction.state.doc;
  receipt.outputEndings = transaction.state.field(lineEndings);
  receipt.changes = transaction.changes;
  receipt.changeContents = JSON.stringify(transaction.changes.toJSON());
  receipt.outputSelection = selectionContents(transaction.newSelection);
  receipt.effects = Object.freeze([...transaction.effects]);
  receipt.effectValues = Object.freeze(transaction.effects.map((effect): unknown => effect.value));
  receipt.endings = Object.freeze(Array.from(transaction.state.field(lineEndings)));
  receipt.time = requireValue(transaction.annotation(Transaction.time), 'paste-import-history');
  return true;
}
/** Accept only that captured actual command after successful dispatch and unchanged original metadata. */
export function acceptPasteImportHistory(
  handle: PasteImportHistory,
  transaction: Transaction,
  state: EditorState,
): boolean {
  const receipt = receipts.get(handle);
  if (
    !receipt ||
    receipt.accepted ||
    receipt.initial !== transaction ||
    transaction.state !== state ||
    !historyReceiptCurrent(handle)
  )
    return false;
  receipt.accepted = true;
  receipt.current = state;
  return true;
}
/** Follow actual neutral native links; navigation, history, source, configuration or permission changes permanently retire the capability. */
export function followPasteImportHistory(
  handle: PasteImportHistory,
  transaction: Transaction,
): boolean {
  const receipt = receipts.get(handle);
  if (!receipt || receipt.retired || !receipt.initial || !receipt.accepted) return false;
  if (
    !historyReceiptCurrent(handle) ||
    transaction.startState !== receipt.current ||
    transaction.state.doc !== receipt.outputDocument ||
    transaction.state.field(lineEndings) !== receipt.outputEndings ||
    changesExactSource(transaction) ||
    transaction.selection ||
    transaction.reconfigured ||
    transaction.state.readOnly ||
    transaction.annotation(Transaction.addToHistory) !== false ||
    transaction.annotation(Transaction.userEvent) !== undefined ||
    transaction.startState.field(authorship) !== transaction.state.field(authorship) ||
    transaction.startState.field(editorFreshness) !== transaction.state.field(editorFreshness)
  ) {
    receipt.retired = true;
    return false;
  }
  receipt.current = transaction.state;
  return true;
}
/** Determine genuine replacement membership at the native history callback without granting detached annotations authority. */
export function joinsPasteImportHistory(transaction: Transaction): boolean {
  const handle = transaction.annotation(pasteImportContinuation),
    receipt = handle && receipts.get(handle);
  return (
    !!receipt &&
    !receipt.retired &&
    !!receipt.initial &&
    receipt.accepted &&
    receipt.current === transaction.startState &&
    transaction.annotation(isolateHistory) === 'after' &&
    transaction.annotation(Transaction.userEvent) === undefined &&
    Object.is(transaction.annotation(Transaction.time), receipt.time) &&
    historyReceiptCurrent(requireValue(handle, 'paste-import-history'))
  );
}
/** Release all retained native state on completion, cancellation or view disposal. */
export function retirePasteImportHistory(handle: PasteImportHistory): void {
  receipts.delete(handle);
}
/** A no-source literal acceptance starts a new import event; genuine initial source edits alone receive native continuation joining. */
export function pasteImportUserEvent(intent: PasteImportHistoryIntent): 'input.paste' | undefined {
  const receipt = receipts.get(intent.handle);
  if (!receipt || !historyReceiptCurrent(intent.handle))
    throw new Error('paste-import-history-retired');
  return intent.phase === 'initial' || !receipt.initial || !changesExactSource(receipt.initial)
    ? 'input.paste'
    : undefined;
}
/** The sole receipt fence binds actual original, accepted and neutral native Text/endings identities and complete selection observations; observed substitution retires permanently. */
function historyReceiptCurrent(handle: PasteImportHistory): boolean {
  const receipt = receipts.get(handle),
    initial = receipt?.initial;
  if (!receipt || receipt.retired) return false;
  const current =
    receipt.origin.doc === receipt.originDocument &&
    receipt.origin.field(lineEndings) === receipt.originEndings &&
    receipt.originEndings.length === receipt.originEndingContents.length &&
    receipt.originEndingContents.every(
      (ending, index) =>
        Object.hasOwn(receipt.originEndings, index) && receipt.originEndings[index] === ending,
    ) &&
    selectionMatches(receipt.origin.selection, receipt.selection) &&
    (!initial ||
      (initial.startState.doc === receipt.originDocument &&
        initial.startState.field(lineEndings) === receipt.originEndings &&
        selectionMatches(initial.startState.selection, receipt.selection) &&
        initial.state.doc === receipt.outputDocument &&
        initial.state.field(lineEndings) === receipt.outputEndings &&
        (!receipt.accepted ||
          (receipt.current.doc === receipt.outputDocument &&
            receipt.current.field(lineEndings) === receipt.outputEndings &&
            receipt.outputSelection !== null &&
            selectionMatches(receipt.current.selection, receipt.outputSelection))) &&
        initial.annotation(pasteImportContinuation) === handle &&
        initial.annotation(isolateHistory) === 'before' &&
        initial.annotation(Transaction.userEvent) === 'input.paste' &&
        Object.is(initial.annotation(Transaction.time), receipt.time) &&
        initial.changes === receipt.changes &&
        JSON.stringify(initial.changes.toJSON()) === receipt.changeContents &&
        receipt.outputSelection !== null &&
        selectionMatches(initial.newSelection, receipt.outputSelection) &&
        initial.effects.length === receipt.effects.length &&
        receipt.effects.every(
          (effect, index) =>
            Object.hasOwn(initial.effects, index) &&
            initial.effects[index] === effect &&
            Object.is(effect.value, receipt.effectValues[index]),
        ) &&
        receipt.endings !== null &&
        receipt.endings.length === initial.state.field(lineEndings).length &&
        receipt.endings.every(
          (ending, index) =>
            Object.hasOwn(initial.state.field(lineEndings), index) &&
            initial.state.field(lineEndings)[index] === ending,
        )));
  if (!current) receipt.retired = true;
  return current;
}
/** Observe the finite accepted receipt without exposing native timestamps or granting historical source authority. */
export function currentPasteImportHistory(handle: PasteImportHistory, state: EditorState): boolean {
  const receipt = receipts.get(handle);
  return (
    !!receipt && receipt.accepted && receipt.current === state && historyReceiptCurrent(handle)
  );
}
/** A source-unchanged literal acceptance restores its original native directional selection when the new import event is undone. */
export function pasteImportOriginalSelection(transaction: Transaction): EditorSelection | null {
  const handle = transaction.annotation(pasteImportContinuation),
    receipt = handle && receipts.get(handle);
  return receipt &&
    receipt.accepted &&
    receipt.initial &&
    !changesExactSource(receipt.initial) &&
    receipt.current === transaction.startState &&
    transaction.annotation(isolateHistory) === 'full' &&
    transaction.annotation(Transaction.userEvent) === 'input.paste' &&
    Object.is(transaction.annotation(Transaction.time), receipt.time) &&
    historyReceiptCurrent(requireValue(handle, 'paste-import-history'))
    ? receipt.origin.selection
    : null;
}
