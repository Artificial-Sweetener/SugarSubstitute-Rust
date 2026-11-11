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
/** The editor draws its own caret in every browser. */
import { Prec, type Extension } from '@codemirror/state';
import { EditorView, ViewPlugin, drawSelection, type ViewUpdate } from '@codemirror/view';

/**
 * One full on-and-off caret cycle, in milliseconds.
 *
 * Chromium blinks the native caret with the Windows default caret blink time of 530 ms per phase;
 * the drawn caret keeps that rhythm.
 */
const caretBlinkCycle = 1060;

/**
 * Publish the drawn caret's width in the editor's own CSS pixels.
 *
 * The native caret is one CSS pixel wide before the canvas scale, which leaves it under one device pixel
 * on a zoomed-out canvas; Firefox then snaps it away at many positions. The drawn caret keeps the native
 * width but never becomes narrower than one device pixel, so it stays visible at every zoom.
 */
class CaretWidth {
  private readonly view: EditorView;
  private width = 0;

  /** Publish the width for the view's current scale. */
  constructor(view: EditorView) {
    this.view = view;
    this.publish();
  }

  /** Re-publish when CodeMirror measured a new scale or device pixel ratio. */
  update(update: ViewUpdate): void {
    if (update.geometryChanged) this.publish();
  }

  /** Write the width only when it changed; reading the measured scale costs no layout. */
  private publish(): void {
    const devicePixelRatio = this.view.dom.ownerDocument.defaultView?.devicePixelRatio ?? 1;
    const width = Math.max(1, 1 / (this.view.scaleX * devicePixelRatio));
    if (width === this.width) return;
    this.width = width;
    this.view.dom.style.setProperty('--sugarprompt-caret-width', `${String(width)}px`);
  }
}

/**
 * The drawn caret matches the native one: the text colour, as tall as CodeMirror's caret rectangle for
 * the selection side and centred on the caret position, which puts it on the device pixel column
 * Chromium's native caret snaps to.
 */
const caretAppearance = EditorView.theme({
  // A filled box rather than a border: Chromium rounds border widths to whole device pixels, while the
  // native caret is antialiased at its scaled width.
  '.cm-cursorLayer .cm-cursor': {
    borderLeft: 'none',
    width: 'var(--sugarprompt-caret-width, 1px)',
    marginLeft: 'calc(var(--sugarprompt-caret-width, 1px) / -2)',
    backgroundColor: 'currentColor',
  },
});

/**
 * The browser keeps painting the selection highlight.
 *
 * `drawSelection` replaces the native highlight with rectangles that fill every wrapped row to the
 * content edge, while the native highlight ends with each row's text. The drawn rectangles are hidden
 * and the native highlight is restored with the selection colour the host publishes, which is the
 * colour the native highlight resolved to before the caret was drawn.
 */
const nativeHighlight = Prec.highest(
  EditorView.theme({
    '& .cm-selectionLayer': { display: 'none' },
    '& .cm-content .cm-line ::selection, & .cm-content .cm-line::selection': {
      backgroundColor: 'var(--sugarprompt-selection-background, Highlight) !important',
    },
  }),
);

/**
 * Draw the caret from CodeMirror's own selection.
 *
 * Each engine paints its native caret from its own idea of the selection. Beneath a fractionally scaled
 * canvas Firefox snaps its one-pixel caret away at many sub-pixel positions, and at a soft wrap each
 * engine needs a different trick to paint it on the row the selection's side names. The drawn caret sits
 * at CodeMirror's caret rectangle for the selection head and its side, so every browser shows the same
 * caret at every zoom. As with the native caret, none is drawn while a range is selected. The native
 * selection still carries input and IME; only its caret is hidden.
 */
export const drawnSelection: Extension = [
  drawSelection({ cursorBlinkRate: caretBlinkCycle, drawRangeCursor: false }),
  ViewPlugin.fromClass(CaretWidth),
  caretAppearance,
  nativeHighlight,
];
