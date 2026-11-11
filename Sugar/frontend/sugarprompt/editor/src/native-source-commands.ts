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
/** One finite native source-command adapter owns exact/logical editing, prepared reservations, directional selection and history delegation. */
import { EditorSelection } from '@codemirror/state';
import { redo, undo } from '@codemirror/commands';
import type { EditorView } from '@codemirror/view';
import type { TagLexicon } from '../../core/src/index.js';
import { documentToSource, sourceToDocument } from '../../core/src/line-endings.js';
import type { PromptEditorApi } from './contracts.js';
import { exactReplacement, lineEndings } from './exact-source.js';
import { enteredTextInsertion } from './ingestion.js';
import { dispatchNativeIngestion } from './native-ingestion.js';
import { PreparedSourceEdits } from './prepared-edits.js';
import { nativeClipboard } from './native-clipboard.js';
import { nativeCompositionStarted } from './input-settlement.js';
import type { FocusHandoff } from './focus-handoff-contracts.js';

/** One finite command boundary delegates insertion, exact edits, selection and history to the mounted CodeMirror view. */
export interface NativeSourceCommands {
  readonly api: Pick<
    PromptEditorApi,
    | 'insertText'
    | 'clipboard'
    | 'selectSource'
    | 'replaceSource'
    | 'prepareSourceEdit'
    | 'applySourceEdit'
    | 'reserveSourceEdit'
    | 'applyReservedSourceEdit'
    | 'undo'
    | 'redo'
  >;
  /** Retire outstanding source-free handles before native view teardown. */
  dispose(): void;
}

/** Native intent admission separates inert continuation validation from accepted source/selection supersession. */
export interface NativeSourceIntent {
  /** Reject foreign or stale handoffs before any source handle is consumed. */
  validate(continuation: FocusHandoff): boolean;
  /** Admit only a prepared operation, preserving its current continuation when supplied. */
  accept(admission: 'selection' | 'source', continuation?: FocusHandoff): void;
}

/** The native surface supplies its current view; commands never retain a replaced platform input lifetime. */
export function nativeSourceCommands(
  getView: () => EditorView,
  lexicon: () => TagLexicon | undefined,
  alive: () => void,
  hostIntent: NativeSourceIntent,
): NativeSourceCommands {
  const prepared = new PreparedSourceEdits();
  return {
    api: {
      /** Standard host and menu clipboard commands delegate to the mounted finite native owner. */
      clipboard(action) {
        alive();
        const view = getView();
        if (action !== 'copy' && action !== 'cut' && action !== 'paste')
          throw new TypeError('Invalid clipboard action');
        const clipboard = view.plugin(nativeClipboard);
        if (!clipboard) throw new Error('Native clipboard owner unavailable');
        return clipboard.execute(action);
      },
      /** Bulk insertion shares rich/raw ingestion and CodeMirror's current selection. */
      insertText(text, continuation) {
        alive();
        const view = getView();
        if (view.state.readOnly || nativeCompositionStarted(view)) return false;
        if (continuation !== undefined && !hostIntent.validate(continuation)) return false;
        const logical = view.state.doc.toString(),
          endings = view.state.field(lineEndings),
          selection = view.state.selection.main;
        const insertion = enteredTextInsertion(
          view.state,
          documentToSource(logical, endings, selection.from),
          documentToSource(logical, endings, selection.to),
          text,
          lexicon(),
        );
        hostIntent.accept('source', continuation);
        return dispatchNativeIngestion(view, insertion);
      },
      /** Source selection delegates caret/history authority to the native transaction owner. */
      selectSource(anchor, head = anchor) {
        alive();
        const view = getView();
        const logical = view.state.doc.toString(),
          endings = view.state.field(lineEndings);
        const selection = EditorSelection.single(
          sourceToDocument(logical, endings, anchor),
          sourceToDocument(logical, endings, head),
        );
        hostIntent.accept('selection');
        view.dispatch({
          selection,
          scrollIntoView: true,
          userEvent: 'select',
        });
      },
      /** Source edits retain the single native transaction/history owner. */
      replaceSource(from, to, source, continuation) {
        alive();
        const view = getView();
        if (view.state.readOnly || nativeCompositionStarted(view)) return false;
        if (continuation !== undefined && !hostIntent.validate(continuation)) return false;
        const replacement = exactReplacement(view.state, from, to, source);
        hostIntent.accept('source', continuation);
        view.dispatch(replacement);
        return true;
      },
      /** Prepare source-free handles without dispatching or moving native selection. */
      prepareSourceEdit(from, to, source) {
        alive();
        const view = getView();
        return prepared.prepare(view.state, nativeCompositionStarted(view), from, to, source);
      },
      /** Validate freshness immediately before one native history transaction. */
      applySourceEdit(command, continuation) {
        alive();
        const view = getView();
        if (continuation !== undefined && !hostIntent.validate(continuation))
          return { status: 'rejected', reason: 'stale-context' };
        const edit = prepared.take(view.state, nativeCompositionStarted(view), command);
        if (edit.status !== 'ready') return edit;
        hostIntent.accept('source', continuation);
        view.dispatch(edit.transaction);
        return { status: 'applied' };
      },
      /** Capture a source-current exact range while the host prepares a replacement. */
      reserveSourceEdit(from, to) {
        alive();
        const view = getView();
        return prepared.reserve(view.state, nativeCompositionStarted(view), from, to);
      },
      /** A current host result contributes one ordinary native history transaction. */
      applyReservedSourceEdit(command, source, continuation) {
        alive();
        const view = getView();
        if (continuation !== undefined && !hostIntent.validate(continuation))
          return { status: 'rejected', reason: 'stale-context' };
        const edit = prepared.takeReserved(
          view.state,
          nativeCompositionStarted(view),
          command,
          source,
        );
        if (edit.status !== 'ready') return edit;
        hostIntent.accept('source', continuation);
        view.dispatch(edit.transaction);
        return { status: 'applied' };
      },
      /** Native composition excludes semantic history commands. */
      undo() {
        alive();
        hostIntent.accept('source');
        const view = getView();
        return !nativeCompositionStarted(view) && undo(view);
      },
      /** Redo shares the same native composition and lifetime boundary. */
      redo() {
        alive();
        hostIntent.accept('source');
        const view = getView();
        return !nativeCompositionStarted(view) && redo(view);
      },
    },
    /** Release the sole prepared-command registry; CodeMirror owns document and history disposal. */
    dispose: () => prepared.dispose(),
  };
}
