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
/** Pure trigger insertion slots preserve exact source boundaries and never normalize configured words. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
import { strip } from '../prompt/text.js';
/** Explicit selection replacement or a captured context position, measured in exact-source UTF-16. */
export type TriggerInsertionTarget =
  | {
      readonly kind: 'selection';
      readonly range: SourceRange;
    }
  | {
      readonly kind: 'position';
      readonly position: number;
    };
/** One source-derived slot can prepare every model alternative without rescanning the source. */
export interface TriggerInsertionSlot {
  readonly range: SourceRange;
  readonly prefix: string;
}
/** Refuse invalid scalar boundaries and positions inside exact CRLF pairs. */
function validBoundary(source: string, position: number): boolean {
  if (!Number.isSafeInteger(position) || position < 0 || position > source.length) return false;
  return !(
    (source[position - 1] === '\r' && source[position] === '\n') ||
    (/[\uD800-\uDBFF]/u.test(source[position - 1] ?? '') &&
      /[\uDC00-\uDFFF]/u.test(source[position] ?? ''))
  );
}
/** Capture the clicked comma/physical-line segment end; line-local separators correct the donor's CRLF split and empty-line prefix. */
export function prepareTriggerInsertionSlot(
  source: string,
  target: TriggerInsertionTarget,
): TriggerInsertionSlot | null {
  if (target.kind === 'selection') {
    const { from, to } = target.range;
    if (from >= to || !validBoundary(source, from) || !validBoundary(source, to)) return null;
    return Object.freeze({
      range: Object.freeze({ from, to }),
      prefix: '',
    });
  }
  if (!validBoundary(source, target.position)) return null;
  let end = target.position;
  while (
    end < source.length &&
    ![',', '\r', '\n'].includes(requireValue(source[end], 'trigger-insertion'))
  )
    end++;
  const lineStart =
    Math.max(source.lastIndexOf('\r', end - 1), source.lastIndexOf('\n', end - 1)) + 1;
  const left = strip(source.slice(lineStart, end), 'right');
  return Object.freeze({
    range: Object.freeze({ from: end, to: end }),
    prefix: left ? (left.endsWith(',') ? ' ' : ', ') : '',
  });
}
/** Whole configured entries retain commas, underscores, parentheses and Unicode; blank metadata creates no action. */
export function triggerInsertionText(slot: TriggerInsertionSlot, words: string): string | null {
  const text = strip(words);
  return text ? `${slot.prefix}${text}` : null;
}
