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
/** Adapt CodeMirror's visual bidi and atomic motion to the browser's full Unicode character boundaries. */
import { EditorSelection, type SelectionRange } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { nativeGraphemeRange, nativeGraphemeWidths } from './native-graphemes.js';

/** Preserve native atomic traversal when a logical-document UTF-16 boundary falls within concealed source. */
function atomicPosition(view: EditorView, position: number): boolean {
  let atomic = false;
  for (const ranges of view.state.facet(EditorView.atomicRanges))
    ranges(view).between(position, position, (from, to) => {
      if (from < position && position < to) atomic = true;
    });
  return atomic;
}

/** Return a logical-document UTF-16 boundary while retaining CodeMirror's visual association and run level where applicable. */
function boundaryCursor(
  view: EditorView,
  native: SelectionRange,
  position: number,
  forward: boolean,
): SelectionRange {
  if (position === native.head || atomicPosition(view, position)) return native;
  const line = view.state.doc.lineAt(position);
  const level = view
    .bidiSpans(line)
    .some(
      (span) =>
        span.level === native.bidiLevel &&
        span.from <= position - line.from &&
        position - line.from <= span.to,
    )
    ? (native.bidiLevel ?? undefined)
    : undefined;
  return EditorSelection.cursor(position, forward ? -1 : 1, level);
}

/** One supported visual step resolves logical-document UTF-16 ranges; native run-edge canonicalization and atomic skips remain CodeMirror-owned. */
export function moveByGrapheme(
  view: EditorView,
  start: SelectionRange,
  forward: boolean,
): SelectionRange {
  let crossed = '';
  const moved = view.moveByChar(start, forward, (initial) => {
    crossed = initial;
    return () => false;
  });
  if (moved.head === start.head || !crossed) return moved;
  const from = Math.min(start.head, moved.head),
    to = Math.max(start.head, moved.head);
  if (to - from === crossed.length && view.state.doc.sliceString(from, to) === crossed) {
    const sourceForward = moved.head > start.head;
    const range = nativeGraphemeRange(view, start.head, sourceForward);
    return range
      ? boundaryCursor(view, moved, sourceForward ? range.to : range.from, sourceForward)
      : moved;
  }
  const widths = nativeGraphemeWidths(crossed);
  if (widths.first < crossed.length) {
    const inverse = view.moveByChar(moved, !forward);
    const sourceForward = inverse.assoc > 0;
    const edge = inverse.head;
    const interval = sourceForward
      ? { from: edge, to: edge + crossed.length }
      : { from: edge - crossed.length, to: edge };
    if (
      interval.from >= 0 &&
      interval.to <= view.state.doc.length &&
      view.state.doc.sliceString(interval.from, interval.to) === crossed
    )
      return boundaryCursor(
        view,
        inverse,
        sourceForward ? edge + widths.first : edge - widths.last,
        sourceForward,
      );
  }
  const sourceForward = moved.assoc < 0;
  const range = nativeGraphemeRange(view, moved.head, sourceForward);
  if (range && range.from < moved.head && moved.head < range.to)
    return boundaryCursor(view, moved, sourceForward ? range.to : range.from, sourceForward);
  return moved;
}

/** CodeMirror owns row hit testing and the preferred column; its native read phase chooses the nearest full character edge in logical-document UTF-16. */
export function moveByGraphemeLine(
  view: EditorView,
  start: SelectionRange,
  forward: boolean,
  extend: boolean,
): SelectionRange {
  let moved = view.moveVertically(start, forward);
  if (!extend && moved.head === start.head) moved = view.moveToLineBoundary(start, forward);
  const range = nativeGraphemeRange(view, moved.head, true);
  if (!range || moved.head === range.from || moved.head === range.to) return moved;
  let closest: number | null = null;
  view.requestMeasure({
    /** Supported coordinate reads share CodeMirror's read phase; no parallel row or caret geometry is retained. */
    read: () => {
      const from = view.coordsAtPos(range.from, 1),
        to = view.coordsAtPos(range.to, -1),
        origin = view.coordsAtPos(start.head, start.assoc || 1);
      if (!from || !to || !origin) return null;
      const target =
        moved.goalColumn === undefined
          ? origin.left
          : view.contentDOM.getBoundingClientRect().left + moved.goalColumn;
      return Math.abs(from.left - target) <= Math.abs(to.left - target) ? range.from : range.to;
    },
    /** Publish one transient command result; the native view alone owns scheduled measurement lifetime. */
    write: (position) => {
      closest = position;
    },
  });
  // The supported coordinate API flushes the requested native measurement before this synchronous command dispatches.
  view.coordsAtPos(moved.head, moved.assoc || 1);
  const position = closest ?? (moved.assoc < 0 ? range.to : range.from);
  if (atomicPosition(view, position)) return moved;
  return EditorSelection.cursor(
    position,
    position === range.from ? 1 : -1,
    moved.bidiLevel ?? undefined,
    moved.goalColumn,
  );
}
