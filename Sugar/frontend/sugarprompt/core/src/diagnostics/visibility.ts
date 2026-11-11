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
/** Prepared token boundaries and native caret positions determine diagnostic visibility. */
import { whitespace } from '../prompt/text.js';

const punctuation = new Set(Array.from(',;:.!?)]}"\''));

/** Classify one adjacent scalar with pinned Python whitespace and the reference's commit punctuation; empty input is uncommitted. */
export function isDiagnosticCommitBoundary(character: string): boolean {
  const point = character.codePointAt(0);
  if (point === undefined || character.length !== (point > 0xffff ? 2 : 1)) return false;
  return whitespace.test(character) || punctuation.has(character);
}

/** Hide an editable-token diagnostic at an interior caret or an uncommitted end. Bounds and caret must share one UTF-16 coordinate space: exact source for inspection, logical document for native projection. */
export function diagnosticIsActive(
  from: number,
  to: number,
  caret: number,
  committedAtEnd: boolean,
): boolean {
  if (![from, to, caret].every(Number.isSafeInteger) || from < 0 || to < from || caret < 0)
    throw new RangeError('Invalid diagnostic position');
  return (from < caret && caret < to) || (caret === to && !committedAtEnd);
}
