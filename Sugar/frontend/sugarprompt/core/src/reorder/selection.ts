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
/** Directional chip-relative endpoint mapping is pure; the native editor owns actual caret and selection. */
import { requireValue } from '../required.js';
import { codePointBoundaries } from '../offsets.js';
import { freezeStructure } from '../prompt/text.js';
import type { PreparedValueReorder, ReorderOutput, ReorderSelection } from './contracts.js';
/** Validate decoded UTF-16 endpoints without accepting half a scalar or half an authored CRLF. */
function boundary(text: string, at: number): boolean {
  return (
    Number.isSafeInteger(at) &&
    codePointBoundaries(text).includes(at) &&
    !(at > 0 && text[at - 1] === '\r' && text[at] === '\n')
  );
}
/** Capture the containing or preceding source-owned chip and both original directional offsets. */
export function captureReorderSelection(
  prepared: PreparedValueReorder,
  anchor: number,
  head: number,
): ReorderSelection | null {
  const source = prepared.scope.document.sourceText;
  if (!boundary(source, anchor) || !boundary(source, head)) return null;
  const primary = Math.min(anchor, head);
  const boundaries = codePointBoundaries(source);
  const last = Math.max(anchor, head);
  const secondary =
    anchor === head
      ? primary
      : requireValue(boundaries[Math.max(0, boundaries.indexOf(last) - 1)], 'selection');
  /** Match containing endpoints before choosing a preceding separator boundary. */
  const containing = (position: number) =>
    prepared.chips.find(
      (item) => item.contentRange.from <= position && position <= item.contentRange.to,
    );
  /** Source-order chips make the last ending before the endpoint its bounded fallback. */
  const preceding = (position: number) =>
    prepared.chips.findLast((item) => item.contentRange.to <= position);
  const chip =
    containing(primary) ?? containing(secondary) ?? preceding(primary) ?? preceding(secondary);
  if (!chip) return null;
  /** Off-chip endpoints request a whole-chip restored selection, preserving direction. */
  const offset = (position: number): number | null =>
    chip.contentRange.from <= position && position <= chip.contentRange.to
      ? position - chip.contentRange.from
      : null;
  return freezeStructure({
    chip: chip.index,
    anchorOffset: offset(anchor),
    headOffset: offset(head),
    reversed: anchor > head,
  });
}
/** Map captured offsets into actual serialized content, with explicit refusal for unrelated output. */
export function mapReorderSelection(
  prepared: PreparedValueReorder,
  output: ReorderOutput,
  selection: ReorderSelection,
): {
  readonly anchor: number;
  readonly head: number;
} | null {
  const chip = prepared.chips[selection.chip],
    range = output.contentRanges[selection.chip];
  if (!chip || !range || output.text.slice(range.from, range.to) !== chip.text) return null;
  const anchor =
    selection.anchorOffset === null || selection.headOffset === null
      ? selection.reversed
        ? range.to
        : range.from
      : range.from + selection.anchorOffset;
  const head =
    selection.anchorOffset === null || selection.headOffset === null
      ? selection.reversed
        ? range.from
        : range.to
      : range.from + selection.headOffset;
  if (
    anchor < range.from ||
    anchor > range.to ||
    head < range.from ||
    head > range.to ||
    !boundary(output.text, anchor) ||
    !boundary(output.text, head)
  )
    return null;
  return freezeStructure({ anchor, head });
}
