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
/** Clipboard source fidelity, including a private format surviving platform plain-text newline conversion. */
import { EditorView } from '@codemirror/view';
import type { PasteImportHistoryIntent } from './paste-import-history.js';
import type { EditorState, TransactionSpec } from '@codemirror/state';
import { isolateHistory } from '@codemirror/commands';
import type { TagLexicon } from '../../core/src/index.js';
import type { ClipboardTransfer } from '../../services/src/clipboard/contracts.js';
import { enteredTextInsertion, type PreparedNativeIngestion } from './ingestion.js';
import { dispatchNativeIngestion } from './native-ingestion.js';
import { documentToSource } from '../../core/src/line-endings.js';
import { exactReplacement, lineEndings, readExactSource } from './exact-source.js';

/** Native clipboard events and the asynchronous browser adapter share this private source identity. */
export const exactClipboardMime = 'application/x-sugarprompt-source';

/** Snapshot a native event's transfer without retaining its short-lived DataTransfer or interpreting HTML. */
export function eventClipboardTransfer(data: DataTransfer): ClipboardTransfer {
  const exact = [exactClipboardMime, `web ${exactClipboardMime}`].find((type) =>
    data.types.includes(type),
  );
  return {
    kind: exact ? 'exact' : 'plain',
    text: exact
      ? data.getData(exact)
      : data.types.includes('text/plain')
        ? data.getData('text/plain')
        : '',
  };
}

/** Capture one nonempty exact-source UTF-16 selection from CodeMirror; no selection leaves the clipboard unchanged. */
export function clipboardSelection(
  state: EditorState,
): { from: number; to: number; text: string } | undefined {
  const selection = state.selection.main;
  if (selection.empty) return undefined;
  const logical = state.doc.toString(),
    endings = state.field(lineEndings);
  const from = documentToSource(logical, endings, selection.from),
    to = documentToSource(logical, endings, selection.to);
  return { from, to, text: readExactSource(state).slice(from, to) };
}

/** Keyboard and menu cuts use the same exact-source transaction and isolated native history event. */
export function clipboardCut(state: EditorState): TransactionSpec | null {
  if (state.readOnly) return null;
  const selected = clipboardSelection(state);
  return selected
    ? {
        ...exactReplacement(state, selected.from, selected.to, ''),
        annotations: isolateHistory.of('full'),
        userEvent: 'delete.cut',
      }
    : null;
}

/** One ingestion owner handles both clipboard paths; empty transfers leave the selected source intact. */
export function clipboardPaste(
  state: EditorState,
  transfer: ClipboardTransfer,
  lexicon?: TagLexicon,
  history?: PasteImportHistoryIntent,
): PreparedNativeIngestion | null {
  if (state.readOnly || !transfer.text) return null;
  const logical = state.doc.toString(),
    endings = state.field(lineEndings),
    selection = state.selection.main;
  const from = documentToSource(logical, endings, selection.from),
    to = documentToSource(logical, endings, selection.to);
  if (transfer.kind === 'plain')
    return enteredTextInsertion(state, from, to, transfer.text, lexicon, history);
  const transaction = state.update({
    ...exactReplacement(state, from, to, transfer.text),
    annotations: isolateHistory.of('full'),
    userEvent: 'input.paste',
  });
  return Object.freeze({
    startState: state,
    transactions: Object.freeze([transaction]),
    edit: null,
  });
}

/** Write exact source to native clipboard events; cut participates in CodeMirror history. */
export function copySource(view: EditorView, event: ClipboardEvent, cut: boolean): boolean {
  if (!event.clipboardData) return false;
  const selected = clipboardSelection(view.state);
  if (!selected || (cut && (view.state.readOnly || view.composing || view.compositionStarted))) {
    event.preventDefault();
    return true;
  }
  event.clipboardData.clearData();
  event.clipboardData.setData('text/plain', selected.text);
  event.clipboardData.setData(exactClipboardMime, selected.text);
  const deletion = cut ? clipboardCut(view.state) : null;
  if (deletion) view.dispatch(deletion);
  event.preventDefault();
  return true;
}

/** Internal transfer retains exact source; external plain text follows rich/raw editing policy. Composition/read-only reject mutation. */
export function pasteSource(
  view: EditorView,
  event: ClipboardEvent,
  lexicon?: TagLexicon,
): boolean {
  if (view.state.readOnly || view.composing || view.compositionStarted) {
    event.preventDefault();
    return true;
  }
  if (!event.clipboardData) return false;
  const insertion = clipboardPaste(
    view.state,
    eventClipboardTransfer(event.clipboardData),
    lexicon,
  );
  if (insertion) dispatchNativeIngestion(view, insertion);
  event.preventDefault();
  return true;
}
