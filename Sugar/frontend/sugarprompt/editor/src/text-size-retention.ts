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
 * Keep an editor's measured text size while only the scale it is shown at changes.
 *
 * CodeMirror measures its line height and character width again whenever the scale of an ancestor
 * transform changes, and for a prompt it does so by inserting a probe line into the document. A
 * graph zoom changes that scale on every step, and each inserted probe makes the host restyle the
 * whole node around the editor. The text itself has not changed: its size in the editor's own units
 * is the same at every zoom. This owner answers a repeated measurement from the last one, scaled to
 * the current zoom, until the font the editor computes or the document's loaded fonts change.
 */
import type { EditorView } from '@codemirror/view';

/** What CodeMirror's document view reports for its text, in on-screen pixels. */
interface TextSize {
  lineHeight: number;
  charWidth: number;
  textHeight: number;
}

/** The slice of CodeMirror's document view this owner wraps. It is not part of the public API. */
interface MeasuringDocumentView {
  measureTextSize(): TextSize;
}

/** Counts completed font loads per document, so every editor in it measures afresh afterwards. */
const fontLoads = new WeakMap<Document, { count: number }>();

function loadedFonts(document: Document): { count: number } {
  let loads = fontLoads.get(document);
  if (!loads) {
    const counted = { count: 0 };
    loads = counted;
    fontLoads.set(document, counted);
    document.fonts?.addEventListener?.('loadingdone', () => {
      counted.count += 1;
    });
  }
  return loads;
}

/** Name everything that decides how large the editor's text is drawn. */
function fontSignature(view: EditorView, loads: { count: number }): string {
  const style = view.dom.ownerDocument.defaultView?.getComputedStyle(view.contentDOM);
  return [
    loads.count,
    style?.fontFamily,
    style?.fontSize,
    style?.fontWeight,
    style?.fontStyle,
    style?.fontStretch,
    style?.letterSpacing,
    style?.lineHeight,
  ].join('|');
}

/** Whether `value` is a measurement this owner can scale. */
function usable(value: TextSize, scaleX: number, scaleY: number): boolean {
  return (
    scaleX > 0 &&
    scaleY > 0 &&
    [value.lineHeight, value.charWidth, value.textHeight].every(
      (size) => Number.isFinite(size) && size > 0,
    )
  );
}

/**
 * Answer `view`'s repeated text measurements from its last one while its font is unchanged.
 * Returns false, changing nothing, when the editor does not expose the measurement to wrap.
 */
export function retainTextSize(view: EditorView): boolean {
  const documentView = (view as unknown as { docView?: Partial<MeasuringDocumentView> }).docView;
  if (!documentView || typeof documentView.measureTextSize !== 'function') return false;
  const measure = documentView.measureTextSize.bind(documentView);
  const loads = loadedFonts(view.dom.ownerDocument);
  let retained: { signature: string; size: TextSize } | null = null;
  documentView.measureTextSize = (): TextSize => {
    const signature = fontSignature(view, loads);
    const scaleX = view.scaleX;
    const scaleY = view.scaleY;
    if (retained?.signature === signature && scaleX > 0 && scaleY > 0)
      return {
        lineHeight: retained.size.lineHeight * scaleY,
        charWidth: retained.size.charWidth * scaleX,
        textHeight: retained.size.textHeight * scaleY,
      };
    const measured = measure();
    retained = usable(measured, scaleX, scaleY)
      ? {
          signature,
          size: {
            lineHeight: measured.lineHeight / scaleY,
            charWidth: measured.charWidth / scaleX,
            textHeight: measured.textHeight / scaleY,
          },
        }
      : null;
    return measured;
  };
  return true;
}
