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
/** Native DOM event boundaries delegate source policy and observe composition without taking ownership of input. */
import { EditorView } from '@codemirror/view';
import type { Extension } from '@codemirror/state';
import { documentToSource } from '../../core/src/line-endings.js';
import { nativeClipboard } from './native-clipboard.js';
import { exactReplacement, lineEndings } from './exact-source.js';

/** Install one view-owned input boundary; event callbacks observe the currently mounted editor. */
export function nativeInputEvents(
  getView: () => EditorView,
  publish: () => void,
  isAlive: () => boolean,
): Extension {
  return EditorView.domEventHandlers({
    /** Copy, cut and paste events go to the prompt's one edit-command owner. */
    copy(event) {
      return getView().plugin(nativeClipboard)?.fromCopyEvent(event, false) ?? false;
    },
    cut(event) {
      return getView().plugin(nativeClipboard)?.fromCopyEvent(event, true) ?? false;
    },
    paste(event) {
      return getView().plugin(nativeClipboard)?.fromPasteEvent(event) ?? false;
    },
    /** Native composition owns preedit; publication observes its lifecycle. */
    compositionstart() {
      if (!isAlive()) return false;
      getView().plugin(nativeClipboard)?.cancel();
      publish();
      return false;
    },
    /** A microtask observes native committed state without a settling timer. */
    compositionend() {
      queueMicrotask(publish);
      return false;
    },
    /**
     * Typing over a selection replaces it through the editor rather than native editing: Chrome
     * merges the zero-width text of non-editable markers it deletes into the typed text node.
     */
    beforeinput(event) {
      const editor = getView();
      if (
        event.inputType !== 'insertText' ||
        !event.data ||
        event.isComposing ||
        editor.composing ||
        editor.state.readOnly ||
        editor.state.selection.ranges.every((range) => range.empty)
      )
        return false;
      event.preventDefault();
      editor.dispatch(editor.state.replaceSelection(event.data), {
        userEvent: 'input.type',
        scrollIntoView: true,
      });
      return true;
    },
    /** Reject unsupported external drag formats at the editor boundary. */
    drop(event) {
      const editor = getView();
      if (!event.dataTransfer) return false;
      const types = [...event.dataTransfer.types];
      if (
        types.some((type) => type !== 'text/plain') ||
        editor.state.readOnly ||
        editor.composing
      ) {
        event.preventDefault();
        return true;
      }
      // Empty and virtualized lines need CodeMirror's supported estimated boundary.
      const position = editor.posAtCoords(
        {
          x: event.clientX,
          y: event.clientY,
        },
        false,
      );
      if (position === null) {
        event.preventDefault();
        return true;
      }
      const logical = editor.state.doc.toString(),
        endings = editor.state.field(lineEndings);
      const sourcePosition = documentToSource(logical, endings, position);
      editor.dispatch({
        ...exactReplacement(
          editor.state,
          sourcePosition,
          sourcePosition,
          event.dataTransfer.getData('text/plain'),
        ),
        userEvent: 'input.drop',
      });
      event.preventDefault();
      return true;
    },
  });
}
