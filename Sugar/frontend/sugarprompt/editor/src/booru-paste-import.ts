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
 * Paste a Danbooru post link as the post's tags.
 *
 * Nothing is inserted while the post is looked up. When the lookup answers, the tags are pasted
 * where the link was pasted, as one ordinary paste, so a single undo removes them and the link
 * never appears. A lookup that finds nothing pastes the link as written. Typing meanwhile moves
 * the paste point with the text; leaving the field drops the paste.
 */
import type { EditorState } from '@codemirror/state';
import type { EditorView, ViewUpdate } from '@codemirror/view';
import type { TagLexicon } from '../../core/src/index.js';
import { documentToSource } from '../../core/src/line-endings.js';
import {
  classifyBooruLink,
  importBooruLink,
  type BooruPostLookup,
} from '../../services/src/booru/links.js';
import { featureEnabled } from '../../services/src/features/profile.js';
import { featureProfile } from './features.js';
import { enteredTextInsertion, prepareNativeIngestionState } from './ingestion.js';
import { dispatchNativeIngestion } from './native-ingestion.js';
import { lineEndings } from './exact-source.js';
import { nativeDocumentValueAt } from './document-values.js';
import { semanticStructures } from './structure.js';

/** One paste waiting for its lookup: where it goes, in document positions, and what was pasted. */
interface PendingPaste {
  readonly abort: AbortController;
  readonly text: string;
  from: number;
  to: number;
}

/** The mounted clipboard plugin's one Danbooru paste at a time. */
export class BooruPasteImport {
  private pending: PendingPaste | null = null;
  private disposed = false;
  private readonly view: EditorView;
  private readonly lookup: () => BooruPostLookup | undefined;
  private readonly lexicon: () => TagLexicon | undefined;

  /** Capture the native view and the host's current lookup and tag knowledge. */
  constructor(
    view: EditorView,
    lookup: () => BooruPostLookup | undefined,
    lexicon: () => TagLexicon | undefined,
  ) {
    this.view = view;
    this.lookup = lookup;
    this.lexicon = lexicon;
  }

  /**
   * Take a plain paste that is a supported link, or return null to leave it to ordinary paste.
   * The returned promise settles when the tags, or the link, have been pasted or the paste was
   * dropped.
   */
  start(text: string): Promise<void> | null {
    this.cancel();
    const state = this.view.state;
    if (!featureEnabled(state.facet(featureProfile), 'danbooru_url_import')) return null;
    const lookup = this.lookup();
    if (
      this.disposed ||
      !lookup ||
      !classifyBooruLink(text) ||
      state.readOnly ||
      this.view.composing ||
      this.view.compositionStarted
    )
      return null;
    const selection = state.selection.main;
    if (!this.withinOneValue(state)) return null;
    const pending: PendingPaste = {
      abort: new AbortController(),
      text,
      from: selection.from,
      to: selection.to,
    };
    this.pending = pending;
    return this.resolve(pending, lookup);
  }

  /** Move the paste point with edits made while waiting; leaving the field drops the paste. */
  update(update: Pick<ViewUpdate, 'transactions' | 'focusChanged'>): void {
    const pending = this.pending;
    if (!pending) return;
    if (update.focusChanged && !this.view.hasFocus) {
      this.cancel();
      return;
    }
    for (const transaction of update.transactions) {
      if (!transaction.docChanged) continue;
      // Text typed at the paste point stays after the pasted tags.
      pending.from = transaction.changes.mapPos(pending.from, -1);
      pending.to = Math.max(pending.from, transaction.changes.mapPos(pending.to, -1));
    }
  }

  /** Drop the waiting paste, if any. */
  cancel(): void {
    const pending = this.pending;
    this.pending = null;
    pending?.abort.abort();
  }

  /** Release the mounted view's only paste owner. */
  destroy(): void {
    this.disposed = true;
    this.cancel();
  }

  /** Paste the tags, or the link when the lookup found none, while the paste is still wanted. */
  private async resolve(pending: PendingPaste, lookup: BooruPostLookup): Promise<void> {
    try {
      const result = await importBooruLink(pending.text, lookup, pending.abort.signal);
      if (result.status === 'cancelled' || !this.wanted(pending)) return;
      this.paste(pending, result.status === 'ready' && result.text ? result.text : pending.text);
    } finally {
      if (this.pending === pending) this.pending = null;
    }
  }

  /**
   * Whether the selection lies inside one prompt value. A paste into a CSV header or across two
   * values is no prompt text, so it stays an ordinary paste and nothing is looked up.
   */
  private withinOneValue(state: EditorState): boolean {
    const prepared = prepareNativeIngestionState(state).at(-1)?.state ?? state;
    const native = prepared.field(semanticStructures).native;
    if (!native) return false;
    const selection = prepared.selection.main;
    return (
      nativeDocumentValueAt(
        prepared,
        {
          values: native.nativeValues,
          semantics: native.documentSemantics,
          enabled: true,
          composing: false,
        },
        { from: selection.from, to: selection.to },
      ).status === 'ready'
    );
  }

  /** Whether the paste can still land: not dropped, and the field still editable and focused. */
  private wanted(pending: PendingPaste): boolean {
    return (
      !this.disposed &&
      this.pending === pending &&
      !pending.abort.signal.aborted &&
      this.view.hasFocus &&
      !this.view.state.readOnly &&
      !this.view.composing &&
      !this.view.compositionStarted &&
      featureEnabled(this.view.state.facet(featureProfile), 'danbooru_url_import')
    );
  }

  /** Paste `text` over the paste range as one ordinary paste. */
  private paste(pending: PendingPaste, text: string): void {
    const state = this.view.state;
    const logical = state.doc.toString(),
      endings = state.field(lineEndings);
    const from = documentToSource(logical, endings, Math.min(pending.from, state.doc.length)),
      to = documentToSource(logical, endings, Math.min(pending.to, state.doc.length));
    dispatchNativeIngestion(
      this.view,
      enteredTextInsertion(state, from, to, text, this.lexicon(), true),
    );
  }
}
