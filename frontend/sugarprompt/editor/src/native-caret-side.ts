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
/** The painted native caret follows the side of a soft wrap that the collapsed selection belongs to. */
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';

/** A DOM boundary the browser should paint the caret at instead of its current equivalent position. */
interface CaretBoundary {
  readonly node: Node;
  readonly offset: number;
}

/**
 * Align the browser caret with the selection's `assoc` side.
 *
 * At a soft wrap the end of one visual row and the start of the next are the same document
 * offset. CodeMirror records which row the caret belongs to in `assoc`, but it skips rewriting
 * the DOM selection when the new DOM boundary is equivalent to the current one, so the browser
 * keeps painting the caret on the previous row. This owner only corrects that painted row; it
 * never changes the document selection.
 */
class NativeCaretSide {
  private readonly view: EditorView;
  /** Bind one mounted native view. */
  constructor(view: EditorView) {
    this.view = view;
  }
  /** Re-check the painted row only when a collapsed caret changed side without moving. */
  update(update: ViewUpdate): void {
    const main = update.state.selection.main,
      previous = update.startState.selection.main;
    // CodeMirror writes a side-correct DOM boundary whenever the caret moves; only a side-only
    // change can leave the browser painting the old row, so ordinary typing and navigation pay nothing.
    if (
      !update.selectionSet ||
      !main.empty ||
      !main.assoc ||
      !previous.empty ||
      previous.head !== main.head ||
      previous.assoc === main.assoc
    )
      return;
    this.view.requestMeasure({
      key: this,
      read: () => this.misplaced(),
      write: (boundary) => {
        if (!boundary) return;
        const selection = this.view.contentDOM.ownerDocument.getSelection();
        if (selection?.isCollapsed) selection.collapse(boundary.node, boundary.offset);
      },
    });
  }
  /** Return the side-correct DOM boundary when the browser paints the caret on another visual row. */
  private misplaced(): CaretBoundary | null {
    const view = this.view,
      main = view.state.selection.main;
    if (!main.empty || !main.assoc || !view.hasFocus || view.composing || view.compositionStarted)
      return null;
    const selection = view.contentDOM.ownerDocument.getSelection();
    const focus = selection?.focusNode;
    if (!selection?.isCollapsed || !focus || !view.contentDOM.contains(focus)) return null;
    if (view.posAtDOM(focus, selection.focusOffset) !== main.head) return null;
    const painted = selection.getRangeAt(0).getBoundingClientRect(),
      intended = view.coordsAtPos(main.head, main.assoc);
    // Zero-sized native rects occur at element boundaries and carry no row evidence.
    if (!intended || painted.height <= 0) return null;
    const row = Math.min(painted.height, intended.bottom - intended.top) / 2;
    if (Math.abs(painted.top - intended.top) < row) return null;
    const target = view.domAtPos(main.head, main.assoc);
    return target.node === focus && target.offset === selection.focusOffset ? null : target;
  }
}

/** Native extension keeping the painted caret on the visual row its selection side names. */
export const nativeCaretSide = ViewPlugin.fromClass(NativeCaretSide);
