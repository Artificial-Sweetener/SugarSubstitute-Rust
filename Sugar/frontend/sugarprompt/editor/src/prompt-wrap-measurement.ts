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
/** Measure native rendered source without temporarily forcing unmeasured text onto one line. */
import type { EditorView } from '@codemirror/view';
import type { PromptWrapGroup } from './prompt-wrap-groups.js';

/** Native inline fragments can include both a widget's outer box and its nested glyph boxes. */
export interface WrapRectangle {
  readonly left: number;
  readonly right: number;
  readonly top: number;
  readonly bottom: number;
}

/** Sum visual rows while counting overlapping widget and glyph rectangles only once. */
export function renderedPromptWidth(rectangles: readonly WrapRectangle[]): number {
  const sorted = rectangles
    .filter((rect) => rect.right > rect.left && rect.bottom > rect.top)
    .sort((left, right) => left.top - right.top || left.left - right.left);
  let width = 0;
  let bottom = -Infinity;
  let row: WrapRectangle[] = [];
  /** Merge one row's horizontal intervals, including nested native widget boxes. */
  const finish = (): void => {
    row.sort((left, right) => left.left - right.left);
    let end = -Infinity;
    for (const rect of row) {
      width += Math.max(0, rect.right - Math.max(end, rect.left));
      end = Math.max(end, rect.right);
    }
    row = [];
  };
  for (const rect of sorted) {
    if (rect.top >= bottom - 0.01 && row.length) finish();
    row.push(rect);
    bottom = Math.max(row.length === 1 ? rect.bottom : bottom, rect.bottom);
  }
  finish();
  return width;
}

/** Borrow one reusable native range so layout probes do not accumulate live DOM endpoints. */
export function measurePromptWrapGroup(
  view: Pick<EditorView, 'domAtPos' | 'contentDOM'>,
  group: PromptWrapGroup,
  scale: number,
  range: Range,
): number {
  const start = view.domAtPos(group.from),
    end = view.domAtPos(group.to);
  try {
    range.setStart(start.node, start.offset);
    range.setEnd(end.node, end.offset);
    return renderedPromptWidth(Array.from(range.getClientRects())) / scale;
  } finally {
    range.setStart(view.contentDOM, 0);
    range.collapse(true);
  }
}
