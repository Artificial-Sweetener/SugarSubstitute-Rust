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
/** One mounted native paste lifetime publishes a literal immediately and replaces only its still-current admitted value. */
import { requireValue } from '../../core/src/required.js';
import { Transaction, type EditorState } from '@codemirror/state';
import type { EditorView, ViewUpdate } from '@codemirror/view';
import type { TagLexicon, SourceRange } from '../../core/src/index.js';
import { sourceToDocument } from '../../core/src/line-endings.js';
import {
  classifyBooruLink,
  importBooruLink,
  type BooruPostLookup,
} from '../../services/src/booru/links.js';
import { featureEnabled } from '../../services/src/features/profile.js';
import { featureProfile } from './features.js';
import { clipboardPaste } from './clipboard.js';
import {
  enteredTextInsertion,
  prepareNativeIngestionState,
  type PreparedNativeIngestion,
} from './ingestion.js';
import { dispatchNativeIngestion } from './native-ingestion.js';
import { lineEndings, readExactSource } from './exact-source.js';
import { nativeDocumentValueAt } from './document-values.js';
import { semanticStructures } from './structure.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { isIssuedDocumentValueEdit } from './document-value-edit-issuance.js';
import {
  selectionContents,
  selectionMatches,
  type NativeSelectionContents,
} from './native-selection.js';
import {
  currentPasteImportHistory,
  preparePasteImportHistory,
  acceptPasteImportHistory,
  capturePasteImportHistory,
  followPasteImportHistory,
  retirePasteImportHistory,
  type PasteImportHistory,
} from './paste-import-history.js';
/** Current accepted URL bounds and observations are retained only until this one request settles or retires. */
interface PendingPaste {
  readonly abort: AbortController;
  readonly history: PasteImportHistory;
  readonly initial: PreparedNativeIngestion;
  readonly transaction: Transaction;
  accepted: boolean;
  readonly range: SourceRange;
  readonly literal: string;
  readonly freshness: EditorFreshness;
  readonly selection: NativeSelectionContents;
  state: EditorState;
}
/** The mounted clipboard plugin delegates lookup cancellation and accepted-paste freshness without another editable buffer. */
export class BooruPasteImport {
  private pending: PendingPaste | null = null;
  private disposed = false;
  private readonly view: EditorView;
  private readonly lookup: () => BooruPostLookup | undefined;
  private readonly lexicon: () => TagLexicon | undefined;
  /** Capture actual native view and injected current host lookup/knowledge; no networking is implemented here. */
  constructor(
    view: EditorView,
    lookup: () => BooruPostLookup | undefined,
    lexicon: () => TagLexicon | undefined,
  ) {
    this.view = view;
    this.lookup = lookup;
    this.lexicon = lexicon;
  }
  /** Accept supported ordinary plain paste synchronously; unsupported or disabled requests stay with the ordinary clipboard owner. */
  start(text: string): Promise<void> | null {
    const state = this.view.state;
    this.cancel();
    if (this.view.state !== state) return Promise.resolve();
    if (!featureEnabled(state.facet(featureProfile), 'danbooru_url_import')) return null;
    const lookup = this.lookup();
    if (this.view.state !== state) return Promise.resolve();
    if (
      this.disposed ||
      !lookup ||
      !classifyBooruLink(text) ||
      this.view.state.readOnly ||
      this.view.composing ||
      this.view.compositionStarted
    )
      return null;
    const lexicon = this.lexicon();
    if (this.view.state !== state) return Promise.resolve();
    const preparation = prepareNativeIngestionState(state);
    const prepared = preparation.at(-1)?.state ?? state;
    const native = requireValue(prepared.field(semanticStructures).native, 'booru-paste-import');
    const admitted = nativeDocumentValueAt(
      prepared,
      {
        values: native.nativeValues,
        semantics: native.documentSemantics,
        enabled: true,
        composing: false,
      },
      { from: prepared.selection.main.from, to: prepared.selection.main.to },
    );
    const history = preparePasteImportHistory(state);
    let initial: PreparedNativeIngestion | null;
    try {
      const command = clipboardPaste(prepared, { kind: 'plain', text }, lexicon, {
        handle: history,
        phase: 'initial',
      });
      initial = command
        ? Object.freeze({
            ...command,
            startState: state,
            transactions: Object.freeze([...preparation, ...command.transactions]),
          })
        : null;
    } catch (cause) {
      retirePasteImportHistory(history);
      throw cause;
    }
    const transaction = initial?.edit?.transactions[0] ?? initial?.transactions.at(-1);
    const range = initial?.acceptedRange;
    if (!initial || !transaction || !range || !capturePasteImportHistory(history, transaction)) {
      retirePasteImportHistory(history);
      return Promise.resolve();
    }
    const postState = transaction.state;
    const pending: PendingPaste = {
      abort: new AbortController(),
      history,
      initial,
      transaction,
      accepted: false,
      range,
      literal: readExactSource(postState).slice(range.from, range.to),
      freshness: postState.field(editorFreshness),
      selection: selectionContents(postState.selection),
      state: postState,
    };
    this.pending = pending;
    try {
      if (!dispatchNativeIngestion(this.view, initial)) {
        if (this.pending === pending) this.cancel();
        return Promise.resolve();
      }
    } catch (cause) {
      if (this.pending === pending) this.cancel();
      throw cause;
    }
    if (admitted.status !== 'ready' || !pending.accepted || !this.current(pending)) {
      if (this.pending === pending) this.cancel();
      return Promise.resolve();
    }
    return this.resolve(pending, text, lookup);
  }
  /** Observe actual continuous native transactions; source/context/navigation/composition/focus retirement cannot be reversed by equal bytes. */
  update(update: Pick<ViewUpdate, 'transactions' | 'focusChanged'>): void {
    if (!this.pending) return;
    if (
      (update.focusChanged && !this.view.hasFocus) ||
      this.view.composing ||
      this.view.compositionStarted
    ) {
      this.cancel();
      return;
    }
    for (const transaction of update.transactions) {
      const pending = this.pending;
      if (!pending) return;
      if (!pending.accepted) {
        if (transaction === pending.transaction) {
          if (!acceptPasteImportHistory(pending.history, transaction, transaction.state)) {
            this.cancel();
            return;
          }
          pending.accepted = true;
        } else {
          const index = pending.initial.transactions.indexOf(transaction),
            command = pending.initial.transactions.indexOf(pending.transaction);
          if (index < 0 || index >= command) {
            this.cancel();
            return;
          }
          continue;
        }
      } else if (!followPasteImportHistory(pending.history, transaction)) {
        this.cancel();
        return;
      }
      pending.state = transaction.state;
    }
  }
  /** Retire one request and its history capability; ignored-abort providers are still prevented from publishing. */
  cancel(): void {
    const pending = this.pending;
    this.pending = null;
    if (pending) {
      pending.abort.abort();
      retirePasteImportHistory(pending.history);
    }
  }
  /** Release the mounted view's only request owner. */
  destroy(): void {
    this.disposed = true;
    this.cancel();
  }
  /** Compare all live native premises and original issued payload before preparing any replacement. */
  private current(pending: PendingPaste): boolean {
    const state = this.view.state;
    return (
      !this.disposed &&
      featureEnabled(state.facet(featureProfile), 'danbooru_url_import') &&
      this.view.hasFocus &&
      this.pending === pending &&
      !pending.abort.signal.aborted &&
      state === pending.state &&
      currentPasteImportHistory(pending.history, state) &&
      state.field(editorFreshness) === pending.freshness &&
      !state.readOnly &&
      !this.view.composing &&
      !this.view.compositionStarted &&
      selectionMatches(state.selection, pending.selection) &&
      (!pending.initial.edit ||
        isIssuedDocumentValueEdit(
          pending.initial.edit.transactions[0]?.state ?? pending.initial.edit.startState,
          pending.initial.edit.postValues.scopes.profileKey,
          pending.initial.edit,
          requireValue(
            pending.initial.edit.startState.field(semanticStructures).native,
            'booru-paste-import',
          ).documentSemantics,
        )) &&
      readExactSource(state).slice(pending.range.from, pending.range.to) === pending.literal
    );
  }
  /** External causes remain observable; every typed unavailability keeps the literal and every terminal path releases native receipts. */
  private async resolve(
    pending: PendingPaste,
    text: string,
    lookup: BooruPostLookup,
  ): Promise<void> {
    try {
      const result = await importBooruLink(text, lookup, pending.abort.signal);
      if (result.status !== 'ready' || !result.text || !this.current(pending)) return;
      for (const transaction of prepareNativeIngestionState(this.view.state)) {
        if (!this.current(pending)) return;
        this.view.dispatch(transaction);
      }
      if (!this.current(pending)) return;
      const state = this.view.state,
        native = state.field(semanticStructures).native;
      if (!native) return;
      const logical = state.doc.toString(),
        endings = state.field(lineEndings);
      const admission = nativeDocumentValueAt(
        state,
        {
          values: native.nativeValues,
          semantics: native.documentSemantics,
          enabled: true,
          composing: false,
        },
        {
          from: sourceToDocument(logical, endings, pending.range.from),
          to: sourceToDocument(logical, endings, pending.range.to),
        },
      );
      if (admission.status !== 'ready') return;
      const lexicon = this.lexicon();
      if (!this.current(pending)) return;
      const plan = enteredTextInsertion(
        state,
        pending.range.from,
        pending.range.to,
        result.text,
        lexicon,
        { handle: pending.history, phase: 'replacement' },
      );
      if (this.current(pending)) dispatchNativeIngestion(this.view, plan);
    } finally {
      if (this.pending === pending) this.cancel();
    }
  }
}
