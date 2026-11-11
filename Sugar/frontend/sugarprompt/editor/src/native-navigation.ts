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
/** Native arrow commands delegate visual traversal to CodeMirror and full Unicode boundaries to the shared grapheme owner. */
import { EditorSelection, type SelectionRange } from '@codemirror/state';
import { Direction, type EditorView } from '@codemirror/view';
import {
  cursorCharLeft,
  cursorCharRight,
  cursorLineUp,
  cursorLineDown,
} from '@codemirror/commands';
import { moveByGrapheme, moveByGraphemeLine } from './grapheme-motion.js';
import { regionContentCaret } from './region-caret.js';

/** Move or extend the authoritative native selection; horizontal commands retain no caret or source state of their own. */
export function horizontalNavigation(view: EditorView, right: boolean, extend: boolean): boolean {
  if (view.compositionStarted) return false;
  if (!extend && !view.state.selection.main.empty)
    return (right ? cursorCharRight : cursorCharLeft)(view);
  const forward =
    right === (view.textDirectionAt(view.state.selection.main.head) === Direction.LTR);
  return navigateSelection(view, forward, extend, (range) => moveByGrapheme(view, range, forward));
}

/** Native visual rows, wrapping and preferred columns remain CodeMirror-owned; complete character endpoints share the horizontal Unicode contract. */
export function verticalNavigation(view: EditorView, forward: boolean, extend: boolean): boolean {
  if (view.compositionStarted) return false;
  if (!extend && !view.state.selection.main.empty)
    return (forward ? cursorLineDown : cursorLineUp)(view);
  return navigateSelection(view, forward, extend, (range) =>
    moveByGraphemeLine(view, range, forward, extend),
  );
}

/** One native selection transaction preserves directional and undirectional anchors for all installed arrow gestures. */
function navigateSelection(
  view: EditorView,
  forward: boolean,
  extend: boolean,
  move: (range: SelectionRange) => SelectionRange,
): boolean {
  const ranges = view.state.selection.ranges.map((original): SelectionRange => {
    const range =
      extend && original.undirectional && original.head >= original.anchor !== forward
        ? EditorSelection.range(original.head, original.anchor)
        : original;
    const moved = move(range);
    if (!extend) {
      // Separator rows hold no caret: arrows continue into the neighbouring region's content.
      const head = regionContentCaret(view.state, moved.head, forward);
      if (head !== moved.head) return EditorSelection.cursor(head, forward ? 1 : -1);
    }
    return extend
      ? EditorSelection.range(
          range.anchor,
          moved.head,
          moved.goalColumn,
          moved.bidiLevel ?? undefined,
          moved.assoc,
        )
      : moved;
  });
  const selection = EditorSelection.create(ranges, view.state.selection.mainIndex);
  if (selection.eq(view.state.selection, true)) return false;
  view.dispatch({ selection, userEvent: 'select', scrollIntoView: true });
  return true;
}
